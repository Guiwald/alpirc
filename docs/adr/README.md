# Décisions d'architecture (ADR)

Chaque décision structurante a sa fiche : contexte, décision, conséquences. Une décision remplacée n'est pas
effacée : sa fiche passe au statut « remplacée par ADR n » et reste là. Les treize premières sont issues du
grilling par lots du 17/10/2026.

| N° | Décision | Statut |
|---|---|---|
| [0001](0001-reimplementation-propre.md) | Réimplémentation propre, sans rien emprunter à mIRC | acceptée |
| [0002](0002-plateformes-et-livrable.md) | Plateformes (macOS, Android, Linux) et définition du « livrable » | acceptée |
| [0003](0003-tout-en-rust.md) | Tout en Rust, Java minimal sur Android | acceptée |
| [0004](0004-perimetre-1-0.md) | Périmètre de la 1.0 (et ce qui vient après) | acceptée |
| [0005](0005-allure.md) | Allure : mIRC 5.x/6.x fidèle, avec des thèmes | acceptée |
| [0006](0006-android.md) | Android : disposition adaptée, saisie native, connexion permanente | acceptée |
| [0007](0007-scripts-msl.md) | Scripts mSL : l'essentiel et les dialogues, en bac à sable | acceptée |
| [0008](0008-boite-a-outils.md) | Interface : notre propre boîte à outils (winit, softbuffer, cosmic-text) | acceptée |
| [0009](0009-relais.md) | Le relais d'AlpIRC (bouncer), écrit dans la 1.0, mis en service plus tard | acceptée |
| [0010](0010-undernet-et-reseaux.md) | Undernet et réseaux par défaut, compte X | acceptée |
| [0011](0011-depot-ci-environnement.md) | Dépôt public, CI, publication et environnement de travail | acceptée |
| [0012](0012-langues-et-encodages.md) | Langues de l'interface et encodages | acceptée |
| [0013](0013-nom-alpirc.md) | Le client s'appelle AlpIRC | acceptée |
