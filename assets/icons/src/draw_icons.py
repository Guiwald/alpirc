#!/usr/bin/env python3
"""Icônes d'AlpIRC, dessinées en pixel-art sous forme de grilles de caractères.

Chaque icône est une liste de chaînes : un caractère = un pixel, selon la palette
ci-dessous. Lancer depuis la racine du dépôt :

    python3 assets/icons/src/draw_icons.py

Le script écrit les PNG dans assets/icons/{app,toolbar,window,ui}/ et la planche
de contrôle assets/icons/preview.png. Œuvres originales pour AlpIRC
(CC0 1.0 et MIT).
"""
from __future__ import annotations

import math
import os
import sys

from PIL import Image, ImageDraw, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))
ICONS_DIR = os.path.dirname(HERE)
REPO = os.path.dirname(os.path.dirname(ICONS_DIR))

# ---------------------------------------------------------------------------
# Palette « Windows 98 », 16 couleurs + transparence
# ---------------------------------------------------------------------------
PALETTE = {
    ".": (0, 0, 0, 0),          # transparent
    "k": (0, 0, 0, 255),        # noir
    "w": (255, 255, 255, 255),  # blanc
    "l": (192, 192, 192, 255),  # gris clair C0C0C0
    "d": (128, 128, 128, 255),  # gris foncé 808080
    "n": (0, 0, 128, 255),      # marine 000080
    "b": (0, 0, 255, 255),      # bleu 0000FF
    "t": (0, 128, 128, 255),    # sarcelle 008080
    "c": (0, 255, 255, 255),    # cyan 00FFFF
    "g": (0, 128, 0, 255),      # vert 008000
    "G": (0, 255, 0, 255),      # vert vif 00FF00
    "m": (128, 0, 0, 255),      # bordeaux 800000
    "r": (255, 0, 0, 255),      # rouge FF0000
    "o": (128, 128, 0, 255),    # olive 808000
    "y": (255, 255, 0, 255),    # jaune FFFF00
    "p": (128, 0, 128, 255),    # violet 800080
    "M": (255, 0, 255, 255),    # magenta FF00FF
    "s": (0, 0, 0, 80),         # ombre portée translucide
}

# Palette un peu plus riche, réservée à l'icône de l'application.
APP_PALETTE = dict(PALETTE)
APP_PALETTE.update({
    "B": (56, 96, 200, 255),    # face éclairée de la montagne
    "N": (16, 32, 120, 255),    # face à l'ombre
    "i": (176, 200, 236, 255),  # neige à l'ombre
    "T": (0, 88, 88, 255),      # sarcelle sombre (collines à l'ombre)
    "C": (64, 176, 168, 255),   # sarcelle claire (crête des collines)
    "L": (150, 175, 235, 255),  # sommets du fond, face éclairée
    "Q": (100, 130, 215, 255),  # sommets du fond, face à l'ombre
})

ICONS: dict[str, tuple[list[str], dict]] = {}


def icon(path: str, grid: list[str], palette: dict | None = None) -> None:
    ICONS[path] = (grid, palette or PALETTE)


# ===========================================================================
# 1. Icône de l'application
# ===========================================================================
# Trois sommets enneigés (les Alpes) derrière une bulle de dialogue blanche ;
# ses trois points sont rouge, vert et bleu, clin d'œil aux codes couleur
# d'IRC. Collines sarcelle au pied des montagnes.

icon("app/alpirc-32", [
    "................................",  # 0
    ".................k..............",  # 1
    "................kwk.............",  # 2
    "................kwik............",  # 3
    "...............kwwiik...........",  # 4
    "..............kwwwwiik..........",  # 5
    "......k......kwwwwwiiik....k....",  # 6
    ".....kwk.....kwwBwwiiNik..kwk...",  # 7
    "....kwwik...kBwBBwBiNNik.kwwik..",  # 8
    "...kwwLwQk.kBBBBBBBwNNNNkwLwwQk.",  # 9
    "..kLLLLwQQkBBBBBBBBBNNNNNNLwLQQk",  # 10
    ".kLLLLLLQQBBBBBBBBBBNNNNNNNLLLQk",  # 11
    "kLLLLLLLLBBBBBBBBBBBBNNNNNNNLLQk",  # 12
    "kLLLLLLLBBBBBBBBBBBBBNNNNNNNNLQk",  # 13
    "kLLLLLLBBBBBBBBBBBBBBNNNNNNNNNLk",  # 14
    "kLkkkkkkkkkkkkkkBBBBBNNNNNNNNNNk",  # 15
    "kkwwwwwwwwwwwwwwkBBBBNNNNNNNNNNk",  # 16
    "kwwwwwwwwwwwwwwwlkBBBBNNNNNCCCtk",  # 17
    "kwwwwwwwwwwwwwwwlkBBBBNNNCCttttk",  # 18
    "kwwrrwwwggwwwbbwlkBBBBBCCtttttTk",  # 19
    "kwwrrwwwggwwwbbwlkBBCCtttttttTTk",  # 20
    "kwwwwwwwwwwwwwwwlkCCtttttttttTTk",  # 21
    "kwwwwwwwwwwwwwwwlkttttttttttTTTk",  # 22
    "kwwwwwwwwwwwwwwwlkkkkkkkkkkkkkkk",  # 23
    ".klwwlllllllllllk...............",  # 24
    "..kwwlkkkkkkkkkk................",  # 25
    "..kwlk..........................",  # 26
    ".kwlk...........................",  # 27
    ".klk............................",  # 28
    "kk..............................",  # 29
    "................................",  # 30
    "................................",  # 31
], APP_PALETTE)

icon("app/alpirc-16", [
    "................",  # 0
    "........k.......",  # 1
    ".......kwk......",  # 2
    ".......kwik.....",  # 3
    "...k..kwwik..k..",  # 4
    "..kwkkBwwiNkkwk.",  # 5
    ".kLLQBBBwNNNNLQk",  # 6
    "kLLQBBBBBBNNNNQk",  # 7
    "kkkkkkkkkBBNNNNk",  # 8
    "kwwwwwwwlkBNNNCk",  # 9
    "kwrwgwbwlkBNCCtk",  # 10
    "kwwwwwwwlkCCttTk",  # 11
    "kllllllllkkkkkkk",  # 12
    ".kwkkkkkk.......",  # 13
    "kwk.............",  # 14
    "kk..............",  # 15
], APP_PALETTE)

