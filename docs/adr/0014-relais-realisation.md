# ADR 0014 — Réalisation du relais (`alpirc-relay`)

- Statut : acceptée (17/10/2026 ; réalise l'ADR 0009)

## Contexte

L'ADR 0009 décide d'un relais (bouncer) écrit en Rust dans la 1.0 : plusieurs appareils sur la même session,
historique gardé sur le relais, compatibilité avec les extensions de soju, conçu pour plusieurs comptes. Cette
fiche dit comment il est fait, ce qu'il parle du protocole, ce qu'il garde sur le disque, sa sécurité, et ce qui
n'est pas fait.

## Décision

### 1. Architecture

- Un seul fil tient tout l'état (utilisateurs, connexions aux réseaux, appareils, historique, marques de lecture)
  et reçoit les événements d'`alpirc-net` par un canal : pas de verrou, un ordre déterministe. `alpirc-net` sert
  des deux côtés : écoutes TLS (`Net::listen`) et connexions aux réseaux (`Net::connect`).
- Côté réseau, chaque (utilisateur, réseau) a une connexion permanente au-dessus de la `Session`
  d'`alpirc-core` (enregistrement, CAP 302, SASL, Login on Connect d'Undernet, ISUPPORT, canaux, membres, IAL).
  Le relais ne réécrit pas le protocole : il le réutilise.
- Modules : `relay` (le cœur), `upstream` (connexion à un réseau, anti-flood), `downstream` (appareil, mise en
  forme selon ses capacités), `routing` (à quel appareil va une réponse), `history`, `markers`, `networks`
  (rangement sur le disque), `config`, `auth`, `ids`.

### 2. Identification et sécurité

- Mot de passe de serveur `utilisateur[/réseau][@appareil]:mot-de-passe` (comme soju), ou le mot de passe seul
  avec l'utilisateur dans `USER`. SASL PLAIN avec la même identité (`gui/Libera@téléphone`). Le réseau peut aussi
  se choisir par `BOUNCER BIND <id>` avant `CAP END`.
- Empreintes **argon2id** (paramètres par défaut d'argon2 : 19 Mio, 2 passes) au format PHC, dans `relay.toml`.
  `--add-user` et `--passwd` lisent le mot de passe sur l'entrée standard (sans écho sur un terminal) et modifient
  le fichier en gardant ses commentaires, son propriétaire et ses droits. Un utilisateur inconnu coûte le même
  calcul qu'un mot de passe faux ; au-delà de 8 échecs en une minute, une adresse est refusée sans calcul.
- **TLS** par défaut (port 6697). Sans certificat, le relais crée un certificat autosigné ECDSA P-256 valable
  10 ans (`--init` ou premier démarrage, clé en 0600) ; `--init` et `--fingerprint` affichent son empreinte
  SHA-256, qu'AlpIRC épingle (`verify = "pinned"`, comme SSH). Un vrai certificat (Let's Encrypt) se met dans
  `cert` et `key`. Une écoute en clair ouverte au-delà de la machine déclenche un avertissement.
- Fichiers du dossier de données écrits en 0600 (mots de passe des réseaux ajoutés, historique).

### 3. Côté réseaux

- Capacités demandées : celles qu'`alpirc-core` lit (multi-prefix, extended-join, account-notify, away-notify,
  chghost, setname, userhost-in-names, invite-notify, server-time, message-tags, batch, cap-notify, account-tag,
  standard-replies), **sans** echo-message, labeled-response, chathistory, read-marker ni bouncer-networks : le
  relais les assure lui-même pour ses appareils. `WHO` à l'arrivée sur chaque canal, pour connaître tout le monde.
- Anti-flood : seau de 10 lignes, puis une toutes les 1,5 s (réglable) ; PONG et QUIT passent tout de suite.
- Reconnexion : délai qui double de 10 s à 300 s, serveur suivant de la liste à chaque échec. PING après 90 s de
  silence, connexion tenue pour morte à 240 s, enregistrement abandonné à 90 s.
- À la fin du MOTD : perform (lignes IRC brutes ; `/` de tête toléré, `/msg`, `/notice`, `/quote` traduits),
  puis les canaux : ceux de `relay.toml` et ceux rejoints depuis les appareils (gardés dans `networks.json`,
  oubliés au PART ou au KICK), avec leurs clés.
- **Absence automatique** : sans appareil attaché, le relais se met absent (message réglable) et revient dès qu'un
  appareil se lie ; ses propres 305/306 ne sont pas montrés. Une absence posée par un appareil est respectée et
  remise après une reconnexion.
- Encodage par réseau (`auto`, `utf-8`, `latin1`, ADR 0012) ; les appareils parlent toujours UTF-8.

### 4. Côté appareils

