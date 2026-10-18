# Changelog

Toutes les évolutions notables d'AlpIRC. Format [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/),
numérotation [SemVer](https://semver.org/lang/fr/) adaptée (voir `docs/versions.md`). Chaque entrée commence par
le composant concerné (« Scripts : … »). La traduction anglaise, section par section, est dans
[`CHANGELOG.en.md`](CHANGELOG.en.md).

## [Non publié]

## [0.16.0] - 2026-10-19

> Mise en service : rien à faire, installer par-dessus la 0.15.0 (réglages conservés).

### Ajouté
- Channel Central : le sujet propose la liste des derniers sujets du canal (les 10 derniers vus), comme dans mIRC,
  pour en remettre un ancien.
- Commandes : `/join -i` rejoint le dernier canal où l'on t'a invité, comme dans mIRC.

## [0.15.0] - 2026-10-19

> Mise en service : rien à faire, installer par-dessus la 0.14.0 (réglages conservés).

### Ajouté
- Interface : les raccourcis des boîtes de mIRC : Alt+L liste des canaux, Alt+N Notify, Alt+U URL, Alt+S envoi
  DCC, Alt+C discussion DCC, Alt+E choix du serveur, Alt+A favoris, Alt+X agrandir ou rétablir la fenêtre, Alt+1 à
  9 pour aller à une fenêtre (Linux ; sur Mac, Option sert à taper des caractères).

### Corrigé
- Interface : un double-clic dans le texte d'un canal ouvre Channel Central, comme dans mIRC ; dans Status, il
  fait `/lusers` ; dans un privé, `/whois`.
- Interface : Alt+O ouvrait le menu « Outils » au lieu des Options quand la langue donne la lettre O à un menu ;
  les raccourcis passent maintenant avant les lettres soulignées de la barre de menus, comme dans mIRC.

## [0.14.0] - 2026-10-19

> Mise en service : rien à faire, installer par-dessus la 0.13.0 (réglages conservés).

### Ajouté
- Interface : un menu au clic droit (appui long sur le téléphone) dans la liste des canaux (rejoindre, ajouter aux
  favoris, copier le nom, refaire la liste) et dans le capteur d'URL (ouvrir, copier, retirer, tout effacer), comme
  dans mIRC.
- Interface : « Enregistrer le texte... » dans le menu d'une fenêtre (Buffer › Save de mIRC) : tout le texte de la
  fenêtre, sans les couleurs, dans un fichier ; sur Android, dans le dossier des fichiers reçus.

## [0.13.0] - 2026-10-19

> Mise en service : rien à faire, installer par-dessus la 0.12.0 (réglages conservés).

### Ajouté
- Commandes de mIRC qui manquaient : `/anick` (pseudo de secours), `/mnick` (pseudo principal, sans changer celui
  du moment), `/tnick` (pseudo pour cette fois), `/fullname` (nom complet), `/ajinvite on|off`, `/creq` et `/sreq`
  (que faire des demandes de DCC), `/remote on|off`, `/toolbar`, `/switchbar` et `/treebar on|off`, `/qmsg` et
  `/qme` (dans tous les privés), `/clipboard`, `/savebuf` (le texte d'une fenêtre dans un fichier du dossier des
  scripts).
- Commandes : `/aop`, `/avoice` et `/protect`, comme dans mIRC (ajouter, `-r` retirer, `-l` montrer, `-w` tous les
  réseaux, type de masque, `on`/`off`) ; le carnet d'adresses a les cases « Listes actives » dans l'onglet Control.
- Commandes : `/dns hôte` ou `/dns pseudo` donne l'adresse IP (il répondait « non pris en charge »).
- Commandes : `/debug @fenêtre` (ou un fichier du dossier des journaux) montre les lignes échangées avec le serveur,
  mots de passe masqués ; `/debug off` l'arrête.

## [0.12.0] - 2026-10-19

> Mise en service : rien à faire, installer par-dessus la 0.11.0 (réglages conservés). Sur Mac et Linux, le dossier
> des canaux s'ouvre désormais à chaque connexion, comme dans mIRC : décoche « Ouvrir à la connexion » dans le
> dossier (Favoris › Dossier des canaux, Alt+J) si tu n'en veux pas.

### Ajouté
- Interface : le menu **Commandes** de la barre de menus, comme celui de mIRC (il était vide) : rejoindre ou
  quitter un canal, discuter en privé, envoyer une notice, whois, CTCP, absence, inviter, bannir, expulser, ignorer
  ou ne plus ignorer quelqu'un, ses modes, changer de pseudo, quitter IRC.
- Interface : le dossier des canaux s'ouvre à la connexion, comme dans mIRC (pas aux reconnexions automatiques).
  La case « Ouvrir à la connexion », dans le dossier, l'arrête. Sur le téléphone, elle est décochée.
- Docs : `docs/ecarts-mirc.md`, l'inventaire de ce qui diffère encore de mIRC 6.x, avec les questions ouvertes.

### Modifié
- Scripts : les menus par défaut suivent ceux de mIRC : « Ignorer » et le nouveau « Ne plus ignorer » en tête du
  sous-menu Contrôle de la liste des pseudos ; un sous-menu CTCP dans le menu d'un privé. Les menus que tu n'as pas
  modifiés sont mis à jour tout seuls.
- Android : la notification du service compte chaque réseau une fois (« Connecté à Undernet (2 connexions) » au
  lieu de « Connecté à 2 réseaux »).

## [0.11.0] - 2026-10-19

> Mise en service : rien à faire, installer par-dessus la 0.10.0 (réglages conservés). Mac : ouvrir le `.dmg` et
> remplacer AlpIRC dans Applications. Android : installer l'`.apk` par-dessus. Linux : `sudo apt update && sudo apt
> upgrade`. Si tu avais modifié les menus par défaut (`popups.mrc`, éditeur de scripts › Popups), ils ne sont pas
> touchés et « Infos » y fait encore un `/whois` : remplace `whois $$1` par `uwho $$1` sur les lignes « Infos »
> pour avoir User Central.

### Ajouté
- Interface : la boîte **User Central**, comme dans mIRC. « Infos » dans le menu d'un pseudo (liste des pseudos,
  fenêtre privée) ou `/uwho pseudo` montre la fiche whois de la personne (adresse, nom, serveur, canaux, compte,
  inactivité, heure de connexion, absence), sans rien écrire dans les fenêtres, et ce que ton carnet d'adresses
  garde sur elle : nom réel, e-mail, site, notes. Boutons Actualiser, Copier tout et Privé.
- Carnet d'adresses : un onglet **Infos**, la liste des personnes et de leurs notes (les mêmes que dans User
  Central).

### Modifié
- Android : « Whois dans la fenêtre active » (Options › IRC) est cochée d'office, la réponse d'un whois s'affiche
  dans la fenêtre où tu es au lieu de Status. Sur un téléphone où AlpIRC est déjà installé, elle est cochée une
  fois à la mise à jour ; si tu la décoches ensuite, elle le reste. Mac et Linux : rien ne change (décochée, comme
  dans mIRC).

### Corrigé
- Scripts : les menus du clic droit restaient dans la langue du premier lancement quand on changeait de langue
  (boîte de bienvenue, Options). Ceux que tu n'as pas modifiés suivent maintenant la langue de l'interface, et
  prennent les nouveautés de la version (« Infos » ouvre User Central).

## [0.10.0] - 2026-10-18

> Mise en service : rien à faire de particulier, installer par-dessus la 0.9.0 (réglages conservés). Mac : ouvrir
> le `.dmg` et remplacer AlpIRC dans Applications. Android : installer l'`.apk` par-dessus. Linux :
> `sudo apt update && sudo apt upgrade` (ou le nouveau `.deb`). C'est la version à essayer sur le Pixel 8 : les
> défauts trouvés dans l'émulateur Android y sont corrigés.

### Ajouté
- Interface : Affichage › Thème liste aussi tes thèmes ; Alt+lettre choisit un onglet dans les boîtes ; le nom
  complet d'une fenêtre s'affiche en info-bulle quand son bouton de la switchbar est réduit.
- Sons : `/splay stop` arrête le son en cours (Mac, Linux, Android).
- Android : toucher la notification d'une mention ou d'un message privé ouvre la fenêtre de la conversation.

### Modifié
- Langues : les erreurs de connexion, de certificat, de DCC et les messages des scripts suivent la langue de
  l'interface ; au premier lancement, le nom complet et le message d'absence sont créés dans ta langue ; sur Mac,
  le menu de l'appli dit « Quitter AlpIRC ».
- Interface : chaque raccourci souligné est unique dans une boîte ou un menu, dans les 7 langues ; Agrandir et
  Réduire la police suivent les tailles de la boîte Options ; les boîtes restent dans la fenêtre quand on la
  réduit ; les boutons de la switchbar prennent la largeur de leur nom.

### Corrigé
- Android : le clavier s'ouvre en touchant un champ d'une boîte, même après Retour ou quand la demande
  d'autorisation des notifications l'avait empêché au premier lancement (boîte de bienvenue) ; en paysage,
  l'interface ne passe plus sous l'encoche de la caméra ; une liste déroulante ouverte suit sa boîte quand le
  clavier se ferme ; les boutons « Envoyer » et « Tab » changent de langue avec l'interface ; « 1 personne » au
  singulier dans la liste des pseudos.
- Interface : la liste des canaux et le capteur d'URL se mettent à jour même quand le nombre de lignes ne change
  pas ; la dernière ligne de « À propos » n'est plus coupée.
- Publication : l'archive du site APT n'est plus jointe aux fichiers d'une version.

## [0.9.0] - 2026-10-18

> Mise en service : première version publiée, à installer telle quelle. Mac : ouvrir le `.dmg`, glisser AlpIRC
> dans Applications ; au premier lancement, Réglages Système › Confidentialité et sécurité › « Ouvrir quand même »
> (l'appli n'est pas signée par Apple, voir le README). Android : installer l'`.apk` (autoriser l'installation
> depuis le navigateur ou le gestionnaire de fichiers). Linux : `sudo apt install ./alpirc_0.9.0_amd64.deb`, qui
> ajoute aussi le dépôt APT pour les mises à jour. Version d'essai : tout le périmètre de la 1.0 est là, la 1.0
> sortira après les essais sur Mac et sur téléphone.

### Ajouté
- Socle : l'espace de travail Rust (protocole, cœur, réseau, scripts, interface, relais, applis de bureau et
  Android) et les décisions de départ (`docs/adr`), dont le nom, AlpIRC, et le périmètre de la 1.0.
- Protocole : lecture et écriture des lignes IRC (RFC 1459, RFC 2812) avec les étiquettes IRCv3, `ISUPPORT`, les
  modes, les numériques, les masques, CTCP, les codes de couleur de mIRC, l'UTF-8 avec repli Windows-1252, et
  SASL (PLAIN, EXTERNAL, SCRAM-SHA-256).
- Scripts : le moteur mSL (`alpirc-script`), en bac à sable : alias, popups, remotes (`on TEXT`, `ctcp`, `raw`…),
  groupes, `%variables`, `$identifiants`, `if`/`while`/`goto`, timers, tables de hachage, fichiers limités au
  dossier des scripts, niveaux d'utilisateurs et dialogues personnalisés ; ni `/run`, ni `$dll`, ni `$com`.
- Cœur : le moteur du client (`alpirc-core`) : plusieurs réseaux à la fois, TLS avec vérification, épinglage ou
  confiance d'un certificat, SASL, NickServ, compte X d'Undernet (connexion avec `+x`), STS, reconnexion,
  IRCv3 (server-time, echo-message, account-notify, away-notify, chghost, extended-join, multi-prefix, MONITOR,
  labeled-response, batch…), relais et bouncers compatibles soju (bouncer-networks, chathistory, read-marker).
- Commandes : celles de mIRC (`/join`, `/msg`, `/query`, `/me`, `/whois`, `/away`, `/ctcp`, `/mode`, `/kick`,
  `/ban`, `/ignore`, `/notify`, `/list`, `/server`, `/raw`, `/window`, `/closemsg`, `/timestamp`, `/splay`,
  `/sound`, `/help`…), l'historique, la complétion par Tab (pseudos, canaux, commandes, alias), Ctrl+K/B/U/I/R/O,
  la garde au collage, `$?` et `$input` (boîte de saisie).
- Interface : la fenêtre façon mIRC (`alpirc-ui`, `alpirc-shell`) : fenêtres MDI, switchbar en haut, treebar en
  option, barre d'outils, menus (natifs sur Mac), menus contextuels des scripts, thèmes Classique et Sombre et
  thèmes à soi, 16 + 99 couleurs, surlignage, couleurs des pseudos, choix de la police (Fixedsys Excelsior par
  défaut), icônes originales en pixel-art.
- Boîtes de dialogue : Options (toutes les pages, dont Sons et DCC), choix et éditeur des réseaux, carnet
  d'adresses (Notify, Ignore, Control), dossier des canaux, Channel Central, éditeur de scripts (Alt+R), couleurs,
  liste des canaux avec filtres, journaux, invite de certificat, accueil au premier lancement.
- Fenêtres : Notify (en direct), capteur d'URL, liste des canaux, journaux de conversation, absence automatique.
- DCC : CHAT, SEND/GET avec reprise et DCC passif, fenêtre des transferts, glisser-déposer d'un fichier.
- Sons : sons d'événements (surlignage, privé, Notify, DCC, connexion, expulsion, invitation…), sons originaux
  fournis, `/splay`, demandes CTCP SOUND (refusées par défaut), et notifications du système.
- Langues : l'interface en anglais, français, espagnol, allemand, italien, valencien et irlandais ; dates
  localisées.
- Mac : liens `irc://` et `ircs://`, notifications au nom d'AlpIRC, touche Option (`|`, `{`, `[`…), défilement au
  trackpad.
- Linux : une seule fenêtre AlpIRC (un lien `irc://` est confié à celle qui tourne déjà), sons.
- Android : la disposition téléphone (tiroirs des fenêtres et des pseudos, menus tactiles, gestes), la saisie
  native avec son historique, la connexion permanente (service avec notification), la déconnexion après N minutes
  en arrière-plan (désactivée par défaut), les notifications, les sons, le choix de fichiers pour le DCC.
- Relais : `alpirc-relay`, le relais d'AlpIRC (ADR 0014) : le même compte IRC sur le téléphone et l'ordinateur,
  historique sur disque, marqueurs de lecture, TLS épinglé, mots de passe argon2id, paquet `.deb` avec systemd.
- Publication : `cargo xtask` (versions, page des versions, icônes, `.app`/`.dmg`, `.apk` sans Gradle, `.tar.gz`,
  `.deb` de l'appli et du relais, licences), la compilation automatique sur GitHub (tests Linux ; compilation
  macOS, Android, Linux) et le flux « Publication » qui fabrique les fichiers d'une version et le dépôt APT signé.
