# Glossaire

Le vocabulaire commun du projet. Un terme = un sens ; le code, les docs et les conversations utilisent ces mots-là.
Les termes anglais entre parenthèses sont ceux de mIRC et d'IRC, souvent gardés dans le code.

## IRC

| Terme | Sens |
|---|---|
| **IRC** | Internet Relay Chat : le protocole de discussion en direct (RFC 1459, RFC 2812, extensions IRCv3). |
| **Réseau** (network) | Un ensemble de serveurs reliés entre eux : Undernet, Libera.Chat, IRCnet… (ADR 0010). |
| **Serveur** | Une machine d'un réseau, à laquelle le client se connecte (adresse, port, TLS ou non). |
| **Canal** (channel) | Un salon de discussion dont le nom commence par `#` (ou `&`) : `#lyon`, `#alpirc-test`. |
| **Pseudo** (nick) | Le nom sous lequel on apparaît. **Pseudo de secours** : celui qu'AlpIRC essaie si le premier est pris. |
| **Privé** (query) | Une conversation à deux, dans sa propre fenêtre. |
| **Notice** | Un message qui n'appelle pas de réponse automatique (services, serveurs, CTCP). |
| **Sujet** (topic) | La phrase affichée en haut d'un canal. |
| **Mode** | Un réglage d'un canal (`+m` modéré, `+k` clé, `+l` limite…) ou d'un utilisateur (`+i` invisible, `+x` adresse masquée). |
| **Op, demi-op, voice** | Les rangs dans un canal (`@`, `%`, `+` devant le pseudo) : l'op gère le canal, le voice peut parler quand le canal est modéré. |
| **Kick, ban** | Expulser d'un canal ; interdire d'y revenir selon un **masque** (`pseudo!ident@hôte`, avec jokers). |
| **Whois** | La fiche d'un utilisateur (nom, adresse, canaux, compte, inactivité). |
| **Away** | « Absent » : un message renvoyé à ceux qui vous écrivent. |
| **Numérique** (numeric) | Une réponse de serveur identifiée par un nombre à trois chiffres (`001` bienvenue, `433` pseudo pris…). |
| **ISUPPORT** | La réponse `005` où le serveur annonce ses règles (préfixes, modes, longueur des pseudos, casse). |
| **IRCv3** | Les extensions modernes d'IRC, négociées par **CAP** : `server-time`, `message-tags`, `echo-message`, `batch`, `labeled-response`, `MONITOR`, `STS`… (ADR 0004). |
| **Étiquettes** (message tags) | Les informations en tête d'une ligne IRCv3 (`@time=…;account=…`). |
| **SASL** | L'identification au réseau pendant la connexion (PLAIN, EXTERNAL, SCRAM-SHA-256). |
| **NickServ** | Le service qui protège les pseudos enregistrés sur la plupart des réseaux (pas sur Undernet). |
| **X** | Le service d'Undernet qui tient les comptes et les canaux enregistrés (Undernet n'a pas de NickServ) (ADR 0010). |
| **LoC** (Login on Connect) | La connexion à X dès la connexion au serveur, par le mot de passe serveur `+x! compte mot-de-passe` : connexion seulement si X est là, adresse IP masquée (`+x`). |
| **CTCP** | Des messages spéciaux entre clients (`VERSION`, `PING`, `TIME`, `ACTION` pour `/me`). **Anti-flood CTCP** : AlpIRC limite ses réponses. |
| **DCC** | Une connexion directe entre deux clients, hors serveur : **DCC CHAT** (discussion), **DCC SEND/GET** (fichiers, avec reprise), **DCC passif** (quand on est derrière une box). |
| **TLS** | Le chiffrement de la connexion au serveur (port 6697 en général), par défaut dans AlpIRC. |
| **Encodage** | La façon d'écrire les lettres en octets : UTF-8 par défaut, repli Windows-1252 (Latin-1 étendu) pour les vieux canaux français (ADR 0012). |

## La fenêtre d'AlpIRC (façon mIRC 5.x/6.x, ADR 0005)

