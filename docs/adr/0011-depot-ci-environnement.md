# ADR 0011 — Dépôt public, CI, publication et environnement de travail

- Statut : acceptée (17/10/2026, grilling par lots, Q9 ; environnement révisé le même jour)

## Décision

- **Dépôt public** `Guiwald/alpirc`, licence **MIT**. Aucun secret versionné ; commits à l'adresse « noreply » de
  GitHub.
- **CI GitHub Actions** : tests Linux, compilation macOS, Linux et Android à chaque envoi ; flux « Publication » sur
  l'étiquette `vX.Y.Z` qui fabrique `.dmg`, `.apk`, `.tar.gz`, `.deb` et le dépôt APT.
- **Convention de versions** du projet Outils : `VERSION`, `CHANGELOG.md` (+ `CHANGELOG.en.md`), SemVer, versionCode
  Android dérivé, page des versions en artefact (`docs/versions.md`).
- **Environnement** :
  - Claude développe et teste dans son environnement cloud (Linux) ;
  - le code est poussé depuis la VM Linux du Mac de Guillaume, où `gh` est connecté par un code (jeton propre à la
    session, révocable) — le Terminal du Mac ne peut pas être piloté au clavier par Claude ;
  - le Mac sert aux essais de l'appli (pilotée par Claude : captures, clics, saisie) et de l'émulateur Android ;
  - copie de secours `~/perso/alpirc-mac/alpirc.bundle` et clés de signature dans `~/perso/alpirc-mac/`.

## Conséquences

- Les builds macOS se font sur GitHub Actions (pas de Xcode sur le Mac de Guillaume).
- Les clés de signature (Android, APT) vont dans les secrets du dépôt ; copie de secours chez Guillaume.
