# Icônes d'AlpIRC

Toutes les icônes d'AlpIRC sont du pixel-art **original**, dessiné pour AlpIRC. Rien n'est repris de
mIRC (logo, icônes, barre d'outils), des icônes de Windows ni d'un autre jeu d'icônes. Le style
rappelle Windows 98 : palette de 16 couleurs, contour noir, lumière venant d'en haut à gauche.

Chaque icône est écrite **en texte** dans `src/draw_icons.py`, sous forme de grille de caractères
(un caractère = un pixel). On peut donc relire et corriger une icône directement dans le code, puis
régénérer les PNG.

## Régénérer

Depuis la racine du dépôt (Python 3 et Pillow) :

```sh
python3 assets/icons/src/draw_icons.py
```

Le script vérifie la taille et les caractères de chaque grille, puis réécrit tous les PNG et la
planche de contrôle `preview.png`.

## Palette

| Car. | Couleur | | Car. | Couleur |
|---|---|---|---|---|
| `.` | transparent | | `g` | vert `008000` |
| `k` | noir `000000` | | `G` | vert vif `00FF00` |
| `w` | blanc `FFFFFF` | | `m` | bordeaux `800000` |
| `l` | gris clair `C0C0C0` | | `r` | rouge `FF0000` |
| `d` | gris foncé `808080` | | `o` | olive `808000` |
| `n` | marine `000080` | | `y` | jaune `FFFF00` |
| `b` | bleu `0000FF` | | `p` | violet `800080` |
| `t` | sarcelle `008080` | | `M` | magenta `FF00FF` |
| `c` | cyan `00FFFF` | | `s` | ombre portée (noir à 31 %) |

L'icône de l'application ajoute quelques teintes (`APP_PALETTE`) : bleus des montagnes (`B`, `N`,
`L`, `Q`), neige à l'ombre (`i`), sarcelles des collines (`C`, `T`).

## Icône de l'application (`app/`)

Trois sommets enneigés (les Alpes, clin d'œil aux canaux d'Undernet de Lyon, Grenoble et Chambéry)
derrière une bulle de dialogue blanche. Les trois points de la bulle sont rouge, vert et bleu, comme
les codes couleur d'IRC. Collines sarcelle au pied des montagnes.

| Fichier | Contenu |
|---|---|
| `alpirc-32.png` | dessin maître 32×32, aussi icône de fenêtre |
| `alpirc-16.png` | version 16×16 redessinée (pas une simple réduction) |
| `alpirc-1024.png` | macOS : le 32×32 agrandi ×26 au plus proche voisin (832 px, ~81 % du canevas), centré sur fond transparent |
| `alpirc-1024-android-fg.png` | Android, avant-plan de l'icône adaptative : agrandi ×14 (448 px), tout le dessin tient dans la zone sûre (disque de 66 dp sur 108 dp), même avec un masque rond |
| `alpirc-1024-android-bg.png` | Android, arrière-plan : sarcelle `008080` uni, le bureau de Windows 98 |

Les grandes versions sont centrées sur la boîte englobante du dessin (centrage optique), pas sur le
canevas 32×32.

## Barre d'outils (`toolbar/`, 16×16)

| Nom | Dessin |
|---|---|
| `connect` | prise et fiche emboîtées, avec leurs cordons |
| `disconnect` | fiche débranchée, broches jaunes visibles |
| `options` | marteau et clé plate croisés |
| `abook` | carnet d'adresses à spirale, onglets de couleur, silhouette |
| `favorites` | dossier jaune marqué d'un `#` (dossier de canaux) |
| `chanlist` | feuille avec un grand `#` et une liste |
| `notify` | cloche |
| `urls` | globe (l'Europe) et maillons de chaîne |
| `dcc_send` | feuille et flèche verte sortante |
| `dcc_chat` | deux bulles, blanche et jaune |
| `transfers` | flèche verte vers le haut, flèche bleue vers le bas |
| `scripts` | parchemin et stylo rouge |
| `colors` | palette de peintre |
| `cascade` | trois fenêtres en cascade |
| `tile_h` | deux fenêtres côte à côte (gauche, droite) |
| `tile_v` | deux fenêtres l'une sous l'autre |
| `find` | loupe |
| `away` | croissant de lune et étoiles |
| `about` | bulle avec un `?` |
| `help` | livre bleu avec un `?` jaune |
| `log` | bloc-notes à spirale |
| `relay` | montagne surmontée d'une antenne-relais qui émet |

## Types de fenêtres (`window/`, 16×16)

| Nom | Dessin |
|---|---|
| `status` | petit écran à antenne (sans éclair) |
| `network_connected` | `status` avec une pastille verte, pour la treebar |
| `network_disconnected` | `status` avec une pastille rouge |
| `channel` | `#` dans une bulle |
| `query` | une personne et sa bulle |
| `dcc_chat` | deux bulles reliées |
| `custom` | fenêtre marquée d'un `@` |
| `chanlist`, `notify`, `urls`, `transfers` | mêmes dessins que les boutons de la barre d'outils |
| `script_editor` | feuille de code colorée et crayon |

## Glyphes d'interface (`ui/`)

| Nom | Taille | Dessin |
|---|---|---|
| `check`, `radio_dot` | 16×16 | coche et puce des menus |
| `arrow_right`, `arrow_left`, `arrow_up`, `arrow_down` | 16×16 | flèches pleines (sous-menus, barres de défilement) |
| `close_x`, `minimize`, `maximize`, `restore` | 16×16 | glyphes des boutons de légende |
| `folder_closed`, `folder_open` | 16×16 | dossiers |
| `warning`, `info`, `error` | 16×16 | triangle jaune `!`, disque bleu `i`, octogone rouge à croix |
| `msg_warning_32`, `msg_info_32`, `msg_error_32`, `msg_question_32` | 32×32 | les mêmes en grand pour les boîtes de message, avec ombre portée ; question : disque sarcelle `?` |

Les glyphes de légende sont placés comme sur un bouton Windows 98 de **16×14** : les lignes 0 et 15
sont vides. On garde les lignes 1 à 14 (ou on dessine l'image une ligne plus haut) pour obtenir le
glyphe exact du bouton.

## Planche de contrôle

`preview.png` montre toutes les icônes à ×1 et ×3 sur fond gris `C0C0C0`, l'icône de l'application à
×8 avec ses aperçus macOS et Android, une barre d'outils façon Windows 98 (×1 et ×2), les icônes de
fenêtre sur des barres de titre active et inactive, et les boutons de légende. Les étiquettes
utilisent la police Pixel Operator de `assets/fonts/` (licence dans ce dossier).

## Licence

Œuvres originales créées pour AlpIRC, publiées sous **CC0 1.0 et MIT** : la licence MIT du projet
(voir `LICENSE` à la racine) et, pour plus de commodité, la dédicace au domaine public CC0 1.0
(<https://creativecommons.org/publicdomain/zero/1.0/>). Chacun peut les réutiliser librement, sous
l'une ou l'autre.
