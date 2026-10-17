# ADR 0003 — Tout en Rust

- Statut : acceptée (17/10/2026, grilling par lots, Q13 ; mêmes principes qu'Alpagamp)

## Décision

- **Toute la logique de l'application est en Rust.**
- **Aucune bibliothèque C ou C++** en dépendance. Les crates qui enveloppent les API du système (AppKit, Cocoa,
  ALSA, D-Bus chargé à l'exécution, JNI…) sont acceptées.
- **Java minimal sur Android** : seulement ce qu'on ne peut pas faire depuis Rust (service au premier plan,
  notifications, champ de saisie natif, sélecteur de fichiers, ouverture d'adresses). Aucune logique IRC en Java.
- **Réseau** : tokio, rustls avec la cryptographie **graviola** (Rust pur), racines de confiance Mozilla
  (`webpki-roots`) et, hors Android, celles du système.
- **Texte** : cosmic-text (mise en forme, repli de polices, emoji) ; **fenêtres** : winit ; **dessin** : au pixel,
  dans un tampon (softbuffer).

## Conséquences

- La cryptographie de SASL SCRAM-SHA-256 et du relais vient des crates RustCrypto (Rust pur).
- Les fichiers de paquet et de configuration (manifeste Android, Info.plist, workflows, scripts) sont tolérés.
