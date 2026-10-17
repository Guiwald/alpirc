# Versions d'AlpIRC

Convention commune aux outils de Guillaume (projet Outils, `conventions-versions.md`), appliquée ici (ADR 0011).

## Numéro

`MAJEUR.MINEUR.CORRECTIF`, source unique : le fichier `VERSION` à la racine, recopié dans
`[workspace.package] version` du `Cargo.toml` racine (toutes les crates ont `version.workspace = true`).

- **MAJEUR** : une action de l'utilisateur est nécessaire (désinstaller, refaire un réglage), ou une ancienne version
  cesse de marcher. Pour AlpIRC : aussi un changement incompatible du protocole entre le client et le relais, ou
  du format des réglages sans reprise automatique.
- **MINEUR** : nouvelle fonction, rien à faire.
- **CORRECTIF** : correction de bug uniquement.

Les préversions sont en `0.x`, publiées au fil de l'eau ; la `1.0.0` sort quand tout le périmètre de l'ADR 0004 est
livré et testé (ADR 0002). Le client, l'appli Android et le relais partagent la même version.

## Règles

1. Une version publiée ne change jamais : les commandes de paquet refusent d'écraser un fichier existant du dossier
   des paquets (`dist/`, ou `ALPIRC_DIST`), et le flux « Publication » n'ajoute à une version déjà publiée que les
   fichiers qui lui manquent.
2. Les fichiers livrés portent la version : `AlpIRC-X.Y.Z.dmg`, `AlpIRC-X.Y.Z.apk`,
   `AlpIRC-X.Y.Z-linux-x86_64.tar.gz`, `alpirc_X.Y.Z_amd64.deb` et `alpirc-relay_X.Y.Z_amd64.deb` (noms imposés
   par Debian pour les deux derniers).
3. `CHANGELOG.md` au format Keep a Changelog ; chaque version porte un paragraphe « Mise en service » (ce qu'il faut
   faire pour l'installer) et chaque entrée commence par son composant. `CHANGELOG.en.md` en est la traduction
   anglaise, section par section, avec les mêmes titres de version (« Rollout » pour « Mise en service ») : les
   notes de chaque version sur GitHub donnent le français puis l'anglais (`scripts/notes-de-version.sh`).
4. Android : `versionCode = MAJEUR×10000 + MINEUR×100 + CORRECTIF`. macOS : `CFBundleVersion` idem.
5. Page des versions : artefact claude.ai privé généré depuis `CHANGELOG.md` par `cargo xtask changelog-page`,
   republié à la même URL à chaque version : *Versions d'AlpIRC* — adresse à noter ici à la première version
   (depuis une autre conversation, passer cette URL comme `url` à la publication).

## Sortir une version

1. `cargo xtask bump patch|minor|major` (met à jour `VERSION`, le `Cargo.toml` racine, `Cargo.lock`, et date la
   section des deux CHANGELOG). `--dry-run` montre ce qui changerait sans rien écrire.
2. Compléter le paragraphe « Mise en service » de la nouvelle section, et la même section en anglais dans
   `CHANGELOG.en.md` (« Rollout », Added / Changed / Fixed).
3. Faire le commit, étiqueter (`git tag vX.Y.Z`) et envoyer (`scripts/publier-sur-github.sh` depuis la VM Linux) :
   le flux « Publication » vérifie que l'étiquette correspond à `VERSION`, fabrique le `.dmg` (Mac universel),
   l'`.apk` signé, le `.tar.gz` et le `.deb` (et celui du relais quand son programme existe), les met sur la page
   de la version avec les notes en français et en anglais, et publie le dépôt APT signé sur GitHub Pages.
4. `cargo xtask changelog-page` puis republier la page des versions.

Pour fabriquer un paquet à la main : `cargo xtask help` (les commandes `bundle-mac`, `apk`, `dist-linux`, `deb` et
`deb-relay` prennent le binaire déjà compilé).

## Clés de signature

- **Android** : `signature-android/alpirc-release.p12` et son mot de passe ; **dépôt APT** :
  `signature-apt/alpirc-apt-cle-privee.asc` et sa phrase de passe. Créées une fois par `scripts/creer-cles.sh`,
  rangées dans `~/perso/alpirc-mac/` (copie de secours chez Guillaume) et dans les secrets du dépôt GitHub
  (`ANDROID_KEYSTORE_B64`, `ANDROID_KEYSTORE_PASSWORD`, `APT_SIGNING_KEY`, `APT_SIGNING_PASSPHRASE`) par
  `scripts/publier-sur-github.sh`. Jamais dans le dépôt.
- La partie publique de la clé APT est versionnée : `packaging/linux/alpirc-archive-keyring.gpg` (et `.asc`). Sans
  elle, le `.deb` est fabriqué sans le dépôt APT (avertissement) et le flux « Publication » ne peut pas publier le
  dépôt.
- Perdre la clé Android empêche d'installer une nouvelle version par-dessus l'ancienne : il faudrait désinstaller
  (changement MAJEUR).