# Icône de la barre d'état d'Android : silhouette blanche sur fond transparent
# (Android n'en garde que la couche alpha). Dessinée en 24×24, zone utile de
# 22×22 avec une marge de 1 px ; exportée ×4 en 96×96 (alpirc-notification.png).
NOTIFICATION = [
    "........................",
    "........................",
    "............w...........",
    "...........www..........",
    "..........wwwww.........",
    ".........wwwwwww........",
    "........wwwwwwwww.......",
    ".......wwwwwwwwwww.w....",
    "......wwwwwwwwwwwwwww...",
    ".....wwwwwwwwwwwwwwwww..",
    ".............wwwwwwwwww.",
    "..wwwwwwwwww..wwwwwwwww.",
    ".wwwwwwwwwwww.wwwwwwwww.",
    ".wwwwwwwwwwww.wwwwwwwww.",
    ".wwwwwwwwwwww.wwwwwwwww.",
    ".w..ww..ww..w...........",
    ".w..ww..ww..w...........",
    ".wwwwwwwwwwww...........",
    ".wwwwwwwwwwww...........",
    "..wwwwwwwwww............",
    "..www...................",
    "..ww....................",
    ".ww.....................",
    "........................",
]


# ===========================================================================
# 2. Barre d'outils (16×16, contour noir, lumière en haut à gauche)
# ===========================================================================

icon("toolbar/connect", [
    "................",
    "...............k",
    "..............kd",
    ".............kdk",
    "...kkk...kkkkdk.",
    "..kwwlk.kwwlkk..",
    "..kwldkykwldk...",
    "..kwldkokwldk...",
    "..kwldk.kwldk...",
    "..kwldkykwldk...",
    "..kdddkokdddk...",
    ".kdkkk...kkk....",
    "kdk.............",
    "dk..............",
    "k...............",
    "................",
])

# La fiche, débranchée, pend plus bas que la prise : broches jaunes à nu,
# trous noirs dans la prise.
icon("toolbar/disconnect", [
    "................",
    "................",
    "...............k",
    "..............kd",
    "..........kkkkdk",
    ".........kwwlkk.",
    "..kkk....kkldk..",
    ".kwwlk...kkldk..",
    ".kwldkyy.kwldk..",
    ".kwldkoo.kkldk..",
    ".kwldk...kdddk..",
    ".kwldkyy..kkk...",
    ".kdddkoo........",
    "dkkkk...........",
    "k...............",
    "................",
])

icon("toolbar/options", [
    "......kk..kk....",
    ".....kwlkkwk..k.",
    "....kwldkkwk.kwk",
    "...kwldk.kwlkwlk",
    "..kwldk.kwllllk.",
    ".kwldkokwlldkk..",
    "kldk.kyokdk.....",
    ".kk..kkyok......",
    "....kwlkyok.....",
    "...kwldkkyok....",
    "..kwldk..kyok...",
    ".kwldk....kyok..",
    "kwldk......kyok.",
    "kldk........kyok",
    ".kk..........kkk",
    "................",
])

icon("toolbar/abook", [
    "................",
    "...kkkkkkkkkk...",
    "...kcccccccck...",
    ".kwkcwwwwwwtkrk.",
    "...kcwwnnwwtkrk.",
    "...kcwwnnwwtk...",
    ".kwkcwnnnnwtkyk.",
    "...kcwnnnnwtkyk.",
    "...kcwwwwwwtk...",
    ".kwkctttttttkGk.",
    "...kctttttttkGk.",
    "...kctttttttk...",
    ".kwkctttttttkbk.",
    "...kctttttttkbk.",
    "...kkkkkkkkkk...",
    "................",
])

TB_FOLDER_HASH = [
    "................",
    "................",
    "..kkkk..........",
    ".kwyyyk.........",
    ".kwyyyykkkkkkkk.",
    ".kwyyyyyyyyyyok.",
    ".kwyyynyynyyyok.",
    ".kwyynnnnnnyyok.",
    ".kwyyynyynyyyok.",
    ".kwyyynyynyyyok.",
    ".kwyynnnnnnyyok.",
    ".kwyyynyynyyyok.",
    ".kwyyyyyyyyyyok.",
    ".kooooooooooook.",
    ".kkkkkkkkkkkkkk.",
    "................",
]
icon("toolbar/favorites", TB_FOLDER_HASH)

TB_CHANLIST = [
    "................",
    "..kkkkkkkkk.....",
    "..kwwwwwwwkk....",
    "..kwwnwnwwklk...",
    "..kwnnnnnwkkkk..",
    "..kwwnwnwwwwwk..",
    "..kwnnnnnwwwwk..",
    "..kwwnwnwwwwwk..",
    "..kwwwwwwwwwwk..",
    "..kwnwddddddwk..",
    "..kwwwwwwwwwwk..",
    "..kwnwddddddwk..",
    "..kwwwwwwwwwwk..",
    "..kwnwddddddwk..",
    "..kwwwwwwwwwwk..",
    "..kkkkkkkkkkkk..",
]
icon("toolbar/chanlist", TB_CHANLIST)

TB_NOTIFY = [
    "................",
    ".......kk.......",
    ".....kkkkkk.....",
    "....kwyyyyok....",
    "....kwyyyyok....",
    "...kwyyyyyyok...",
    "...kwyyyyyyok...",
    "...kwyyyyyyok...",
    "...kwyyyyyyok...",
    "..kwyyyyyyyyok..",
    ".kwyyyyyyyyyyok.",
    ".kooooooooooook.",
    "..kkkkkkkkkkkk..",
    "......kddk......",
    ".......kk.......",
    "................",
]
icon("toolbar/notify", TB_NOTIFY)

TB_URLS = [
    "...kkkk.........",
    ".kkbbggkk.......",
    "kcbbbggggk......",
    "kbgGbbbgbk......",
    "kbgGgbbbbbk.....",
    "kbggggbbggk.....",
    "kbbggbbbggk.kk..",
    "kbbbgbbggnkkldk.",
    ".kbbgbbbgnkl..dk",
    "..kknnnnkkkl..dk",
    "....kkkkkkkl..dk",
    ".....kllllkl..dk",
    "....kl....dkddk.",
    "....kl....dkkk..",
    ".....kddddk.....",
    "......kkkk......",
]
icon("toolbar/urls", TB_URLS)

icon("toolbar/dcc_send", [
    "................",
    ".kkkkkkk........",
    ".kwwwwwkk.......",
    ".kwwwwwklk......",
    ".kwwwwwkkkk.....",
    ".kwddddddwk.....",
    ".kwwwwwwwwk.....",
    ".kwddddkwwk.....",
    ".kwwwwwkGkk.....",
    ".kwddkkkkkGGk...",
    ".kwwwkGGGGGGGk..",
    ".kwwwkgggggggk..",
    ".kkkkkkkkkggk...",
    "..........kgk...",
    "..........kk....",
    "................",
])