| Terme | Sens |
|---|---|
| **Fenêtre principale** | La fenêtre du système qui contient tout : barre de menus, barre d'outils, switchbar, fenêtres internes. |
| **Fenêtres internes** (MDI) | Les fenêtres Status, canaux et privés, à l'intérieur de la fenêtre principale ; elles se déplacent, se réduisent, s'agrandissent (le plus courant), se disposent en cascade ou en mosaïque. |
| **Fenêtre Status** | La fenêtre d'un réseau : messages du serveur, réponses aux commandes. Une par réseau. |
| **Switchbar** | La barre de boutons en haut (une par fenêtre interne) pour passer d'une fenêtre à l'autre ; le bouton change de couleur quand il y a du nouveau. |
| **Treebar** | La variante en arbre de la switchbar, à gauche (réseau › canaux et privés), en option. |
| **Barre d'outils** (toolbar) | Les boutons sous la barre de menus : connexion, options, Channels Folder, DCC… |
| **Ligne de saisie** (editbox) | Le champ en bas d'une fenêtre où l'on tape messages et commandes (`/join #lyon`). Historique, complétion Tab, Ctrl+K pour les couleurs. |
| **Liste des pseudos** (nicklist) | La colonne à droite d'un canal, triée par rang (`@`, `%`, `+`). |
| **Codes de couleur** | Ctrl+K (couleur, palette de 16 puis 99), Ctrl+B (gras), Ctrl+I (italique), Ctrl+U (souligné), Ctrl+R (inverse), Ctrl+O (normal). |
| **Mots surlignés** (highlight) | Les mots (dont votre pseudo) qui colorent la ligne, font clignoter le bouton de la fenêtre et déclenchent une notification. |
| **Ligne de repère** (line marker) | Le trait qui marque, dans une fenêtre, où l'on s'était arrêté de lire. |
| **Marque de lecture** (read marker) | La position de lecture partagée entre appareils par le relais ou un bouncer (`draft/read-marker`) : ce qui est lu sur le téléphone l'est aussi sur le Mac. |
| **Thème** | Un jeu de couleurs (classique façon mIRC, sombre, ou ceux de l'utilisateur) et une police. |
| **Boîte Options** | La grande boîte de réglages de mIRC, avec son arbre de catégories (Connexion, IRC, Affichage, Sons…). |
| **Channels Folder** | Les favoris : canaux à rejoindre d'un clic ou automatiquement (**auto-join**), avec leur clé. |
| **Channel Central** | La boîte d'un canal (double-clic dans le canal) : sujet, modes, listes de bans, d'exceptions et d'invitations. |
| **Liste des canaux** | La fenêtre de `/list` : tous les canaux d'un réseau, avec leur nombre d'utilisateurs et leur sujet, filtrables. |
| **URL catcher** | La fenêtre qui collecte les adresses web passées dans les discussions. |
| **Journaux** (logs) | Les fichiers texte où chaque fenêtre est enregistrée, relisibles depuis AlpIRC. |

## Carnet d'adresses et automatismes

| Terme | Sens |
|---|---|
| **Notify** | La liste des amis dont AlpIRC signale l'arrivée et le départ (par `MONITOR` quand le serveur sait faire). |
| **Ignore** | La liste des masques dont on ne veut plus voir les messages (par type : privés, canaux, CTCP, DCC…). |
| **Control** | Les listes d'auto-op, d'auto-voice et de protection : AlpIRC donne le rang tout seul à ceux qu'il reconnaît. |
| **Perform** | Les commandes envoyées automatiquement à la connexion à un réseau (identification, modes, canaux). |
| **Reconnexion automatique** | AlpIRC se reconnecte après une coupure et rejoint les canaux ouverts. |
| **Anti-flood** | La temporisation des envois, pour ne pas se faire déconnecter par le serveur pour excès de messages. |
| **Garde-fou au collage** | La question posée avant d'envoyer un texte collé de plusieurs lignes. |

## Scripts (mSL, ADR 0007)

| Terme | Sens |
|---|---|
| **mSL** | mIRC Scripting Language : le langage de script de mIRC, réimplémenté par le moteur d'AlpIRC (`alpirc-script`). |
| **Alias** | Une commande ajoutée par un script (`/salut` → plusieurs commandes). |
| **Popups** | Les menus du clic droit (fenêtres, liste des pseudos, barre de menus), définis par les scripts. |
| **Remote** | Les réactions aux événements : `on TEXT`, `on JOIN`, `on CONNECT`…, `ctcp`, `raw` (numériques). |
| **Groupe** | Un bloc de remotes qu'on allume ou éteint (`#nom on`, `#nom off`). |
| **Identifiant** | Une valeur calculée, qui commence par `$` : `$nick`, `$chan`, `$calc(…)`, avec ses propriétés (`$nick(#,1).pnick`). |
| **Variable** | Une valeur nommée qui commence par `%` : locale (`var`) ou globale (`set`, enregistrée). |
| **Niveau d'utilisateur** | Le rang donné à un masque dans la liste Users, qui décide quels remotes réagissent pour lui. |
| **Timer** | Une commande lancée plus tard ou à intervalles (`/timer`). |
| **Table de hachage** | Un tableau clé → valeur en mémoire, enregistrable (`/hadd`, `$hget`). |
| **Fenêtre @** | Une fenêtre personnalisée créée par un script (`/window @nom`). |
| **Dialogue** | Une boîte de dialogue décrite par un script (`dialog`, `/did`, `$did`, `on DIALOG`), dessinée avec les widgets d'AlpIRC. |
| **Éditeur de scripts** | La fenêtre Alt+R, avec ses onglets Aliases, Popups, Remote, Users, Variables. |
| **Bac à sable** | Les limites des scripts : pas de `/run`, ni `$dll`, ni `$com`, fichiers seulement dans le dossier des scripts, aucun DCC accepté tout seul. |

## Relais et bouncers (ADR 0009)

| Terme | Sens |
|---|---|
| **Relais** (bouncer) | Un programme qui tourne en permanence sur une machine allumée, reste connecté aux réseaux sous un seul pseudo, garde l'historique, et auquel les appareils se connectent. Celui d'AlpIRC s'appelle `alpirc-relay`. |
| **soju, ZNC** | Deux bouncers connus ; AlpIRC sait s'en servir, et le relais d'AlpIRC parle comme soju. |
| **bouncer-networks** | L'extension de soju (`soju.im/bouncer-networks`) qui permet de gérer plusieurs réseaux par une seule connexion au relais. |
| **chathistory** | L'extension IRCv3 (`draft/chathistory`) qui permet de redemander l'historique d'une fenêtre : le rattrapage à la connexion. |
| **Empreinte du certificat** | Tant que le relais n'a pas de certificat reconnu, le client retient son empreinte à la première connexion (comme SSH) et refuse ensuite un certificat différent. |

## Android (ADR 0006)

| Terme | Sens |
|---|---|
| **Tiroir** | Le panneau qui glisse depuis la gauche : réseaux, canaux et privés (la switchbar du téléphone). |
| **Champ de saisie natif** | La ligne de saisie d'Android (un `EditText`), pour avoir le clavier complet : autocorrection, dictée, emoji. |
| **Service de connexion** | La partie d'AlpIRC qui garde les connexions ouvertes appli en arrière-plan, avec sa notification (« Connecté à 2 réseaux ») : classe Java `ConnectionService`, service au premier plan de type `specialUse`. |

## Projet

| Terme | Sens |
|---|---|
| **AlpIRC** | Le client (ADR 0013) ; `alpirc` pour le programme, les crates et les identifiants. |
| **Mise en service** (Rollout) | Le paragraphe de chaque version du CHANGELOG qui dit quoi faire pour l'installer (`docs/versions.md`). |
| **Page des versions** | L'artefact claude.ai « Versions d'AlpIRC », tiré du CHANGELOG par `cargo xtask changelog-page`. |
| **Dépôt APT** | Sous Debian, Ubuntu et Mint, l'endroit d'où `apt` installe et met à jour AlpIRC et son relais ; signé par la clé du projet, publié sur GitHub Pages. |
| **`#alpirc-test`** | Le canal d'essai sur Undernet et Libera ; jamais d'essai dans les vrais canaux sans Guillaume (ADR 0010). |
