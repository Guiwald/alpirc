# ADR 0012 — Langues de l'interface et encodages

- Statut : acceptée (17/10/2026, grilling par lots, Q6, Q11)

## Décision

- **Langues** : anglais, français, espagnol, allemand, italien, valencien, irlandais (les mêmes qu'Alpagamp),
  choisies selon la langue du système, modifiables dans les Options. Traductions faites par Claude ; une relecture
  par un locuteur natif est souhaitable pour le valencien et l'irlandais.
- **Encodage des messages** : UTF-8 par défaut. Une ligne reçue qui n'est pas de l'UTF-8 valide est lue en
  Windows-1252 (Latin-1 étendu), l'encodage des vieux canaux français. Réglage par réseau pour forcer un encodage.
