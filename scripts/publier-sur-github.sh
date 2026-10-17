#!/bin/bash
# Publie AlpIRC sur GitHub (ADR 0011) : crée le dépôt public (la première fois), range les clés de signature (Android,
# dépôt APT) dans les secrets du dépôt, active GitHub Pages (dépôt APT pour Linux), envoie le code et les
# étiquettes. L'envoi d'une nouvelle étiquette vX.Y.Z lance le flux « Publication », qui fabrique les fichiers de la
# version (.dmg, .apk, .tar.gz, .deb) et sa page sur GitHub.
#
# Fait pour la VM Linux du Mac de Guillaume, où gh se connecte par un code (aucun jeton à copier), mais marche
# partout où il y a bash, git et gh (ou de quoi l'installer).
#
# Usage : bash publier-sur-github.sh             publier (code, étiquettes, version en cours)
#         bash publier-sur-github.sh --notes     seulement envoyer le code et réécrire les notes (français + anglais)
#                                                de toutes les versions déjà publiées, sans rien fabriquer
#
# Où sont les choses :
#   - le code : un clone d'AlpIRC (celui qui contient ce script) ou, si ce script est posé à côté de alpirc.bundle
#     (copie de secours), un clone fait depuis ce bundle dans ./alpirc ;
#   - les clés : $ALPIRC_CLES (défaut : ~/perso/alpirc-mac, sinon le dossier du script), avec signature-android/
#     et signature-apt/ (voir scripts/creer-cles.sh).
set -euo pipefail
ICI="$(cd "$(dirname "$0")" && pwd)"
DEPOT="alpirc"
MODE="publier"
if [ "${1:-}" = "--notes" ]; then MODE="notes"; fi
CLES="${ALPIRC_CLES:-$HOME/perso/alpirc-mac}"
[ -d "$CLES" ] || CLES="$ICI"
ouvrir() { (command -v xdg-open >/dev/null && xdg-open "$1" >/dev/null 2>&1) || (command -v open >/dev/null && open "$1" >/dev/null 2>&1) || true; }

echo "== 1/7 Outil GitHub (gh)"
GH=""
if command -v gh >/dev/null 2>&1; then
  GH="gh"
elif [ -x "$ICI/.outils/gh/bin/gh" ]; then
  GH="$ICI/.outils/gh/bin/gh"
