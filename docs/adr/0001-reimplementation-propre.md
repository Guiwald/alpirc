# ADR 0001 — Réimplémentation propre, sans rien emprunter à mIRC

- Statut : acceptée (17/10/2026, grilling par lots, Q1, Q14, Q17, Q20)

## Contexte

Guillaume utilisait mIRC à la fin des années 90 et au début des années 2000 (Undernet : #lyon, #grenoble,
#chambery). mIRC existe toujours (7.85, septembre 2026), mais seulement sous Windows, et reste un logiciel
propriétaire (shareware). « mIRC » est une marque déposée de mIRC Co. Ltd. Son code n'est pas publié. Son langage
de script (mSL) et le protocole IRC, eux, sont documentés publiquement ; un langage n'est pas protégé par le droit
d'auteur, et d'autres clients le réimplémentent déjà (AdiIRC).

## Décision

- Le client a **son propre nom** : AlpIRC (ADR 0013). Ni « mIRC », ni son logo, ni ses icônes, ni ses textes.
- **Aucune ligne de code** n'est reprise d'un autre client IRC sous licence copyleft (HexChat, KVIrc, irssi…) :
  on lit la documentation (RFC 1459, RFC 2812, specs IRCv3, aide de mIRC pour le comportement de mSL) et on écrit
  notre code. Du code permissif (MIT, BSD, Apache) peut être réutilisé avec attribution dans `THIRD_PARTY.md`.
- La **disposition** de mIRC 5.x/6.x (fenêtres dans la fenêtre, switchbar, liste des pseudos à droite, ligne de
  saisie en bas, codes de couleur Ctrl+K) est reprise : c'est un usage et une ergonomie, pas une œuvre.
- **Icônes originales** en pixel-art, dessinées pour AlpIRC.
- **Police des discussions : Fixedsys Excelsior** (domaine public), proche du Fixedsys de Windows que mIRC utilisait
  par défaut. Police des menus et boîtes : une police libre de style MS Sans Serif (licence OFL, CC0 ou domaine
  public), sinon dessinée pour AlpIRC.

## Conséquences

- Les palettes de couleurs mIRC (16 + 99 couleurs) sont des valeurs de protocole de fait : elles sont reprises.
- Les scripts mSL écrits pour mIRC peuvent se charger dans AlpIRC, dans la limite de l'ADR 0007.
