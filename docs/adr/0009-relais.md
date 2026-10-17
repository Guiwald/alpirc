# ADR 0009 — Le relais d'AlpIRC (bouncer)

- Statut : acceptée (17/10/2026, grilling par lots, Q7, Q15, Q21)

## Contexte

Guillaume veut être connecté à la fois sur son téléphone et sur le client Mac, sans deux identifiants différents.
Sur IRC, c'est le rôle d'un **bouncer** : un programme qui tourne en permanence sur une machine allumée, reste
connecté aux réseaux sous un seul pseudo, et auquel les appareils se connectent. Le Mac de Guillaume ne peut pas
servir de relais (il dort, c'est un Mac professionnel) ; le serveur de Cullera n'existe pas encore.

## Décision

1. **Notre propre relais en Rust** (`alpirc-relay`), dans la 1.0. Il partage le code IRC du client.
   - Plusieurs appareils sur la même session : ce qui est écrit sur l'un apparaît sur l'autre ; les deux voient la
     même conversation ; le réseau ne voit qu'une connexion.
   - Compatible avec les extensions du bouncer soju (`soju.im/bouncer-networks`, `draft/chathistory`,
     `draft/read-marker`), pour que d'autres applis IRC puissent s'y connecter.
   - Historique gardé sur le relais ; rattrapage à la connexion.
   - Conçu pour plusieurs comptes, configuré pour un seul.
2. **Les clients restent utilisables sans relais** : connexion directe aux réseaux, comme mIRC.
3. **Mise en service : pas tout de suite.** Le relais est écrit et testé dans la 1.0, pas déployé. Ensuite, si le
   serveur de Cullera se fait attendre : un VPS (OVH VPS-1, environ 6,50 €/mois en 2026) avec un certificat propre
   au relais dont le client retient l'empreinte (comme SSH), puis un domaine et un certificat Let's Encrypt.
4. Le client AlpIRC sait aussi se servir de soju et de ZNC.

## Conséquences

- Un troisième programme à construire et à tester : paquet `.deb` et service systemd pour Linux.
- Essais de la 1.0 : relais lancé sur l'ordinateur d'essai, le client bureau et le téléphone connectés à lui.