icon("toolbar/dcc_chat", [
    "................",
    ".kkkkkkkkk......",
    "kwwwwwwwwlk.....",
    "kwddddddwlk.....",
    "kwwwwwwwwlk.....",
    "kwdddddwwlk.....",
    "kwwwwwkkkkkkkkk.",
    "kwwwwkyyyyyyyyok",
    ".kwkkkyyyyyyyyok",
    "kwk..kyddddddyok",
    "kk...kyyyyyyyyok",
    ".....kyddddyyyok",
    ".....koooooooook",
    "......kkkkkkkyk.",
    ".............kyk",
    "..............kk",
])

TB_TRANSFERS = [
    "................",
    "...kk...........",
    "..kGGk....kkkk..",
    ".kGGGgk...kbnk..",
    "kGGGGggk..kbnk..",
    "kkkGgkkk..kbnk..",
    "..kGgk....kbnk..",
    "..kGgk....kbnk..",
    "..kGgk....kbnk..",
    "..kGgk....kbnk..",
    "..kGgk..kkkbnkkk",
    "..kGgk..kbbbbnnk",
    "..kGgk...kbbbnk.",
    "..kkkk....kbbk..",
    "...........kk...",
    "................",
]
icon("toolbar/transfers", TB_TRANSFERS)

icon("toolbar/scripts", [
    "................",
    "..kkkkkkkkk...kk",
    ".kwllllllldk.krk",
    "..kkkkkkkkk.krmk",
    "..kwwwwwwwkkrmk.",
    "..kwnnnnnwkrmk..",
    "..kwwwwwwkrmk...",
    "..kwnnnnkldk....",
    "..kwwwwkdkk.....",
    "..kwnnnkwwk.....",
    "..kwwwwwwwk.....",
    "..kkkkkkkkk.....",
    ".kwllllllldk....",
    "..kkkkkkkkk.....",
    "................",
    "................",
])

icon("toolbar/colors", [
    "................",
    "....kkkkkkk.....",
    "...kyyyyyyyk....",
    "..kyorroobbyk...",
    ".kyoorroobboyk..",
    ".kyooooooooook..",
    "kyGGooooooowwyk.",
    "kyGGooooooowwok.",
    "kyoooooookkkyok.",
    "kyooooook...kyk.",
    ".kyoMMook...kk..",
    ".kyoMMook...kk..",
    "..kyoooookkkk...",
    "...kyoooooook...",
    "....kkkkkkk.....",
    "................",
])

icon("toolbar/cascade", [
    "................",
    "kkkkkkkkkk......",
    "kddddddddk......",
    "kwwwwwwwwk......",
    "kwwkkkkkkkkkk...",
    "kwwkddddddddk...",
    "kwwkwwwwwwwwk...",
    "kwwkwwkkkkkkkkkk",
    "kkkkwwknnnnnnnnk",
    "...kwwkwwwwwwwwk",
    "...kwwkwwwwwwwwk",
    "...kkkkwwwwwwwwk",
    "......kwwwwwwwwk",
    "......kwwwwwwwwk",
    "......kkkkkkkkkk",
    "................",
])

icon("toolbar/tile_h", [
    "................",
    "kkkkkkkkkkkkkkkk",
    "knnnnnnkkddddddk",
    "kwwwwwwkkwwwwwwk",
    "kwwwwwwkkwwwwwwk",
    "kwwwwwwkkwwwwwwk",
    "kwwwwwwkkwwwwwwk",
    "kwwwwwwkkwwwwwwk",
    "kwwwwwwkkwwwwwwk",
    "kwwwwwwkkwwwwwwk",
    "kwwwwwwkkwwwwwwk",
    "kwwwwwwkkwwwwwwk",
    "kwwwwwwkkwwwwwwk",
    "kwwwwwwkkwwwwwwk",
    "kkkkkkkkkkkkkkkk",
    "................",
])

icon("toolbar/tile_v", [
    "................",
    "kkkkkkkkkkkkkkkk",
    "knnnnnnnnnnnnnnk",
    "kwwwwwwwwwwwwwwk",
    "kwwwwwwwwwwwwwwk",
    "kwwwwwwwwwwwwwwk",
    "kwwwwwwwwwwwwwwk",
    "kkkkkkkkkkkkkkkk",
    "kkkkkkkkkkkkkkkk",
    "kddddddddddddddk",
    "kwwwwwwwwwwwwwwk",
    "kwwwwwwwwwwwwwwk",
    "kwwwwwwwwwwwwwwk",
    "kwwwwwwwwwwwwwwk",
    "kkkkkkkkkkkkkkkk",
    "................",
])

icon("toolbar/find", [
    "................",
    "...kkkk.........",
    ".kkllllkk.......",
    ".klwcccdk.......",
    "klwcccccdk......",
    "klccccctdk......",
    "klccccttdk......",
    "kdccttttdk......",
    ".kdttttdkk......",
    ".kkddddkkmk.....",
    "...kkkk.kmmk....",
    ".........kmmk...",
    "..........kmmk..",
    "...........kmmk.",
    "............kmmk",
    ".............kk.",
])

icon("toolbar/away", [
    "................",
    ".....kkkkk......",
    "...kkyyyyk......",
    "..kwyyykk...y...",
    ".kwyyok....yyy..",
    ".kwyyk......y...",
    "kwyyyk..........",
    "kwyyyk......w...",
    "kwyyyk..........",
    "kwyyyk..y.......",
    ".kwyyk..........",
    ".kwyyok.........",
    "..kyyyykk.......",
    "...kkooook......",
    ".....kkkkk......",
    "................",
])

icon("toolbar/about", [
    "................",
    "..kkkkkkkkkkkk..",
    ".kwwwwwwwwwwwwk.",
    "kwwwwwnnnnwwwwlk",
    "kwwwwnnwwnnwwwlk",
    "kwwwwwwwwnnwwwlk",
    "kwwwwwwwnnwwwwlk",
    "kwwwwwwnnwwwwwlk",
    "kwwwwwwwwwwwwwlk",
    "kwwwwwwnnwwwwwlk",
    ".kllllllllllllk.",
    "..kwlkkkkkkkkk..",
    "..klk...........",
    ".kk.............",
    "................",
    "................",
])

icon("toolbar/help", [
    "................",
    ".kkkkkkkkkkkk...",
    ".kbbbbbbbbbbkk..",
    ".kbnnnnnnnnnkwk.",
    ".kbnnyyyynnnkwk.",
    ".kbnyynnyynnkwk.",
    ".kbnnnnnyynnkwk.",
    ".kbnnnnyynnnkwk.",
    ".kbnnnyynnnnkwk.",
    ".kbnnnnnnnnnkwk.",
    ".kbnnnyynnnnkwk.",
    ".kbnnnnnnnnnkwk.",
    ".kkkkkkkkkkkkwk.",
    "..kwwwwwwwwwwwk.",
    "...kkkkkkkkkkk..",
    "................",
])

