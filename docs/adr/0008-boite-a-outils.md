# ADR 0008 — Interface : notre propre boîte à outils

- Statut : acceptée (17/10/2026, grilling par lots, Q16)

## Options examinées

- **Slint** : vrais widgets et Android officiel, mais le style Windows 98 et le MDI sont à recréer quand même ; la
  licence gratuite impose la mention « Made with Slint » (ou la GPL).
- **egui** : rapide à écrire, mais son allure se plie mal au Windows 98 et sa saisie sur Android est faible.
- **Notre propre boîte à outils**, sur la base d'Alpagamp : fidélité totale.

## Décision

- **winit** (fenêtres, clavier, souris, méthode de saisie IME), **softbuffer** (affichage d'un tampon de pixels),
  **cosmic-text** (texte : mise en forme, repli de polices, emoji, écritures de droite à gauche).
- Widgets maison façon Windows 98 : boutons, cases, boutons radio, champs de texte (une ligne et plusieurs),
  listes, listes à colonnes, arbre, onglets, listes déroulantes, barres de défilement, cadres, menus surgissants ;
  fenêtres internes (MDI), switchbar, treebar, barre d'outils.
- Disposition des boîtes de dialogue en unités de dialogue (comme Windows et les dialogues mSL).
- La crate `alpirc-ui` ne dépend d'aucune plateforme : elle dessine dans un tampon et reçoit des événements. Elle
  se teste sur ordinateur, images à l'appui.

## Conséquences

- C'est le plus de code : édition de texte, sélection, défilement, touches mortes et IME.
- VoiceOver ne voit rien au début ; AccessKit pourra être branché plus tard.
- Les dialogues mSL réutilisent les mêmes widgets.