else
  case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) MOTIF="_linux_amd64\.tar\.gz" ;;
    Linux-aarch64 | Linux-arm64) MOTIF="_linux_arm64\.tar\.gz" ;;
    Darwin-arm64) MOTIF="_macOS_arm64\.zip" ;;
    Darwin-*) MOTIF="_macOS_amd64\.zip" ;;
    *) MOTIF="" ;;
  esac
  URL=""
  if [ -n "$MOTIF" ]; then
    URL="$(curl -fsSL https://api.github.com/repos/cli/cli/releases/latest | grep -o "https://[^\"]*${MOTIF}" | head -1 || true)"
  fi
  if [ -z "$URL" ]; then
    echo "Impossible de télécharger gh. L'installer (https://cli.github.com, ou sudo apt install gh) puis relancer."
    exit 1
  fi
  rm -rf "$ICI/.outils/gh" && mkdir -p "$ICI/.outils/gh"
  case "$URL" in
    *.tar.gz) curl -fsSL "$URL" | tar -xz -C "$ICI/.outils/gh" --strip-components=1 ;;
    *) curl -fsSL "$URL" -o "$ICI/.outils/gh.zip" && unzip -q "$ICI/.outils/gh.zip" -d "$ICI/.outils/gh.tmp" &&
       mv "$ICI/.outils/gh.tmp"/*/* "$ICI/.outils/gh/" && rm -rf "$ICI/.outils/gh.tmp" "$ICI/.outils/gh.zip" ;;
  esac
  GH="$ICI/.outils/gh/bin/gh"
fi
echo "gh : $("$GH" --version | head -1)"

echo "== 2/7 Connexion à GitHub"
if ! "$GH" auth status -h github.com >/dev/null 2>&1; then
  echo "gh va afficher un code de 8 caractères : ouvre https://github.com/login/device sur le Mac,"
  echo "colle le code, puis clique sur « Authorize »."
  "$GH" auth login -h github.com -p https -w -s workflow
elif ! "$GH" auth status -h github.com 2>&1 | grep -q "workflow"; then
  # Il faut le droit « workflow » pour envoyer les fichiers de compilation automatique (.github/workflows).
  "$GH" auth refresh -h github.com -s workflow
fi
"$GH" auth setup-git -h github.com
MOI="$("$GH" api user -q .login)"
echo "Connecté en tant que $MOI"

echo "== 3/7 Code source"
if [ -f "$ICI/alpirc.bundle" ]; then
  if [ -d "$ICI/alpirc/.git" ]; then
    git -C "$ICI/alpirc" fetch -q "$ICI/alpirc.bundle" main
    git -C "$ICI/alpirc" checkout -q -f -B main FETCH_HEAD
  else
    git clone -q -b main "$ICI/alpirc.bundle" "$ICI/alpirc"
  fi
  git -C "$ICI/alpirc" fetch -q "$ICI/alpirc.bundle" 'refs/tags/*:refs/tags/*'
  CODE="$ICI/alpirc"
elif git -C "$ICI/.." rev-parse --show-toplevel >/dev/null 2>&1; then
  CODE="$(git -C "$ICI/.." rev-parse --show-toplevel)"
else
  echo "Ni alpirc.bundle à côté de ce script, ni dépôt git autour : rien à publier."
  exit 1
fi
VERSION="$(cat "$CODE/VERSION")"
TAG="v$VERSION"
BRANCHE="$(git -C "$CODE" rev-parse --abbrev-ref HEAD)"
if [ "$BRANCHE" != main ]; then
  echo "$CODE est sur la branche « $BRANCHE » : se placer sur main (git checkout main) puis relancer."
  exit 1
fi
echo "AlpIRC $VERSION ($CODE)"

echo "== 4/7 Dépôt https://github.com/$MOI/$DEPOT"
if ! "$GH" repo view "$MOI/$DEPOT" >/dev/null 2>&1; then
  "$GH" repo create "$MOI/$DEPOT" --public \
    --description "Client IRC à l'allure des années 2000, refait en Rust : Mac, Android, Linux" \
    --homepage "https://github.com/$MOI/$DEPOT/releases/latest"
fi

if [ "$MODE" = "publier" ]; then
  # Avant d'envoyer les étiquettes : l'envoi d'une étiquette lance le flux « Publication », qui a besoin des clés.
  echo "== 5/7 Clés de signature et GitHub Pages (clés lues dans $CLES)"
  CLE="$CLES/signature-android/alpirc-release.p12"
  MDP="$CLES/signature-android/mot-de-passe.txt"
  if [ -f "$CLE" ] && [ -f "$MDP" ]; then
    base64 < "$CLE" | tr -d '\n' | "$GH" secret set ANDROID_KEYSTORE_B64 -R "$MOI/$DEPOT"
    printf %s "$(cat "$MDP")" | "$GH" secret set ANDROID_KEYSTORE_PASSWORD -R "$MOI/$DEPOT"
    echo "Clé Android rangée dans les secrets du dépôt (GitHub signera les APK avec elle)."
  else
    echo "Pas de $CLES/signature-android : secrets Android inchangés (scripts/creer-cles.sh les crée)."
  fi
  CLE="$CLES/signature-apt/alpirc-apt-cle-privee.asc"
  MDP="$CLES/signature-apt/mot-de-passe.txt"
  if [ -f "$CLE" ] && [ -f "$MDP" ]; then
    "$GH" secret set APT_SIGNING_KEY -R "$MOI/$DEPOT" < "$CLE"
    printf %s "$(cat "$MDP")" | "$GH" secret set APT_SIGNING_PASSPHRASE -R "$MOI/$DEPOT"
    echo "Clé du dépôt APT rangée dans les secrets du dépôt."
  else
    echo "Pas de $CLES/signature-apt : secrets APT inchangés (scripts/creer-cles.sh les crée)."
  fi
  # GitHub Pages publié par le flux « Publication » (et non depuis une branche).
  if "$GH" api "repos/$MOI/$DEPOT/pages" >/dev/null 2>&1; then
    "$GH" api -X PUT "repos/$MOI/$DEPOT/pages" -f build_type=workflow >/dev/null
  else
    "$GH" api -X POST "repos/$MOI/$DEPOT/pages" -f build_type=workflow >/dev/null
  fi
  # Le site est déployé depuis les étiquettes vX.Y.Z : il faut les autoriser dans l'environnement « github-pages ».
  printf '{"deployment_branch_policy":{"protected_branches":false,"custom_branch_policies":true}}' |
    "$GH" api -X PUT "repos/$MOI/$DEPOT/environments/github-pages" --input - >/dev/null
  "$GH" api -X POST "repos/$MOI/$DEPOT/environments/github-pages/deployment-branch-policies" \
    -f name=main -f type=branch >/dev/null 2>&1 || true
  "$GH" api -X POST "repos/$MOI/$DEPOT/environments/github-pages/deployment-branch-policies" \
    -f name='v*' -f type=tag >/dev/null 2>&1 || true
  echo "GitHub Pages activé : https://$MOI.github.io/$DEPOT/"
fi

echo "== 6/7 Envoi du code et des étiquettes"
URL_DEPOT="https://github.com/$MOI/$DEPOT.git"
if git -C "$CODE" remote get-url origin >/dev/null 2>&1; then
  git -C "$CODE" remote set-url origin "$URL_DEPOT"
else
  git -C "$CODE" remote add origin "$URL_DEPOT"
fi
# L'étiquette de la version était-elle déjà sur GitHub ? (Si oui, son envoi ne relance pas la publication.)
DEJA=0
if git -C "$CODE" ls-remote --exit-code --tags origin "refs/tags/$TAG" >/dev/null 2>&1; then DEJA=1; fi
git -C "$CODE" push -u origin main
git -C "$CODE" push origin --tags

if [ "$MODE" = "notes" ]; then
  echo "== Notes des versions déjà publiées (français + anglais)"
  NOTES="$(mktemp)"
  for T in $("$GH" release list -R "$MOI/$DEPOT" --limit 200 --json tagName -q '.[].tagName'); do
    if bash "$CODE/scripts/notes-de-version.sh" "${T#v}" "$CODE" > "$NOTES" 2>/dev/null; then
      "$GH" release edit "$T" -R "$MOI/$DEPOT" --notes-file "$NOTES" >/dev/null
      echo "  $T : notes réécrites"
    else
      echo "  $T : pas de section dans le CHANGELOG, notes inchangées"
    fi
  done
  rm -f "$NOTES"
  echo
  echo "Terminé ! Aucun fichier fabriqué ni remplacé : https://github.com/$MOI/$DEPOT/releases"
  exit 0
fi

echo "== 7/7 Version $VERSION"
if ! git -C "$CODE" rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
  echo "Pas d'étiquette $TAG : seul le code est envoyé (pour publier : git tag $TAG && relancer ce script)."
elif "$GH" release view "$TAG" -R "$MOI/$DEPOT" >/dev/null 2>&1; then
  # Une version publiée ne change jamais : rien à refaire.
  echo "La version $VERSION est déjà publiée : rien à refaire."
elif [ "$DEJA" = 1 ]; then
  # L'étiquette était déjà là mais la version n'a pas abouti : on relance sa fabrication.
  "$GH" workflow run release.yml -R "$MOI/$DEPOT" -f tag="$TAG"
  echo "Fabrication de la version $VERSION relancée sur GitHub (environ 20 minutes)."
else
  echo "L'étiquette $TAG vient d'être envoyée : GitHub fabrique les fichiers de la version (environ 20 minutes)."
fi

echo
echo "Terminé !"
echo "  Le code :              https://github.com/$MOI/$DEPOT"
echo "  Les téléchargements :  https://github.com/$MOI/$DEPOT/releases/latest"
echo "  La compilation auto :  https://github.com/$MOI/$DEPOT/actions"
echo "  Le dépôt APT (Linux) : https://$MOI.github.io/$DEPOT/ (mis à jour par la compilation auto)"
ouvrir "https://github.com/$MOI/$DEPOT/actions"
