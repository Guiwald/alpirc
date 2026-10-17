#!/bin/bash
# Fabrique le dépôt APT d'AlpIRC à partir des paquets .deb (l'appli, et le relais s'il est là), et le signe : publié
# sur GitHub Pages par le flux « Publication », à l'adresse https://guiwald.github.io/alpirc/apt (suite « stable »,
# composant « main »).
#
# Usage : bash scripts/depot-apt.sh <dossier contenant les .deb> <dossier du site>
# La clé privée du dépôt doit être importée dans gpg ; sa phrase de passe dans la variable APT_SIGNING_PASSPHRASE.
# La clé publique doit être versionnée : packaging/linux/alpirc-archive-keyring.gpg et .asc
# (scripts/creer-cles.sh les fabrique). Il faut apt-ftparchive (paquet apt-utils).
set -euo pipefail
ICI="$(cd "$(dirname "$0")/.." && pwd)"
DEBS="$(cd "${1:?usage : depot-apt.sh <dossier des .deb> <dossier du site>}" && pwd)"
SITE="${2:?usage : depot-apt.sh <dossier des .deb> <dossier du site>}"
: "${APT_SIGNING_PASSPHRASE:?APT_SIGNING_PASSPHRASE manque}"
for f in alpirc-archive-keyring.gpg alpirc-archive-keyring.asc; do
  if [ ! -f "$ICI/packaging/linux/$f" ]; then
    echo "packaging/linux/$f manque : lancer scripts/creer-cles.sh et versionner la clé publique." >&2
    exit 1
  fi
done
command -v apt-ftparchive >/dev/null || { echo "apt-ftparchive manque : sudo apt-get install apt-utils" >&2; exit 1; }
shopt -s nullglob
APPLI=("$DEBS"/alpirc_*_amd64.deb)
RELAIS=("$DEBS"/alpirc-relay_*_amd64.deb)
if [ ${#APPLI[@]} -eq 0 ]; then
  echo "Aucun alpirc_*_amd64.deb dans $DEBS" >&2
  exit 1
fi

rm -rf "$SITE"
mkdir -p "$SITE/apt/pool/main/a/alpirc" "$SITE/apt/dists/stable/main/binary-amd64"
SITE="$(cd "$SITE" && pwd)"
cp "${APPLI[@]}" "$SITE/apt/pool/main/a/alpirc/"
if [ ${#RELAIS[@]} -gt 0 ]; then
  mkdir -p "$SITE/apt/pool/main/a/alpirc-relay"
  cp "${RELAIS[@]}" "$SITE/apt/pool/main/a/alpirc-relay/"
fi
cd "$SITE/apt"
apt-ftparchive packages pool > dists/stable/main/binary-amd64/Packages
gzip -9kn dists/stable/main/binary-amd64/Packages
printf 'Archive: stable\nComponent: main\nOrigin: AlpIRC\nLabel: AlpIRC\nArchitecture: amd64\n' > dists/stable/main/binary-amd64/Release
apt-ftparchive \
  -o APT::FTPArchive::Release::Origin=AlpIRC \
  -o APT::FTPArchive::Release::Label=AlpIRC \
  -o APT::FTPArchive::Release::Suite=stable \
  -o APT::FTPArchive::Release::Codename=stable \
  -o APT::FTPArchive::Release::Architectures=amd64 \
  -o APT::FTPArchive::Release::Components=main \
  -o "APT::FTPArchive::Release::Description=AlpIRC, client IRC à l'allure des années 2000" \
  release dists/stable > dists/stable/Release.tmp
mv dists/stable/Release.tmp dists/stable/Release
SIGN=(gpg --batch --yes --pinentry-mode loopback --passphrase "$APT_SIGNING_PASSPHRASE")
"${SIGN[@]}" --clearsign -o dists/stable/InRelease dists/stable/Release
"${SIGN[@]}" --armor --detach-sign -o dists/stable/Release.gpg dists/stable/Release
cp "$ICI/packaging/linux/alpirc-archive-keyring.gpg" "$ICI/packaging/linux/alpirc-archive-keyring.asc" .
VERSION="$(awk '/^Package: alpirc$/ {p=1} p && /^Version: / {print $2; exit}' dists/stable/main/binary-amd64/Packages)"
# Page d'accueil du site : comment installer.
sed -e "s/{VERSION}/$VERSION/g" "$ICI/packaging/linux/depot-index.html" > "$SITE/index.html"
cp "$SITE/index.html" "$SITE/apt/index.html"
touch "$SITE/.nojekyll"
echo "Dépôt APT prêt : $SITE (AlpIRC $VERSION$([ ${#RELAIS[@]} -gt 0 ] && echo ', avec le relais'))"