icon("toolbar/log", [
    "................",
    "...k..k..k..k...",
    ".kkdkkdkkdkkdkk.",
    ".kwkwwkwwkwwkwk.",
    ".kwwwwwwwwwwwwk.",
    ".kwwrwwwwwwwwwk.",
    ".kccrccccccccck.",
    ".kwwrwwwwwwwwwk.",
    ".kccrccccccccck.",
    ".kwwrwwwwwwwwwk.",
    ".kccrccccccccck.",
    ".kwwrwwwwwwwwwk.",
    ".kccrccccccccck.",
    ".kwwrwwwwwwwwwk.",
    ".kkkkkkkkkkkkkk.",
    "................",
])

icon("toolbar/relay", [
    ".......rr.......",
    "..b....kk....b..",
    ".b..b..kd..b..b.",
    ".b..b.kkkk.b..b.",
    "..b....kd....b..",
    ".......kd.......",
    "......kwwk......",
    ".....kwwllk.....",
    "....kbwwlnlk....",
    "...kbbwbnlnnk...",
    "..kbbbbbnnnnnk..",
    ".kbbbbbbnnnnnnk.",
    "kbbbbbbbnnnnnnnk",
    "kbbbbbbbbnnnnnnk",
    "kkkkkkkkkkkkkkkk",
    "................",
])


# ===========================================================================
# 3. Types de fenêtres (16×16)
# ===========================================================================

WIN_STATUS = [
    "............r...",
    "...........k....",
    "..........k.....",
    ".kkkkkkkkkkkkkk.",
    ".kwwwwwwwwwwwdk.",
    ".kwlddddddddldk.",
    ".kwldGGGGnnwldk.",
    ".kwldnnnnnnwldk.",
    ".kwldGGnGGGwldk.",
    ".kwldnnnnnnwldk.",
    ".kwlwwwwwwwwldk.",
    ".kwllllllllGldk.",
    ".kddddddddddddk.",
    ".kkkkkkkkkkkkkk.",
    ".....kddddk.....",
    "...kkkkkkkkkk...",
]
icon("window/status", WIN_STATUS)


def with_dot(base: list[str], fill: str, light: str, dark: str) -> list[str]:
    """Pose une pastille 5×5 (état du réseau) en bas à droite d'une icône 16×16."""
    dot = [
        ".kkk.",
        "k" + light + fill + fill + "k",
        "k" + fill + fill + dark + "k",
        "k" + fill + dark + dark + "k",
        ".kkk.",
    ]
    rows = [list(r) for r in base]
    for dy, line in enumerate(dot):
        for dx, ch in enumerate(line):
            if ch != ".":
                rows[11 + dy][11 + dx] = ch
    return ["".join(r) for r in rows]


icon("window/network_connected", with_dot(WIN_STATUS, "G", "w", "g"))
icon("window/network_disconnected", with_dot(WIN_STATUS, "r", "w", "m"))

icon("window/channel", [
    "................",
    "..kkkkkkkkkkkk..",
    ".kwwwwwwwwwwwwk.",
    "kwwwwwnwwnwwwwlk",
    "kwwwwnnnnnnwwwlk",
    "kwwwwwnwwnwwwwlk",
    "kwwwwwnwwnwwwwlk",
    "kwwwwnnnnnnwwwlk",
    "kwwwwwnwwnwwwwlk",
    "kwwwwwwwwwwwwwlk",
    ".kllllllllllllk.",
    "..kwlkkkkkkkkk..",
    "..klk...........",
    ".kk.............",
    "................",
    "................",
])

icon("window/query", [
    "........kkkkkkk.",
    ".......kwwwwwwlk",
    ".......kwnwnwnlk",
    ".......kwwwwwwlk",
    "..kkk..klllllllk",
    ".kwyyk..kwkkkkk.",
    ".kyyok.kwk......",
    ".kyyok.kk.......",
    "..kkk...........",
    ".kkkkk..........",
    "kctttnk.........",
    "kctttnk.........",
    "kctttnk.........",
    "kctttnk.........",
    "kctttnk.........",
    "kkkkkkk.........",
])

icon("window/dcc_chat", [
    "................",
    "................",
    "................",
    ".kkkkk....kkkkk.",
    "kwwwwlk..kyyyyok",
    "kwwwwlk..kyyyyok",
    "kwnwnlkkkkynynok",
    "kwwwwlk..kyyyyok",
    "klllllk..koooook",
    ".kwkkk....kkkyk.",
    "kwk..........kyk",
    "kk............kk",
    "................",
    "................",
    "................",
    "................",
])

icon("window/custom", [
    "................",
    "kkkkkkkkkkkkkkkk",
    "knnnnnnnnnnnnnnk",
    "kwwwwwwwwwwwwwwk",
    "kwwwwnnnnwwwwwwk",
    "kwwwnwwwwnwwwwwk",
    "kwwnwwnnnwnwwwwk",
    "kwwnwnwwnwnwwwwk",
    "kwwnwnwwnwnwwwwk",
    "kwwnwwnnnnwwwwwk",
    "kwwwnwwwwwwwwwwk",
    "kwwwwnnnnnwwwwwk",
    "kwwwwwwwwwwwwwwk",
    "kwwwwwwwwwwwwwwk",
    "kkkkkkkkkkkkkkkk",
    "................",
])

icon("window/chanlist", TB_CHANLIST)
icon("window/notify", TB_NOTIFY)
icon("window/urls", TB_URLS)
icon("window/transfers", TB_TRANSFERS)

icon("window/script_editor", [
    ".kkkkkkkk.......",
    ".kwwwwwwkk......",
    ".kwnnnnwklk.....",
    ".kwwwwwwkkkk..kk",
    ".kwwrrwnnnwk.kMk",
    ".kwwwwwwwwwkkMMk",
    ".kwwgggggwkllk..",
    ".kwwwwwwwkyok...",
    ".kwwppwnkyok....",
    ".kwwwwwkyok.....",
    ".kwnwwkwlk......",
    ".kwwwwkkk.......",
    ".kwwwwwwwwwk....",
    ".kwwwwwwwwwk....",
    ".kkkkkkkkkkk....",
    "................",
])


# ===========================================================================
# 4. Petits glyphes d'interface
# ===========================================================================
# Les glyphes des boutons de légende (close_x, minimize, maximize, restore)
# sont placés comme sur un bouton Windows 98 de 16×14 : la ligne 0 et la ligne
# 15 restent vides ; en ne gardant que les lignes 1 à 14, on obtient le glyphe
# exact pour un bouton 16×14.

icon("ui/check", [
    "................",
    "................",
    "................",
    "................",
    "................",
    "..........k.....",
    ".........kk.....",
    "....k...kkk.....",
    "....kk.kkk......",
    "....kkkkk.......",
    ".....kkk........",
    "......k.........",
    "................",
    "................",
    "................",
    "................",
])

