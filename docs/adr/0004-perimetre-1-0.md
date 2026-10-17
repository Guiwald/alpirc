# ADR 0004 — Périmètre de la 1.0

- Statut : acceptée (17/10/2026, grilling par lots, Q6, Q7, Q15)

## Décision : dans la 1.0

| Domaine | Contenu |
|---|---|
| Connexion | plusieurs réseaux à la fois ; liste des réseaux et serveurs ; TLS par défaut ; mot de passe serveur ; SASL (PLAIN, EXTERNAL, SCRAM-SHA-256) ; NickServ ; compte X d'Undernet (ADR 0010) ; pseudo de secours ; reconnexion automatique ; UTF-8 avec repli Latin-1/CP1252 |
| Fenêtres | Status (une par réseau), canaux, privés, liste des canaux (`/list`), Notify, URL catcher, fenêtres `@` des scripts |
| Canaux | sujet, modes, liste des pseudos (@ % + et autres préfixes), menus du clic droit (whois, privé, op, voice, kick, ban, kick+ban, CTCP, DCC), Channel Central (sujet, modes, listes de bans, d'exceptions et d'invitations) |
| Saisie | les commandes IRC usuelles ; historique ; complétion Tab des pseudos et canaux ; Ctrl+K/B/U/I/R/O ; garde-fou au collage de plusieurs lignes |
| Affichage | couleurs mIRC (16 + 99), gras, italique, souligné, barré, inverse, chasse fixe ; horodatage ; mots surlignés ; pseudos colorés (option) ; recherche ; emoji |
| Carnet d'adresses | Notify, Ignore, Control (auto-op, auto-voice, protection), niveaux d'utilisateurs pour les scripts |
| Automatismes | Perform à la connexion, auto-join, favoris (Channels Folder), away, réponses CTCP |
| DCC | CHAT, SEND/GET avec reprise, DCC passif |
| Journaux | fichiers texte par fenêtre, relecture |
| Alertes | bip, sons sur événements (`/splay`), notifications du système |
| Protection | anti-flood à l'envoi, anti-flood CTCP |
| Options | la grande boîte Options façon mIRC (arbre de catégories) |
| IRCv3 | CAP 302, SASL, server-time, message-tags, echo-message, away-notify, account-notify, extended-join, chghost, multi-prefix, userhost-in-names, batch, invite-notify, setname, MONITOR, STS, labeled-response |
| Bouncers | soju et ZNC : `soju.im/bouncer-networks`, `draft/chathistory`, `draft/read-marker` |
| Relais | notre propre relais (ADR 0009) |
| Notifications | Mac et Android, quand on vous nomme et pour les privés |
| Scripts | ADR 0007 |
| Langues | ADR 0012 |

## Plus tard

- 1.1 : proxy SOCKS ; sockets de script ; synchronisation des réglages et des scripts entre appareils ; indicateur
  « en train d'écrire » (IRCv3 typing) ; `/np` qui annonce le morceau joué par Alpagamp ; recherche dans tous les
  journaux.
- Jamais : identd (port 113 inutilisable derrière une box) ; DCC Server et Fserve ; DLL et COM ; `/run` ; Microsoft
  Agent ; aperçus de liens et d'images (pas l'esprit de mIRC, et cela donnerait l'adresse IP aux sites).
