#!/bin/bash
# Notes d'une version pour sa page GitHub : la section de CHANGELOG.md (français), puis celle de CHANGELOG.en.md
# (anglais), puis scripts/notes-installation.md (français et anglais), où X.Y.Z devient le numéro de la version.
#
# Usage : bash scripts/notes-de-version.sh 0.2.0 [dossier du dépôt]
set -euo pipefail
V="${1:?usage : notes-de-version.sh X.Y.Z [dossier du dépôt]}"
RACINE="${2:-$(cd "$(dirname "$0")/.." && pwd)}"

# La section « ## [V] … » d'un journal, sans son titre ni les lignes vides du début.
section() {
  [ -f "$1" ] || return 0
  awk -v v="$V" 'index($0, "## [" v "]") == 1 {f=1; next} /^## \[/ {if (f) exit} f' "$1" | sed '/./,$!d'
}

FR="$(section "$RACINE/CHANGELOG.md")"
EN="$(section "$RACINE/CHANGELOG.en.md")"
if [ -z "$FR" ]; then
  echo "Pas de section [$V] dans CHANGELOG.md" >&2
  exit 1
fi

if [ -n "$EN" ]; then
  printf '*English below.*\n\n'
fi
printf '%s\n' "$FR"
if [ -n "$EN" ]; then
  printf '\n---\n\n## English\n\n%s\n' "$EN"
fi
sed "s/X\.Y\.Z/$V/g" "$RACINE/scripts/notes-installation.md"
