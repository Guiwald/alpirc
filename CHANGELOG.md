# Changelog

Toutes les évolutions notables d'AlpIRC. Format [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/),
numérotation [SemVer](https://semver.org/lang/fr/) adaptée (voir `docs/versions.md`). Chaque entrée commence par
le composant concerné (« Scripts : … »). La traduction anglaise, section par section, est dans
[`CHANGELOG.en.md`](CHANGELOG.en.md).

## [Non publié]

### Ajouté
- Socle : l'espace de travail Rust (protocole, cœur, réseau, scripts, interface, relais, applis de bureau et
  Android) et les 13 décisions de départ (`docs/adr`), dont le nom, AlpIRC, et le périmètre de la 1.0.
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