- `CAP LS 302` offre : account-notify, account-tag, away-notify, batch, cap-notify, chghost, draft/chathistory,
  draft/read-marker, echo-message, extended-join, invite-notify, message-tags, multi-prefix, sasl (PLAIN),
  server-time, setname, soju.im/bouncer-networks, soju.im/bouncer-networks-notify, userhost-in-names.
- Un appareil lié reçoit 001–004, ISUPPORT du réseau complété de `CHATHISTORY=1000`,
  `MSGREFTYPES=timestamp,msgid` et `BOUNCER_NETID`, un MOTD court, ses modes, puis pour chaque canal : JOIN
  (étendu s'il le veut), sujet (332/333), NAMES, `MARKREAD` (avant 366), 366. Ensuite tout ce qui arrive du
  réseau, mis en forme pour lui : étiquettes non négociées retirées, JOIN étendu ramené, préfixes multiples et
  adresses retirés des NAMES et des WHO, AWAY/ACCOUNT/CHGHOST/SETNAME/TAGMSG/BATCH écartés s'il ne les connaît pas.
- Si le réseau est coupé, les appareils reçoivent un avis (et la notification `state=disconnected`) mais gardent
  leurs canaux : un PART fermerait les fenêtres d'AlpIRC. Au retour, les JOIN et NAMES du réseau leur arrivent,
  et le pseudo et l'ISUPPORT sont remis à jour.
- `QUIT` détache l'appareil sans quitter le réseau. Un appareil non lié ne sert qu'à la gestion des réseaux
  (`BOUNCER`) ; il reçoit des réponses locales aux commandes courantes (ISON, USERHOST, WHO, MODE, AWAY…) pour
  ne pas afficher d'erreurs. Un client qui ne connaît pas bouncer-networks, sur un compte à un seul réseau, y
  est lié d'office.

### 5. Plusieurs appareils sur une session

- Ce qu'un appareil envoie (PRIVMSG, NOTICE, TAGMSG) part **une fois** vers le réseau ; le relais le renvoie aux
  autres appareils comme venant de notre préfixe, et à l'appareil lui-même s'il a négocié echo-message. Le même
  `msgid` et la même heure servent partout, historique compris : les clients ne doublent rien.
- Les réponses à une question (WHOIS, WHO, LIST, NAMES, MODE `#canal` ou `+b`, TOPIC, USERHOST, ISON, MOTD,
  LUSERS, NICK refusé…) vont à l'appareil qui l'a posée, pas aux autres (file des questions par sorte de réponse,
  cible comparée quand des réponses de la même sorte peuvent venir seules, oubli après 60 s).
- `MODE`, `NAMES`, `TOPIC` et `WHO` (WHOX compris) d'un canal rejoint sont répondus depuis l'état connu : un
  appareil qui se lie en demande pour chaque canal, et le réseau n'est pas sollicité à chaque réveil du téléphone.
- Plusieurs appareils répondent au même CTCP : une seule réponse par cible et par commande part en 5 s.
- Un `NICK` réussi depuis un appareil est retenu pour les connexions suivantes.

### 6. Historique (`draft/chathistory`)

- Rangement : `<données>/history/<utilisateur>/<netid>/<cible>/<AAAA-MM-JJ>.jsonl`, un fichier par jour (UTC) et
  par cible (canal, ou l'autre personne d'un privé, nom replié selon CASEMAPPING et échappé), une ligne JSON par
  message (`msgid`, heure, nom affiché, source, commande, paramètres, étiquettes des clients), ajoutée en une
  écriture. Les heures d'une cible ne reculent jamais : l'ordre des lignes est celui du temps.
- `msgid` du relais : base64 (URL) de l'heure en ms, d'un compteur et d'un sel tiré au démarrage. L'heure incluse
  mène droit au fichier du jour. Le relais donne ses propres `msgid` à tout ce qu'il garde.
- Sont gardés : PRIVMSG et NOTICE (pas les requêtes et réponses CTCP, sauf ACTION), TAGMSG utiles (réactions,
  pas les seules indications de frappe), dans les deux sens. Pas JOIN, PART, QUIT… (pas d'`event-playback`).
- `CHATHISTORY LATEST` (`*`, `timestamp=`, `msgid=`), `BEFORE`, `AFTER`, `AROUND` (moitié avant, reste à partir du
  repère), `BETWEEN` (dans les deux sens), `TARGETS` (lot `draft/chathistory-targets`), 1000 messages au plus,
  bornes exclues, réponse en lot `chathistory` avec `draft/chathistory-end` quand il n'y a plus rien. Erreurs :
  `FAIL CHATHISTORY INVALID_PARAMS`, `INVALID_TARGET`.
- Ménage chaque jour : jours au-delà de `retention_days` (365 par défaut), lignes au-delà de
  `max_lines_per_target` (100 000 par défaut).

### 7. Marques de lecture (`draft/read-marker`)

- Par utilisateur, réseau et cible, dans `<données>/users/<nom>/readmarkers.json`. Une marque ne recule jamais et
  est bornée à maintenant ; une marque plus ancienne reçoit en réponse la marque connue. Chaque avance est annoncée
  à tous les appareils liés au réseau qui ont la capacité. `MARKREAD <cible>` rend la marque (ou `*`).

### 8. Réseaux (`soju.im/bouncer-networks`)

- `LISTNETWORKS` (lot `soju.im/bouncer-networks`) avec name, host, port, tls, nickname, username, realname, state,
  error ; jamais le mot de passe. `ADDNETWORK`, `CHANGENETWORK` (un changement de pseudo seul envoie `NICK`, le
  reste reconnecte), `DELNETWORK`, `BIND` avant l'enregistrement, erreurs de la spécification (`INVALID_NETID`,
  `INVALID_ATTRIBUTE`, `READ_ONLY_ATTRIBUTE`, `UNKNOWN_ATTRIBUTE`, `NEED_ATTRIBUTE`, `UNKNOWN_COMMAND`,
  `REGISTRATION_IS_COMPLETED`). Notifications à qui a `bouncer-networks-notify`, liste initiale comprise ; un
  changement ne porte que ce qui change (`state`, `error` posée puis retirée), le client fusionne.
- Identifiants (`1`, `2`…) stables et jamais réutilisés, dans `<données>/users/<nom>/networks.json` avec les
  réseaux ajoutés depuis un client, les changements faits aux réseaux de `relay.toml` et les canaux rejoints. Un
  réseau de `relay.toml` ne se supprime pas depuis un client (`FAIL BOUNCER NETWORK_IN_CONFIG`).

### 9. Programme et paquet

- `alpirc-relay --config <relay.toml> --data <dossier>` (ce que lance le service systemd du paquet `.deb`),
  `--init` (modèle commenté si absent, certificat, empreinte), `--fingerprint`, `--add-user <nom>`,
  `--passwd <nom>`, `--version`. Journal par `RUST_LOG` (`info` par défaut). SIGTERM ou SIGINT : QUIT aux réseaux,
  ERROR aux appareils, puis fin.
- Le modèle commenté `packaging/linux/alpirc-relay/relay.toml` est aussi celui qu'écrit `--init`.
- File d'envoi par appareil bornée (`[clients] max_send_queue_kib`, 4 Mio par défaut) : un appareil qui ne lit
  plus (téléphone injoignable) est coupé avec ERROR et une ligne de journal ; il rattrapera par l'historique. La
  taille de la file vient d'`alpirc-net` (`Net::pending_bytes`).
- Robustesse : lignes des appareils de plus de 512 octets (hors étiquettes) refusées (417), 64 lignes au plus
  avant l'enregistrement, 60 s pour s'enregistrer, anti-flood à l'entrée (500 lignes puis 25 par seconde, coupure
  après 50 lignes jetées), PING aux appareils silencieux (120 s) puis coupure (300 s), aucune panique sur une
  ligne mal formée (essai dédié).

