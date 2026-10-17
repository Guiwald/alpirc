# ADR 0010 — Undernet et réseaux par défaut

- Statut : acceptée (17/10/2026, grilling par lots, Q10, Q19)

## Décision

1. **Liste des réseaux par défaut**, Undernet en tête : Undernet, Libera.Chat, OFTC, IRCnet, EFnet, DALnet,
   QuakeNet, Rizon, EpiKnet.
2. **Favoris préremplis** : #lyon, #grenoble, #chambery sur Undernet (les canaux de Guillaume à l'époque).
3. **Compte X d'Undernet** (Undernet n'a pas de NickServ) : champ « Compte Undernet » dans les réglages du réseau ;
   AlpIRC se connecte par Login on Connect (`+x!` dans le mot de passe serveur : connexion seulement si X est là,
   et masquage de l'adresse IP par `+x`). Le compte se crée sur le site d'Undernet, par l'utilisateur.
4. **Pseudo par défaut : Guiwald** (le pseudo de Guillaume à l'époque), modifiable à la première ouverture.
5. **Essais** : dans `#alpirc-test` sur Undernet et sur Libera, sous un pseudo de test ; jamais dans les vrais
   canaux sans Guillaume.

## Conséquences

- Les adresses des serveurs et la prise en charge de TLS par chaque réseau sont vérifiées en vrai depuis le Mac.
