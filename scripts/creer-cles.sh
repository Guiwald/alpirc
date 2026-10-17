#!/bin/bash
# Crée, une seule fois, les deux clés de signature d'AlpIRC (ADR 0011) :
#   - la clé Android (signature des APK) : <dossier>/signature-android/alpirc-release.p12 et mot-de-passe.txt ;
#   - la clé du dépôt APT (Linux)      : <dossier>/signature-apt/alpirc-apt-cle-privee.asc et mot-de-passe.txt,
#     et sa partie publique, à versionner : packaging/linux/alpirc-archive-keyring.gpg et .asc.
# Une clé qui existe déjà n'est jamais remplacée (perdre la clé Android empêche de mettre l'appli à jour).
#
# Usage : bash scripts/creer-cles.sh [dossier des clés]      (défaut : ~/perso/alpirc-mac)
# Ensuite : garder une copie de secours du dossier, versionner la clé publique, puis lancer publier-sur-github.sh
# (il range les clés dans les secrets du dépôt GitHub). Il faut keytool (JDK) et gpg.
set -euo pipefail
ICI="$(cd "$(dirname "$0")/.." && pwd)"
DOSSIER="${1:-$HOME/perso/alpirc-mac}"
mkdir -p "$DOSSIER"
DOSSIER="$(cd "$DOSSIER" && pwd)"
umask 077

mot_de_passe() { head -c 48 /dev/urandom | base64 | tr -d '/+=\n' | head -c 32; }

echo "== Clé Android"
A="$DOSSIER/signature-android"
if [ -f "$A/alpirc-release.p12" ]; then
  echo "$A/alpirc-release.p12 existe déjà : inchangée."
else
  command -v keytool >/dev/null || { echo "keytool manque : installer un JDK (sudo apt install openjdk-17-jdk-headless)" >&2; exit 1; }
  mkdir -p "$A"
  ALPIRC_MDP="$(mot_de_passe)"
  export ALPIRC_MDP
  keytool -genkeypair -keystore "$A/alpirc-release.p12" -storetype PKCS12 \
    -storepass:env ALPIRC_MDP -keypass:env ALPIRC_MDP -alias alpirc \
    -keyalg RSA -keysize 4096 -validity 10000 -dname "CN=AlpIRC, O=Guiwald" >/dev/null
  printf %s "$ALPIRC_MDP" > "$A/mot-de-passe.txt"
  unset ALPIRC_MDP
  echo "Créée : $A/alpirc-release.p12 (mot de passe dans mot-de-passe.txt)"
fi

echo "== Clé du dépôt APT"
P="$DOSSIER/signature-apt"
PUB_GPG="$ICI/packaging/linux/alpirc-archive-keyring.gpg"
PUB_ASC="$ICI/packaging/linux/alpirc-archive-keyring.asc"
if [ -f "$P/alpirc-apt-cle-privee.asc" ]; then
  echo "$P/alpirc-apt-cle-privee.asc existe déjà : inchangée."
else
  command -v gpg >/dev/null || { echo "gpg manque : sudo apt install gnupg" >&2; exit 1; }
  mkdir -p "$P"
  MDP="$(mot_de_passe)"
  GNUPGHOME="$(mktemp -d)"
  export GNUPGHOME
  trap 'rm -rf "$GNUPGHOME"' EXIT
  printf %s "$MDP" | gpg --batch --pinentry-mode loopback --passphrase-fd 0 \
    --quick-generate-key "AlpIRC APT repository <guiwald@users.noreply.github.com>" rsa4096 sign never 2>/dev/null
  FPR="$(gpg --batch --list-secret-keys --with-colons | awk -F: '/^fpr:/ {print $10; exit}')"
  printf %s "$MDP" | gpg --batch --pinentry-mode loopback --passphrase-fd 0 --armor --export-secret-keys "$FPR" > "$P/alpirc-apt-cle-privee.asc"
  printf %s "$MDP" > "$P/mot-de-passe.txt"
  gpg --batch --export "$FPR" > "$P/alpirc-archive-keyring.gpg"
  gpg --batch --armor --export "$FPR" > "$P/alpirc-archive-keyring.asc"
  echo "Créée : $P/alpirc-apt-cle-privee.asc (empreinte $FPR)"
fi
# La partie publique va dans le dépôt (le .deb l'installe, le site APT la publie).
umask 022
if [ -f "$P/alpirc-archive-keyring.gpg" ]; then
  if [ ! -f "$PUB_GPG" ]; then
    cp "$P/alpirc-archive-keyring.gpg" "$PUB_GPG"
    cp "$P/alpirc-archive-keyring.asc" "$PUB_ASC"
    chmod 644 "$PUB_GPG" "$PUB_ASC"
    echo "Clé publique copiée dans packaging/linux/ : à versionner (git add packaging/linux/alpirc-archive-keyring.*)."
  elif ! cmp -s "$P/alpirc-archive-keyring.gpg" "$PUB_GPG"; then
    echo "Attention : $PUB_GPG ne correspond pas à la clé de $P." >&2
  fi
fi
echo
echo "Terminé. Faire une copie de secours de $DOSSIER (sans ces clés, plus de mises à jour signées)."