## Ce qui n'est pas fait

- `labeled-response` n'est pas offert aux appareils ; pas d'`event-playback` (les JOIN, PART… ne sont pas dans
  l'historique).
- Le relais renvoie aux autres appareils un message dès qu'il part, sans attendre le réseau : un message refusé
  (404, canal modéré) est quand même montré et gardé ; l'erreur du réseau est transmise.
- SASL côté appareils : PLAIN seulement (ni SCRAM ni certificat client).
- Pas de rechargement des réglages à chaud (redémarrer le service), pas d'administration depuis IRC.
- STS des réseaux : ignoré (c'est le réglage du serveur qui décide du TLS).
- DCC : non relayé.
- Conçu pour plusieurs utilisateurs, essayé avec un seul (ADR 0009).

## Conséquences

- Essais : unitaires (réglages, mots de passe, `msgid` et heures, historique, marques, réseaux, aiguillage,
  mise en forme), d'intégration dans le processus (faux serveur IRC, relais, deux appareils en TLS épinglé),
  d'interopérabilité avec le moteur du client AlpIRC, et en vrai (ngircd, deux AlpIRC de bureau sur un écran
  virtuel).
- Remarques pour le client AlpIRC vues pendant les essais, corrigées au lot 3 : notifications `BOUNCER NETWORK`
  partielles fusionnées (le nom n'est plus remplacé par l'identifiant) ; un message privé écrit sur un autre
  appareil ouvre le privé ; la connexion parente (non liée) ne lance plus perform, favoris ni Notify ;
  `LISTNETWORKS` n'est plus demandé avec `bouncer-networks-notify`. Le relais garde ses réponses locales aux
  commandes courantes sur une connexion non liée, pour les autres clients.
