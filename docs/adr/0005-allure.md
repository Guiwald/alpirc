# ADR 0005 — Allure : mIRC 5.x/6.x fidèle, avec des thèmes

- Statut : acceptée (17/10/2026, grilling par lots, Q2, Q17)

## Décision

- **Fidèle à mIRC 5.x/6.x par défaut** : une fenêtre principale qui contient les fenêtres Status, canaux et privés
  (MDI), avec barre d'outils, **switchbar** en haut (la **treebar** à gauche en option), liste des pseudos à droite,
  ligne de saisie en bas, police à chasse fixe, boîtes de dialogue grises en relief façon Windows 98.
- Les fenêtres internes se déplacent, se redimensionnent, se réduisent, s'agrandissent (le cas le plus courant :
  agrandies, leurs boutons passant dans la barre de menus), se disposent en cascade ou en mosaïque.
- **Thèmes** : jeux de couleurs (classique façon mIRC, sombre, et ceux de l'utilisateur), choix de la police.
- **macOS** : les menus vont dans la vraie barre de menus du Mac ; les raccourcis Ctrl deviennent Cmd quand ils ne
  sont pas réservés par macOS. **Linux** : barre de menus dessinée dans la fenêtre, comme sous Windows.
- Menus contextuels (clic droit) et boîtes de dialogue dessinés par AlpIRC, partout.

## Conséquences

- Tous les widgets sont dessinés par AlpIRC (ADR 0008).
- Une seule fenêtre du système : sous Wayland, aucune contrainte de placement (contrairement au groupe aimanté
  d'Alpagamp).
