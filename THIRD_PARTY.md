# Tiers

AlpIRC est une réimplémentation propre (voir [ADR 0001](docs/adr/0001-reimplementation-propre.md)) : aucun code,
aucune image, aucun texte de mIRC n'y figure, et aucune ligne n'est reprise d'un client IRC sous licence copyleft.
Voici ce qui vient d'ailleurs.

## Polices

### Fixedsys Excelsior 3.00 — domaine public

Police des discussions (chasse fixe, proche du Fixedsys de Windows que mIRC utilisait par défaut) :
`assets/fonts/FSEX300.ttf`, par Darien Valentine (version 3.010 du fichier, 2007,
<http://www.fixedsysexcelsior.com/>). L'auteur l'a placée dans le domaine public ; voir aussi la déclaration
reprise sur <https://github.com/kika/fixedsys>. Aucune obligation d'attribution ; nous la citons quand même.

### Pixel Operator — CC0 1.0

Police des menus et des boîtes de dialogue (dans l'esprit de MS Sans Serif) : `assets/fonts/PixelOperator.ttf` et
`assets/fonts/PixelOperator-Bold.ttf`, par Jayvee Enaguas (HarvettFox96), 2009-2018, sous Creative Commons Zero
1.0 Universal : texte complet dans `assets/fonts/PixelOperator-LICENSE.txt`.

## Icônes et images

Les icônes d'AlpIRC (application, barre d'outils, fenêtres) sont des œuvres originales en pixel-art, dessinées
pour AlpIRC (`assets/icons/src/draw_icons.py`) ; elles ne reprennent ni le logo ni les icônes de mIRC.

## Valeurs de protocole

Les palettes de couleurs de mIRC (16 couleurs, puis 99) et les codes de mise en forme (Ctrl+K, Ctrl+B…) sont des
conventions de fait du protocole IRC, documentées publiquement ; elles sont reprises pour que les messages
s'affichent comme chez les autres (ADR 0001). Le langage de script mSL est réimplémenté d'après sa documentation
publique (ADR 0007).

## Crates Rust

AlpIRC est construit avec des bibliothèques Rust libres (licences MIT, Apache-2.0, BSD, ISC, Zlib, Unicode,
CDLA-Permissive pour les racines de confiance de Mozilla…). La liste à jour des crates compilées dans l'appli de
bureau, l'appli Android et le relais, avec la licence de chacune, s'obtient par :

```sh
cargo xtask licenses > licences.md
```

(Elle suit `Cargo.lock` : dépendances normales et de compilation, toutes plateformes confondues.)

Une seule crate est sous licence à réciprocité par fichier : `option-ext` (MPL-2.0, tirée par `directories`),
utilisée sans modification ; son code source est celui publié sur crates.io. Quelques crates proposent une licence
GPL/LGPL **au choix** avec MIT ou Apache-2.0 (`r-efi`, `self_cell`) : AlpIRC les utilise sous MIT/Apache-2.0.

## Marques

« mIRC » est une marque de mIRC Co. Ltd. AlpIRC n'est ni affilié à mIRC Co. Ltd., ni approuvé par elle ; mIRC est
cité pour décrire l'inspiration et la compatibilité des scripts. « Undernet », « Libera.Chat » et les autres noms
de réseaux appartiennent à leurs propriétaires.