icon("ui/radio_dot", [
    "................",
    "................",
    "................",
    "................",
    "................",
    "......kkkk......",
    ".....kkkkkk.....",
    ".....kkkkkk.....",
    ".....kkkkkk.....",
    ".....kkkkkk.....",
    "......kkkk......",
    "................",
    "................",
    "................",
    "................",
    "................",
])

ARROW_RIGHT = [
    "................",
    "................",
    "................",
    "................",
    "......k.........",
    "......kk........",
    "......kkk.......",
    "......kkkk......",
    "......kkk.......",
    "......kk........",
    "......k.........",
    "................",
    "................",
    "................",
    "................",
    "................",
]
ARROW_DOWN = [
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "....kkkkkkk.....",
    ".....kkkkk......",
    "......kkk.......",
    ".......k........",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
]


def mirror_h(grid: list[str]) -> list[str]:
    """Symétrie gauche-droite autour du milieu de l'icône."""
    return [row[::-1] for row in grid]


def mirror_v(grid: list[str]) -> list[str]:
    """Symétrie haut-bas autour du milieu de l'icône."""
    return list(reversed(grid))


icon("ui/arrow_right", ARROW_RIGHT)
icon("ui/arrow_left", mirror_h(ARROW_RIGHT))
icon("ui/arrow_down", ARROW_DOWN)
icon("ui/arrow_up", mirror_v(ARROW_DOWN))

icon("ui/close_x", [
    "................",
    "................",
    "................",
    "................",
    "....kk....kk....",
    ".....kk..kk.....",
    "......kkkk......",
    ".......kk.......",
    "......kkkk......",
    ".....kk..kk.....",
    "....kk....kk....",
    "................",
    "................",
    "................",
    "................",
    "................",
])

icon("ui/minimize", [
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "................",
    "....kkkkkk......",
    "....kkkkkk......",
    "................",
    "................",
    "................",
    "................",
])

icon("ui/maximize", [
    "................",
    "................",
    "................",
    "...kkkkkkkkk....",
    "...kkkkkkkkk....",
    "...k.......k....",
    "...k.......k....",
    "...k.......k....",
    "...k.......k....",
    "...k.......k....",
    "...k.......k....",
    "...kkkkkkkkk....",
    "................",
    "................",
    "................",
    "................",
])

icon("ui/restore", [
    "................",
    "................",
    "................",
    ".....kkkkkk.....",
    ".....kkkkkk.....",
    ".....k....k.....",
    "...kkkkkk.k.....",
    "...kkkkkk.k.....",
    "...k....kkk.....",
    "...k....k.......",
    "...k....k.......",
    "...kkkkkk.......",
    "................",
    "................",
    "................",
    "................",
])

icon("ui/folder_closed", [
    "................",
    "................",
    "..kkkk..........",
    ".kwyyyk.........",
    ".kwyyyykkkkkkkk.",
    ".kwyyyyyyyyyyok.",
    ".kwyyyyyyyyyyok.",
    ".kwyyyyyyyyyyok.",
    ".kwyyyyyyyyyyok.",
    ".kwyyyyyyyyyyok.",
    ".kwyyyyyyyyyyok.",
    ".kwyyyyyyyyyyok.",
    ".kwyyyyyyyyyyok.",
    ".kooooooooooook.",
    ".kkkkkkkkkkkkkk.",
    "................",
])

icon("ui/folder_open", [
    "................",
    "................",
    ".kkkk...........",
    "kwyyyk..........",
    "kwyyyykkkkkkk...",
    "kwyyyyyyyyyyk...",
    "kwyykkkkkkkkkkkk",
    "kwykwyyyyyyyyyok",
    "kwkwyyyyyyyyyok.",
    "kwkwyyyyyyyyyok.",
    "kkwyyyyyyyyyok..",
    "kkwyyyyyyyyyok..",
    "kwyyyyyyyyyok...",
    "kkkkkkkkkkkkk...",
    "................",
    "................",
])

icon("ui/warning", [
    "................",
    ".......kk.......",
    "......kwok......",
    "......kwok......",
    ".....kwyyok.....",
    ".....kwkkok.....",
    "....kwykkyok....",
    "....kwykkyok....",
    "...kwyykkyyok...",
    "...kwyykkyyok...",
    "..kwyyykkyyyok..",
    "..kwyyyyyyyyok..",
    ".kwyyyykkyyyyok.",
    ".kwoooooooooook.",
    "kkkkkkkkkkkkkkkk",
    "................",
])

icon("ui/info", [
    "................",
    "......kkkk......",
    "....kkccbbkk....",
    "...kcbbwwbbbk...",
    "..kcbbbwwbbbbk..",
    "..kcbbbbbbbbbk..",
    ".kcbbbwwwbbbbbk.",
    ".kcbbbbwwbbbbbk.",
    ".kbbbbbwwbbbbnk.",
    ".kbbbbbwwbbbbnk.",
    "..kbbbbwwbbbnk..",
    "..kbbbwwwwbbnk..",
    "...kbbbbbbbnk...",
    "....kkbnnnkk....",
    "......kkkk......",
    "................",
])

icon("ui/error", [
    "................",
    ".....kkkkkk.....",
    "....krrrrrrk....",
    "...krrrrrrrrk...",
    "..krwwrrrrwwrk..",
    ".krrwwwrrwwwrrk.",
    ".krrrwwwwwwrrrk.",
    ".krrrrwwwwrrrrk.",
    ".krrrrwwwwrrrmk.",
    ".krrrwwwwwwrrmk.",
    ".krrwwwrrwwwrmk.",
    "..krwwrrrrwwmk..",
    "...krrrrrrrmk...",
    "....krrmmmmk....",
    ".....kkkkkk.....",
    "................",
])

# Icônes des boîtes de message, 32×32, avec ombre portée translucide.

