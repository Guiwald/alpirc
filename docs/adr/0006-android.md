# ADR 0006 — Android : disposition adaptée, saisie native, connexion permanente

- Statut : acceptée (17/10/2026, grilling par lots, Q3, Q8)

## Contexte

Des fenêtres dans une fenêtre sont inutilisables sur un écran de 6 pouces. Avec une `NativeActivity` seule, le
clavier du téléphone (Gboard) marche mal : ni autocorrection, ni suggestions, ni dictée, ni emoji fiables. Enfin,
Android coupe le réseau des applis en arrière-plan ; le type de service « dataSync » est limité à 6 h par 24 h depuis
Android 15, pas les types « specialUse » ni « remoteMessaging ».

## Décision

1. **Disposition adaptée au style rétro** (couleurs, police, icônes d'AlpIRC) : une fenêtre à la fois en plein
   écran ; un tiroir à gauche (réseaux, canaux, privés) et la liste des pseudos à droite ; barre du haut avec le
   nom de la fenêtre et le sujet.
2. **Champ de saisie natif** (un `EditText` Android) en bas de l'écran : clavier complet, autocorrection, dictée,
   emoji, coller. Tout le reste est dessiné par Rust.
3. **Connexion permanente** par défaut : service au premier plan (type `specialUse`) avec une notification
   (« Connecté à 2 réseaux »). Option : « se déconnecter après N minutes en arrière-plan ».
4. Accueil ou Retour laisse AlpIRC connecté ; **balayer l'appli des applis récentes déconnecte et quitte** (comme
   Alpagamp, ADR 0023 d'Alpagamp).
5. Notifications quand on vous nomme et pour les privés.
6. Boîtes de dialogue : celles du bureau, mises à l'échelle de la largeur de l'écran, avec des zones tactiles
   agrandies.

## Conséquences

- Écart limité à l'ADR 0003 : quelques classes Java (service, activité, champ de saisie), comme pour Alpagamp.
- APK construit sans Gradle par `cargo xtask apk`.
