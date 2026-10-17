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
- Publication : `cargo xtask` (versions, page des versions, icônes, `.app`/`.dmg`, `.apk` sans Gradle, `.tar.gz`,
  `.deb` de l'appli et du relais, licences), la compilation automatique sur GitHub (tests Linux ; compilation
  macOS, Android, Linux) et le flux « Publication » qui fabrique les fichiers d'une version et le dépôt APT signé.