# Information : disque bleu, « i » blanc.
icon("ui/msg_info_32", [
    "................................",
    "...........kkkkkkkk.............",
    ".........kkcccccccckk...........",
    ".......kkccbbbbbbbbcckk.........",
    "......kccbbbbbbbbbbbbbbk........",
    ".....kcbbbbbbbbbbbbbbbbbk.......",
    "....kcbbbbbbbbwwbbbbbbbbbk......",
    "...kcbbbbbbbbwwwwbbbbbbbbbk.....",
    "...kcbbbbbbbbwwwwbbbbbbbbbks....",
    "..kcbbbbbbbbbbwwbbbbbbbbbbnks...",
    "..kcbbbbbbbbbbbbbbbbbbbbbbnks...",
    ".kcbbbbbbbbbbbbbbbbbbbbbbbbnks..",
    ".kcbbbbbbbbbwwwwbbbbbbbbbbbnks..",
    ".kcbbbbbbbbbbbwwbbbbbbbbbbbnkss.",
    ".kcbbbbbbbbbbbwwbbbbbbbbbbbnkss.",
    ".kcbbbbbbbbbbbwwbbbbbbbbbbbnkss.",
    ".kcbbbbbbbbbbbwwbbbbbbbbbbbnkss.",
    ".kcbbbbbbbbbbbwwbbbbbbbbbbbnkss.",
    ".kcbbbbbbbbbbbwwbbbbbbbbbbbnkss.",
    "..kcbbbbbbbbbbwwbbbbbbbbbbnksss.",
    "..kcbbbbbbbbbbwwbbbbbbbbbbnksss.",
    "...kbbbbbbbbbwwwwbbbbbbbbnksss..",
    "...kbbbbbbbbwwwwwwbbbbbbbnksss..",
    "....kbbbbbbbbbbbbbbbbbbbnksss...",
    ".....kbbbbbbbbbbbbbbbbbnkssss...",
    "......kbbbbbbbbbbbbbbnnkssss....",
    ".......kknnbbbbbbbbnnkkssss.....",
    "........skknnnnnnnnkksssss......",
    ".........sskkkkkkkkssssss.......",
    "...........ssssssssssss.........",
    ".............ssssssss...........",
    "................................",
])

# Question : disque sarcelle, « ? » blanc.
icon("ui/msg_question_32", [
    "................................",
    "...........kkkkkkkk.............",
    ".........kkcccccccckk...........",
    ".......kkccttttttttcckk.........",
    "......kccttttttttttttttk........",
    ".....kctttttttttttttttttk.......",
    "....kctttttttttttttttttttk......",
    "...kctttttttttttttttttttttk.....",
    "...kctttttttwwwwwwttttttttks....",
    "..kctttttttwwwwwwwwtttttttnks...",
    "..kcttttttwwwttttwwwttttttnks...",
    ".kctttttttwwttttttwwtttttttnks..",
    ".kcttttttttttttttwwwtttttttnks..",
    ".kctttttttttttttwwwttttttttnkss.",
    ".kcttttttttttttwwwtttttttttnkss.",
    ".kctttttttttttwwwttttttttttnkss.",
    ".kctttttttttttwwtttttttttttnkss.",
    ".kctttttttttttwwtttttttttttnkss.",
    ".kcttttttttttttttttttttttttnkss.",
    "..kcttttttttttwwttttttttttnksss.",
    "..kctttttttttwwwwtttttttttnksss.",
    "...kttttttttttwwtttttttttnksss..",
    "...ktttttttttttttttttttttnksss..",
    "....ktttttttttttttttttttnksss...",
    ".....ktttttttttttttttttnkssss...",
    "......kttttttttttttttnnkssss....",
    ".......kknnttttttttnnkkssss.....",
    "........skknnnnnnnnkksssss......",
    ".........sskkkkkkkkssssss.......",
    "...........ssssssssssss.........",
    ".............ssssssss...........",
    "................................",
])

# Avertissement : triangle jaune, « ! » noir.
icon("ui/msg_warning_32", [
    "................................",
    "..............kk................",
    ".............kwwk...............",
    ".............kwwks..............",
    "............kwyywks.............",
    "............kwyywks.............",
    "...........kwyyyywks............",
    "...........kwyyyywks............",
    "..........kwyyyyyywks...........",
    "..........kwyykkyywks...........",
    ".........kwyykkkkyywks..........",
    "........kwyyykkkkyyyyk..........",
    "........kwyyykkkkyyyyks.........",
    ".......kwyyyykkkkyyyyyks........",
    ".......kwyyyykkkkyyyyyks........",
    "......kwyyyyyykkyyyyyyoks.......",
    "......kwyyyyyykkyyyyyyoks.......",
    ".....kwyyyyyyykkyyyyyyyoks......",
    ".....kwyyyyyyykkyyyyyyyoks......",
    "....kwyyyyyyyyyyyyyyyyyyoks.....",
    "....kwyyyyyyyyyyyyyyyyyyoks.....",
    "...kwyyyyyyyyykkyyyyyyyyyoks....",
    "..kwyyyyyyyyykkkkyyyyyyyyyok....",
    "..kwyyyyyyyyykkkkyyyyyyyyyoks...",
    ".kwyyyyyyyyyyykkyyyyyyyyyyyoks..",
    ".kwyyyyyyyyyyyyyyyyyyyyyyyyoks..",
    "kwyyyyyyyyoooooooooooooooooooks.",
    "kkkkkkkkkkkkkkkkkkkkkkkkkkkkkks.",
    "..ssssssssssssssssssssssssssssss",
    "..ssssssssssssssssssssssssssssss",
    "................................",
    "................................",
])

# Erreur : octogone rouge, croix blanche.
icon("ui/msg_error_32", [
    "................................",
    ".........kkkkkkkkkkkk...........",
    "........krrrrrrrrrrrrk..........",
    ".......krrrrrrrrrrrrrrk.........",
    "......krrrrrrrrrrrrrrrrk........",
    ".....krrrrrrrrrrrrrrrrrrk.......",
    "....krrrrrrrrrrrrrrrrrrrrk......",
    "...krrrrrrrrrrrrrrrrrrrrrrk.....",
    "..krrrrrwwrrrrrrrrrrwwrrrrmk....",
    ".krrrrrrwwwrrrrrrrrwwwrrrrrmk...",
    ".krrrrrrrwwwrrrrrrwwwrrrrrrmks..",
    ".krrrrrrrrwwwrrrrwwwrrrrrrrmkss.",
    ".krrrrrrrrrwwwrrwwwrrrrrrrrmkss.",
    ".krrrrrrrrrrwwwwwwrrrrrrrrrmkss.",
    ".krrrrrrrrrrrwwwwrrrrrrrrrrmkss.",
    ".krrrrrrrrrrrwwwwrrrrrrrrrrmkss.",
    ".krrrrrrrrrrwwwwwwrrrrrrrrrmkss.",
    ".krrrrrrrrrwwwrrwwwrrrrrrrrmkss.",
    ".krrrrrrrrwwwrrrrwwwrrrrrrrmkss.",
    ".krrrrrrrwwwrrrrrrwwwrrrrrrmkss.",
    ".krrrrrrwwwrrrrrrrrwwwrrrrrmkss.",
    "..krrrrrwwrrrrrrrrrrwwrrrrmksss.",
    "...krrrrrrrrrrrrrrrrrrrrrmkssss.",
    "....krrrrrrrrrrrrrrrrrrrmkssss..",
    ".....krrrrrrrrrrrrrrrrrmkssss...",
    "......krrrrrrrrrrrrrrrmkssss....",
    ".......kmrrrrrrrrrrrrmkssss.....",
    "........kmmmmmmmmmmmmkssss......",
    ".........kkkkkkkkkkkkssss.......",
    "..........ssssssssssssss........",
    "...........ssssssssssss.........",
    "................................",
])


