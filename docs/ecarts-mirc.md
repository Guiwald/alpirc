# AlpIRC face à mIRC 6.x : inventaire des écarts

Revue commencée le 19/10/2026 (session autonome, après la 0.10.0). Le but : qu'un habitué de mIRC 6.x retrouve ses
repères. Chaque élément est **présent**, **absent**, **différent** (présent mais pas comme dans mIRC), **fait** (écart
corrigé, avec la version) ou **hors périmètre** (ADR 0004 et 0007 : identd, DCC Server, Fserve, DLL/COM, `/run`,
Microsoft Agent, aperçus de liens ; sockets et proxy en 1.1 ; ce qui ne sert que sous Windows).

Ce qui demande un vrai choix est dans [Questions pour Guillaume](#questions-pour-guillaume), en fin de fichier.

## Sources et méthode

- L'aide de mIRC en ligne (`mirc.co.uk/help`, `mirc.doc`) : c'est celle de la 7.x, plus large que la 6.x.
- `versions.txt` de mIRC : la date d'apparition de chaque fonction, pour ne garder que ce qui existait en 6.35
  (17/10/2008). Exemples vérifiés ainsi : `/anick`, `/mnick`, `/tnick`, `/aop`, `/creq`, `/pdcc`, `/qmsg`,
  `/savebuf`, `/debug`, `/dns`, la boîte Mouse, la boîte Confirm, la section Ignore du DCC, les couleurs des
  pseudos du carnet d'adresses (5.9), le surlignage déplacé dans le carnet d'adresses (6.3). `/logview` (7.11) et
  les commandes de la 7.x sont écartés.
- Les menus par défaut de mIRC 6 (`popups.ini` d'origine) de mémoire : leur contenu exact n'est pas dans l'aide en
  ligne. Ce qui n'a pas pu être vérifié est marqué « (de mémoire) ».
- AlpIRC : le code de `main` (commandes : `crates/core/src/client/commands.rs` et le moteur mSL ; menus :
  `crates/shell/src/menus.rs` et `crates/core/src/client/defaults.rs` ; Options : `crates/shell/src/dialogs/`), et
  un passage de chaque commande de l'aide de mIRC dans AlpIRC (réponse « commande inconnue » ou non).

## Les trois points relevés le 18/10

| Point | État | Ce qui a été fait |
|---|---|---|
| Menus contextuels pas retraduits après un changement de langue | **fait (0.11.0)** | Les fichiers de menus par défaut que l'utilisateur n'a pas modifiés (reconnus dans les 7 langues et dans chaque version) sont réécrits dans la nouvelle langue et rechargés, au changement de langue et au démarrage. Un fichier modifié n'est jamais touché. |
| Notification du service : « Connecté à 2 réseaux » pour deux connexions au même réseau | **fait (0.12.0)** | Les réseaux sont comptés une fois : « Connecté à Undernet », « Connecté à Undernet (2 connexions) », « Connecté à 2 réseaux ». |
| Bouton ▲ (Historique) sans effet sur le Pixel 8 | **à voir avec Guillaume** | Relu : le bouton natif envoie `histmenu`, qui ouvre le menu « Historique » au-dessus du champ (« (vide) » s'il n'y a rien) ; la boucle de l'appli se réveille au moins à chaque tic, donc le menu devrait se dessiner. Sans le téléphone ni le pont adb, pas moyen de reproduire. Voir la question 1. |

## Menus

### Barre de menus

| Menu | mIRC 6.x | AlpIRC | État |
|---|---|---|---|
| Fichier | Connecter/Déconnecter, Choisir un serveur, Nouvelle fenêtre de serveur, Options (Alt+O), Quitter | idem, plus Absent | présent |
| Affichage | Barre de menus, Barre d'outils, Switchbar, Treebar, Police… | Barre d'outils, Switchbar, Treebar, Liste des pseudos, Thème, Agrandir/Réduire la police, Rechercher, Effacer | présent (la barre de menus ne se cache pas : elle est native sur Mac) |
| Favoris | Ajouter aux favoris, Organiser les favoris, la liste | idem | présent |
| Outils | Carnet d'adresses, Liste Notify, Liste des canaux, Liste des URL, Éditeur de scripts, Couleurs, Fichiers reçus, Journaux, Minuteur en ligne | Carnet d'adresses, Couleurs, Éditeur de scripts, Liste des canaux, URL, Notify, Envoyer, Discussion, Transferts, Journaux | présent ; « Fichiers reçus » : le dossier s'ouvre depuis Transferts ; « Minuteur en ligne » absent (peu utile) |
| Commandes | le menu `menubar` des popups par défaut : rejoindre/quitter un canal, privé, notice, whois, CTCP, absence, inviter, bannir, expulser, ignorer, modes, changer de pseudo, quitter IRC (de mémoire) | **vide** : les popups par défaut n'avaient pas de section `menubar` | **fait (0.12.0)** |
| Fenêtre | Cascade, Mosaïque, Réorganiser les icônes, Ordre, Groupe, auto-mosaïque | Cascade, Mosaïque horizontale/verticale, Réorganiser, Suivante, Fermer, la liste des fenêtres | présent ; Ordre, Groupe, auto-mosaïque absents (peu utiles) |
| Aide | Sommaire, Rechercher, liens du site, Mise à jour, À propos | Aide (README), À propos | présent (pas de fichier d'aide de mIRC à reprendre) |

### Menus du clic droit (popups par défaut)

| Menu | mIRC 6.x (de mémoire) | AlpIRC avant la revue | État |
|---|---|---|---|
| Status | Lusers, Motd, Time, Rejoindre un canal, Quitter | idem | présent |
| Canal | Modes du canal (`/channel`)… | Modes du canal, Dossier des canaux, Quitter le canal | présent |
| Privé | Info (`/uwho`), Whois, Query, Ignorer, CTCP, DCC | Info faisait un `/whois` ; pas de CTCP | **fait** : Info → User Central (0.11.0), sous-menu CTCP (0.12.0) |
| Liste des pseudos | Info (`/uwho`), Whois, Query, Control (Ignore, Unignore, Op, Deop, Voice, Devoice, Kick, Kick (why), Ban, Ban Kick, Ban Kick (why)), CTCP, DCC, Slap! | Info faisait un `/whois` ; Ignorer hors de Control, pas de « Ne plus ignorer » | **fait** : Info → User Central (0.11.0) ; Ignorer et Ne plus ignorer en tête de Control (0.12.0) |
| Menu de la fenêtre (icône en haut à gauche) | Background, Beeping, Buffer (effacer, enregistrer), Desktop, Editbox, Events, Flashing, Font, Logging, Nicklist, Position, Spacing, Timestamp, Track URLs | Restaurer, Réduire, Agrandir, Effacer, Journal, Fermer | différent : voir « Fenêtres » |
| Liste des canaux | un menu (rejoindre…) | pas de menu ; double-clic pour rejoindre | absent |
| Liste des URL | un menu (ouvrir, copier, effacer…) | pas de menu ; double-clic pour ouvrir | absent |

## Commandes

Toutes les commandes de l'aide de mIRC ont été tapées dans AlpIRC. Sont listées ici celles qui existaient en 6.35 et
qu'AlpIRC ne connaissait pas (les autres sont présentes, ou envoyées au serveur telles quelles comme dans mIRC).

| Commande | Rôle dans mIRC | État |
|---|---|---|
| `/uwho` | boîte User Central (ici : fiche whois + carnet d'adresses) | **fait (0.11.0)** (faisait un `/whois`) |
| `/anick`, `/mnick`, `/tnick` | pseudo de secours ; pseudo principal sans changer l'actuel ; pseudo temporaire | **fait (0.13.0)** |
| `/fullname`, `/emailaddr` | nom complet ; adresse e-mail (l'identifiant en vient) | `/fullname` : **fait (0.13.0)** ; `/emailaddr` : absent (AlpIRC a un identifiant, pas d'e-mail) |
| `/ajinvite` | rejoindre sur invitation, on/off | **fait (0.13.0)** |
| `/aop`, `/avoice`, `/protect` | listes d'auto-op, d'auto-voice, de protection (`on`/`off`, `-l`, `-r`, `-w`, type de masque) | **fait (0.13.0)**, avec les cases « Listes actives » de l'onglet Control |
| `/creq`, `/sreq` | réponse aux demandes de DCC CHAT, de DCC SEND | **fait (0.13.0)** |
| `/pdcc` | envoyer les blocs DCC sans attendre les accusés de réception | sans objet : AlpIRC le fait toujours |
| `/toolbar`, `/switchbar`, `/treebar` | montrer ou cacher les barres | **fait (0.13.0)** |
| `/qmsg`, `/qme` | message ou action dans tous les privés | **fait (0.13.0)** |
| `/savebuf` | enregistrer le texte d'une fenêtre dans un fichier | **fait (0.13.0)** (dans le dossier des scripts, comme `/write`) |
| `/findtext` | rechercher dans la fenêtre | différent : `/find` ouvre la boîte de recherche |
| `/dns` | adresse IP d'un hôte ou d'un pseudo ; nom d'une adresse IP | **fait (0.13.0)** pour l'hôte et le pseudo (répondait « non pris en charge ») ; le nom d'une adresse IP reste impossible (pas de résolution inverse sans bibliothèque C) |
| `/debug` | journal des lignes échangées avec le serveur | **fait (0.13.0)** : fenêtre `@` ou fichier du dossier des journaux ; les mots de passe sont masqués |
| `/remote` | scripts on/off | **fait (0.13.0)** |
| `/ctcps`, `/events`, `/ebeeps` | interrupteurs (CTCP des scripts, événements, sons) | absent, peu utiles |
| `/flood`, `/strip`, `/color`, `/cnick` | réglages en ligne de commande | absent ; `/cnick` dépend des couleurs de pseudos du carnet (voir plus bas) |
| `/clipboard` | copier dans le presse-papiers | **fait (0.13.0)** |
| `/editbox` | texte de la ligne de saisie (scripts) | absent |
| `/play` | lire un fichier texte dans un canal, ligne à ligne | absent → question 4 |
| `/dqwindow`, `/fsend`, `/localinfo`, `/titlebar`, `/updatenl`, `/groups`, `/ulist` | fenêtre de messages unique ; envoi DCC rapide ; adresse locale ; titre ; liste des pseudos ; groupes et utilisateurs des scripts | absent, peu utile |
| fichiers binaires (`/bread`…), fichiers ouverts (`/fopen`…), images (`/draw…`), `/filter`, `/loadbuf`, `/speak`, `/vol`… | scripts avancés | absent : hors ADR 0007 pour la 1.0 (question 5) |
| `/dccserver`, `/fserve`, `/dll`, `/com…`, `/run`, `/dde`, `/tray`, `/showmirc`, `/setlayer`, `/tips`, `/background`, `/proxy`, `/sock…` | | hors périmètre |
| `/nick` | change le pseudo **et** le pseudo des Options (`/tnick` pour un changement temporaire) | **différent** : temporaire dans AlpIRC → question 2 |

## Options

| Page de mIRC 6.x | AlpIRC | État |
|---|---|---|
| Connect › Servers | Connexion › Serveurs (réseaux, serveurs, identité par réseau, Perform) | présent |
| Connect › Options (reconnexion, connexion au démarrage, nouvelle fenêtre…) | Connexion › Options | présent |
| Dossier des canaux ouvert à la connexion (case « Pop up favorites on connect ») | rien | **fait (0.12.0)** : case « Ouvrir à la connexion » du dossier ; cochée sur l'ordinateur, décochée sur le téléphone (question 3) |
| Connect › Identd, Proxy | | hors périmètre (identd jamais ; proxy en 1.1) |
| Connect › Local Info | Options › DCC › adresse annoncée | différent (suffit au DCC) |
| IRC (options générales), Messages, Highlight, Flood, Logging, Catcher | IRC, Messages et CTCP, Surlignage, Anti-flood, Journaux, URL | présent |
| Sounds, Requests | Sons (par événement, demandes de sons) | présent ; Agent : hors périmètre |
| Mouse (actions du double-clic par fenêtre) | double-clic fixe (Status : rien ; canal : Channel Central ; pseudo : privé ; Notify : whois) | absent → question 6 |
| Display, Lines | Affichage (thème, police, barres, langue) | présent ; espacement des lignes absent |
| Other › Confirm | collage de plusieurs lignes, quitter avec des connexions ouvertes | présent en partie |
| Other › Lock | | hors périmètre (le bac à sable des scripts remplace le verrou) |
| DCC (demandes, dossier, ports, passif, délais) | Options › DCC | présent |
| DCC › Folders (un dossier par type de fichier) | un seul dossier | absent, peu utile |
| DCC › Ignore (types de fichiers refusés : `*.exe *.com *.bat *.dll *.ini *.mrc *.vbs *.js *.pif *.scr *.lnk *.pl *.shs *.htm *.html`) | une liste fixe de types à risque, toujours demandés même en acceptation automatique | différent (suffit) |
| DCC › Server, Fserve | | hors périmètre |

## Carnet d'adresses

| Partie | mIRC 6.x | AlpIRC | État |
|---|---|---|---|
| Info (nom réel, e-mail, site, notes, image) + fiche `/uwho` | | onglet Infos + User Central | **fait (0.11.0)** ; l'image n'est pas reprise |
| Notify | | onglet Notify | présent |
| Control (auto-op, auto-voice, protection, ignore) | | onglets Ignore et Control | présent |
| Colors (couleur d'un pseudo, 5.9) | | « Pseudos colorés » (couleur calculée) seulement | absent → question 7 |
| Highlight (dans le carnet depuis la 6.3) | | dans Options › Surlignage (comme mIRC 6.0 à 6.2) | présent |

## Liste des canaux

| Élément | mIRC 6.x | AlpIRC | État |
|---|---|---|---|
| Boîte de recherche (texte, nombre de personnes, tri) | | « Liste des canaux » | présent |
| Fenêtre de la liste, tri par colonne, double-clic pour rejoindre | | idem | présent |
| Menu du clic droit | rejoindre… | aucun | absent → lot « fenêtres et DCC » |
| Liste gardée d'une fois sur l'autre | oui | le temps de la session | différent, peu gênant |

## Capteur d'URL

| Élément | mIRC 6.x | AlpIRC | État |
|---|---|---|---|
| Liste des adresses vues, avec qui et où | | fenêtre URL | présent |
| Ouvrir (double-clic) | | idem | présent |
| Menu : ouvrir, copier, retirer, tout effacer | | aucun | absent → lot « fenêtres et DCC » |

## DCC

| Élément | mIRC 6.x | AlpIRC | État |
|---|---|---|---|
| DCC CHAT, SEND/GET, reprise, passif | | | présent |
| Boîtes d'envoi et de discussion, fenêtre des transferts | | | présent |
| Demande d'accord à la réception, choix si le fichier existe | | | présent |
| Types de fichiers ignorés (`/dcc ignore`) | | `/dcc ignore` refuse une offre ; types à risque toujours demandés | différent (suffit) |
| DCC Server, Fserve | | | hors périmètre |

## Fenêtres

| Élément du menu de la fenêtre (mIRC) | AlpIRC | État |
|---|---|---|
| Buffer › Effacer | Effacer | présent |
| Buffer › Enregistrer | rien | absent → lot « fenêtres et DCC » (avec `/savebuf`) |
| Logging | Journal | présent |
| Timestamp, Beeping, Flashing, Font, Events (réglages propres à une fenêtre) | réglages communs à toutes les fenêtres | différent → question 8 |
| Background, Desktop, Position, Spacing, Editbox (seconde ligne de saisie), Nicklist (position), Track URLs | | absent, peu utiles |

## Fait pendant la revue

- **0.11.0** : User Central (`/uwho`, Infos), onglet Infos du carnet d'adresses, Whois dans la fenêtre active sur
  Android, menus par défaut qui suivent la langue.
- **0.12.0** : menu Commandes, Control avec Ignorer / Ne plus ignorer, CTCP dans le menu d'un privé, dossier des
  canaux ouvert à la connexion, notification du service qui compte les réseaux.
- **0.13.0** : `/anick`, `/mnick`, `/tnick`, `/fullname`, `/ajinvite`, `/aop`, `/avoice`, `/protect` (et les cases
  « Listes actives »), `/creq`, `/sreq`, `/remote`, `/toolbar`, `/switchbar`, `/treebar`, `/qmsg`, `/qme`,
  `/clipboard`, `/savebuf`, `/dns`, `/debug`.

## Questions pour Guillaume

Rien n'est bloqué : chaque question a une réponse par défaut, appliquée ou laissée telle quelle.

1. **Bouton ▲ sur le Pixel 8.** Dans quelle fenêtre l'as-tu touché (Status, un canal où tu avais déjà écrit, un
   privé) ? Clavier ouvert ou fermé ? Le menu « Historique » n'apparaît-il jamais, ou apparaît-il un instant ? Si
   le pont adb peut tourner, une session pourra chercher avec `logcat`. *Par défaut : rien de changé.*
2. **`/nick`** : dans mIRC, il change aussi le pseudo des Options (celui de la prochaine connexion) ; `/tnick` le
   change pour cette fois seulement. Dans AlpIRC, `/nick` est temporaire (comme `/tnick`). Faire comme mIRC ?
   Avec les pseudos par réseau d'AlpIRC, il faudrait choisir lequel changer. *Par défaut : `/nick` reste
   temporaire ; `/mnick` (lot « commandes ») change le pseudo des Options.*
3. **Dossier des canaux à la connexion sur le téléphone** : coché sur l'ordinateur comme dans mIRC ; décoché sur le
   téléphone (une boîte à chaque connexion y gêne). Le vouloir aussi sur le téléphone ? *Par défaut : non.*
4. **`/play`** (lire un fichier texte dans un canal) : à ajouter ? Il sert surtout aux scripts de « quiz » et aux
   ASCII art ; risque de flood si mal réglé. *Par défaut : non, pas en 1.0.*
5. **Scripts avancés** (fichiers binaires, `/fopen`, fenêtres d'images `/draw…`) : hors ADR 0007 pour la 1.0.
   Certains vieux scripts en ont besoin. Les prévoir en 1.1 ? *Par défaut : non.*
6. **Options › Souris** (choisir la commande du double-clic dans Status, un canal, un privé, la liste des pseudos,
   Notify) : à ajouter ? Utile surtout aux habitués qui avaient changé ces réglages. *Par défaut : non, les
   double-clics de mIRC par défaut sont déjà ceux d'AlpIRC.*
7. **Couleurs des pseudos du carnet d'adresses** (choisir la couleur d'un pseudo, `/cnick`) : AlpIRC ne sait que
   colorer tous les pseudos automatiquement. À ajouter ? *Par défaut : non.*
8. **Réglages par fenêtre** (horodatage, bip, clignotement, police propres à un canal) : AlpIRC n'a que des réglages
   communs. À ajouter ? *Par défaut : non.*
