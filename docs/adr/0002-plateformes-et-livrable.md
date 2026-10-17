# ADR 0002 — Plateformes et définition du « livrable »

- Statut : acceptée (17/10/2026, grilling par lots, Q5, Q12)

## Décision

| Plateforme | Livrable | Essais |
|---|---|---|
| **macOS** (Apple Silicon et Intel) | `AlpIRC-X.Y.Z.dmg` (universel) | Claude pilote l'appli sur le Mac de Guillaume (captures, clics, saisie) |
| **Android** (Pixel 8, Android 8 et plus) | `AlpIRC-X.Y.Z.apk` | simulateur de téléphone sur ordinateur, émulateur d'Android Studio sur le Mac, puis recette sur le Pixel 8 |
| **Linux** (x86_64 ; Ubuntu 22.04+, Debian 12+, Fedora 36+) | `AlpIRC-X.Y.Z-linux-x86_64.tar.gz`, `alpirc_X.Y.Z_amd64.deb`, dépôt APT | Claude sur écran virtuel (Xvfb) |
| Windows, iPhone | plus tard | — |

- Le code reste portable vers Windows et iOS : aucun choix qui les exclurait.
- **Versions 0.x publiées au fil de l'eau** (GitHub Releases), sans attendre Guillaume entre deux jalons.
- **La 1.0.0 est livrable** quand toutes les décisions du grilling du 17/10/2026 sont réalisées, que les tests
  automatiques et la CI sont verts, et que Claude l'a essayée sur Mac et sur Android (simulateur, émulateur).
  Reste ensuite la recette de Guillaume sur le Pixel 8.

## Conséquences

- Pas de publication dans les stores ; installation à la main (DMG, APK, `.deb`/APT).
- Pas de certificat payant : sur Mac, premier lancement par Réglages Système › Confidentialité et sécurité ›
  « Ouvrir quand même ».