# ===========================================================================
# Rendu
# ===========================================================================

def grid_to_image(grid: list[str], palette: dict, name: str = "?") -> Image.Image:
    h = len(grid)
    w = len(grid[0])
    img = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    px = img.load()
    for y, row in enumerate(grid):
        if len(row) != w:
            raise ValueError(f"{name}: ligne {y} de longueur {len(row)} (attendu {w})")
        for x, ch in enumerate(row):
            if ch not in palette:
                raise ValueError(f"{name}: caractère inconnu {ch!r} en ({x},{y})")
            px[x, y] = palette[ch]
    return img


def expected_size(path: str) -> int:
    if path in ("app/alpirc-32",) or path.startswith("ui/msg_"):
        return 32
    return 16


def render_all() -> dict[str, Image.Image]:
    out = {}
    for path, (grid, pal) in ICONS.items():
        img = grid_to_image(grid, pal, path)
        n = expected_size(path)
        if img.size != (n, n):
            raise ValueError(f"{path}: taille {img.size}, attendu {n}×{n}")
        out[path] = img
    return out


def scale(img: Image.Image, k: int) -> Image.Image:
    """Agrandissement au plus proche voisin (pixels nets)."""
    return img.resize((img.width * k, img.height * k), Image.NEAREST)


def placement(art: Image.Image, k: int, size: int = 1024) -> tuple[int, int]:
    """Décalage qui centre `art` agrandi ×k sur sa boîte englobante (centrage optique)."""
    x0, y0, x1, y1 = art.getbbox()
    return ((size - (x1 - x0) * k) // 2 - x0 * k,
            (size - (y1 - y0) * k) // 2 - y0 * k)


def place_centered(art: Image.Image, k: int, size: int = 1024) -> Image.Image:
    big = scale(art, k)
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    canvas.paste(big, placement(art, k, size), big)
    return canvas


def max_scale_in_circle(art: Image.Image, diameter: float, size: int = 1024) -> int:
    """Plus grand facteur entier tel que tous les pixels opaques tiennent dans un
    disque centré de ce diamètre (zone sûre des icônes adaptatives Android)."""
    alpha = art.getchannel("A").load()
    opaque = [(x, y) for y in range(art.height) for x in range(art.width) if alpha[x, y]]
    r, c = diameter / 2, size / 2
    k = 1
    while True:
        n = k + 1
        ox, oy = placement(art, n, size)
        if any(math.hypot(ox + (x + dx) * n - c, oy + (y + dy) * n - c) > r
               for x, y in opaque for dx in (0, 1) for dy in (0, 1)):
            return k
        k = n


def save_all(images: dict[str, Image.Image]) -> None:
    for path, img in images.items():
        dest = os.path.join(ICONS_DIR, path + ".png")
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        img.save(dest, optimize=True)


# Facteurs des grandes versions de l'icône (calculés une fois, voir README).
MACOS_SCALE = 26                 # 32 × 26 = 832 px, ~81 % du canevas 1024
ANDROID_SAFE_DIAMETER = 1024 * 66 / 108   # zone sûre : disque de 66 dp sur 108 dp
NOTIFICATION_SCALE = 4         # 24 × 4 = 96 px (xxxhdpi)
TEAL = (0, 128, 128, 255)


def save_app_large(art32: Image.Image) -> dict[str, Image.Image]:
    out = {}
    out["app/alpirc-1024"] = place_centered(art32, MACOS_SCALE)
    k = max_scale_in_circle(art32, ANDROID_SAFE_DIAMETER)
    out["app/alpirc-1024-android-fg"] = place_centered(art32, k)
    out["app/alpirc-1024-android-bg"] = Image.new("RGBA", (1024, 1024), TEAL)
    notif = grid_to_image(NOTIFICATION, PALETTE, "notification")
    if notif.size != (24, 24):
        raise ValueError(f"notification : taille {notif.size}, attendu 24×24")
    out["app/alpirc-notification"] = scale(notif, NOTIFICATION_SCALE)
    for path, img in out.items():
        img.save(os.path.join(ICONS_DIR, path + ".png"), optimize=True)
    print(f"icône Android : facteur ×{k} ({32 * k} px dans 1024)")
    return out


# ===========================================================================
# Planche de contrôle (preview.png)
# ===========================================================================

GRAY = (192, 192, 192, 255)
DARK = (128, 128, 128, 255)
WHITE = (255, 255, 255, 255)
BLACK = (0, 0, 0, 255)
NAVY = (0, 0, 128, 255)


def _font(bold: bool = False) -> ImageFont.ImageFont:
    name = "PixelOperator-Bold.ttf" if bold else "PixelOperator.ttf"
    path = os.path.join(REPO, "assets", "fonts", name)
    try:
        return ImageFont.truetype(path, 16)
    except OSError:
        return ImageFont.load_default()


def _bevel(d: ImageDraw.ImageDraw, x: int, y: int, w: int, h: int, raised: bool = True) -> None:
    """Relief Windows 98 (bouton) : blanc/gris foncé/noir."""
    tl, br = (WHITE, BLACK) if raised else (BLACK, WHITE)
    d.rectangle([x, y, x + w - 1, y + h - 1], fill=GRAY)
    d.line([x, y, x + w - 2, y], fill=tl)
    d.line([x, y, x, y + h - 2], fill=tl)
    d.line([x, y + h - 1, x + w - 1, y + h - 1], fill=br)
    d.line([x + w - 1, y, x + w - 1, y + h - 1], fill=br)
    if raised:
        d.line([x + 1, y + h - 2, x + w - 2, y + h - 2], fill=DARK)
        d.line([x + w - 2, y + 1, x + w - 2, y + h - 2], fill=DARK)


def _toolbar_strip(images: dict[str, Image.Image], names: list[str]) -> Image.Image:
    """Barre d'outils façon Windows 98 : boutons 23×22, icône 16×16."""
    w = 4 + len(names) * 23 + 4
    strip = Image.new("RGBA", (w, 28), GRAY)
    d = ImageDraw.Draw(strip)
    d.line([0, 0, w - 1, 0], fill=WHITE)
    d.line([0, 27, w - 1, 27], fill=DARK)
    for i, n in enumerate(names):
        x = 4 + i * 23
        _bevel(d, x, 3, 23, 22)
        icon_img = images[n]
        strip.paste(icon_img, (x + 3, 6), icon_img)
    return strip


def _title_strip(images: dict[str, Image.Image], names: list[str], active: bool) -> Image.Image:
    w = 4 + len(names) * 20
    strip = Image.new("RGBA", (w, 20), NAVY if active else DARK)
    for i, n in enumerate(names):
        icon_img = images[n]
        strip.paste(icon_img, (4 + i * 20, 2), icon_img)
    return strip


def _caption_buttons(images: dict[str, Image.Image]) -> Image.Image:
    names = ["ui/minimize", "ui/maximize", "ui/restore", "ui/close_x"]
    strip = Image.new("RGBA", (4 + len(names) * 18, 18), NAVY)
    d = ImageDraw.Draw(strip)
    for i, n in enumerate(names):
        x = 2 + i * 18
        _bevel(d, x, 2, 16, 14)
        glyph = images[n].crop((0, 1, 16, 15))   # glyphe 16×14
        strip.paste(glyph, (x, 2), glyph)
    return strip


def build_preview(images: dict[str, Image.Image]) -> Image.Image:
    font, bold = _font(), _font(True)
    meas = ImageDraw.Draw(Image.new("RGBA", (1, 1)))
    width = 1180
    blocks: list[tuple[str, object]] = []

    def section(title):
        blocks.append(("title", title))

    section("AlpIRC — icône de l'application (×8)")
    blocks.append(("app", None))
    for group, title in (("toolbar/", "Barre d'outils (×1 et ×3)"),
                         ("window/", "Types de fenêtres (×1 et ×3)"),
                         ("ui/", "Glyphes d'interface (×1 et ×3)")):
        section(title)
        names = [n for n in ICONS if n.startswith(group)]
        blocks.append(("grid", names))
        if group == "toolbar/":
            blocks.append(("toolbar", names))
        if group == "window/":
            blocks.append(("titles", names))
        if group == "ui/":
            blocks.append(("captions", None))

    # Mise en page en deux passes : on mesure, puis on dessine.
    def layout(draw: ImageDraw.ImageDraw | None, canvas: Image.Image | None) -> int:
        y = 12
        for kind, data in blocks:
            if kind == "title":
                if draw:
                    draw.text((12, y), data, font=bold, fill=NAVY)
                y += 24
            elif kind == "app":
                a32, a16 = images["app/alpirc-32"], images["app/alpirc-16"]
                # 1re rangée : le dessin à ×8, puis les tailles réelles sur quatre fonds.
                row1 = [(scale(a32, 8), "alpirc-32 ×8"), (scale(a16, 8), "alpirc-16 ×8")]
                # 2e rangée : les grandes versions réduites à 256 px, et la notification.
                mac = images["app/alpirc-1024"].resize((256, 256), Image.LANCZOS)
                android = images["app/alpirc-1024-android-bg"].copy()
                fg = images["app/alpirc-1024-android-fg"]
                android.paste(fg, (0, 0), fg)
                mask = Image.new("L", (1024, 1024), 0)
                ImageDraw.Draw(mask).ellipse([171, 171, 852, 852], fill=255)  # 72 dp sur 108
                android.putalpha(mask)
                android = android.crop((171, 171, 853, 853)).resize((256, 256), Image.LANCZOS)
                notif = images["app/alpirc-notification"]
                panel = Image.new("RGBA", (256, 256), (40, 40, 40, 255))
                panel.paste(notif, (80, 40), notif)
                small = notif.resize((24, 24), Image.BOX)  # 24 dp en mdpi
                status = Image.new("RGBA", (256, 32), (25, 90, 160, 255))
                status.paste(small, (8, 4), small)
                panel.paste(status, (0, 200))
                row2 = [(mac, "alpirc-1024 (macOS), réduite"),
                        (android, "Android : fg sur bg, masque rond"),
                        (panel, "notification 96 px, et 24 px")]
                for row in (row1, row2):
                    x = 12
                    for im, label in row:
                        if canvas:
                            canvas.paste(im, (x, y), im)
                            draw.text((x, y + im.height + 4), label, font=font, fill=BLACK)
                        x += max(im.width, int(meas.textlength(label, font=font))) + 28
                    if row is row1 and canvas:
                        # tailles réelles sur fonds gris, marine, sarcelle et blanc
                        for i, bgc in enumerate((GRAY, NAVY, TEAL, WHITE)):
                            for j, im in enumerate((a32, a16)):
                                cx, cy = x + j * 44, y + i * 44
                                draw.rectangle([cx, cy, cx + 39, cy + 39], fill=bgc)
                                canvas.paste(im, (cx + (40 - im.width) // 2,
                                                  cy + (40 - im.height) // 2), im)
                    y += max(im.height for im, _ in row) + 40
            elif kind == "grid":
                x, row_h = 12, 0
                for n in data:
                    im = images[n]
                    k = 3
                    label_w = int(meas.textlength(n.split("/", 1)[1], font=font))
                    cell_w = max(im.width + 6 + im.width * k, 130, label_w + 4)
                    cell_h = 20 + im.height * k + 8
                    if x + cell_w > width - 12:
                        x = 12
                        y += row_h
                        row_h = 0
                    if canvas:
                        draw.text((x, y), n.split("/", 1)[1], font=font, fill=BLACK)
                        canvas.paste(im, (x, y + 20), im)
                        big = scale(im, k)
                        canvas.paste(big, (x + im.width + 6, y + 20), big)
                    x += cell_w + 10
                    row_h = max(row_h, cell_h)
                y += row_h + 6
            elif kind == "toolbar":
                strip = _toolbar_strip(images, data)
                if canvas:
                    canvas.paste(strip, (12, y))
                    s2 = scale(strip, 2)
                    canvas.paste(s2, (12, y + strip.height + 8))
                y += strip.height * 3 + 22
            elif kind == "titles":
                small = [n for n in data if True]
                a, b = _title_strip(images, small, True), _title_strip(images, small, False)
                if canvas:
                    canvas.paste(a, (12, y))
                    canvas.paste(b, (12 + a.width + 12, y))
                    canvas.paste(scale(a, 2), (12, y + 28))
                y += 28 + 40 + 16
            elif kind == "captions":
                cb = _caption_buttons(images)
                if canvas:
                    canvas.paste(cb, (12, y))
                    canvas.paste(scale(cb, 3), (12 + cb.width + 16, y))
                y += cb.height * 3 + 20
        return y

    height = layout(None, None)
    sheet = Image.new("RGBA", (width, height), GRAY)
    d = ImageDraw.Draw(sheet)
    d.fontmode = "1"
    layout(d, sheet)
    return sheet


def main() -> int:
    images = render_all()
    save_all(images)
    images.update(save_app_large(images["app/alpirc-32"]))
    sheet = build_preview(images)
    sheet.save(os.path.join(ICONS_DIR, "preview.png"), optimize=True)
    print(f"{len(images)} images écrites dans {ICONS_DIR} (+ preview.png)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
