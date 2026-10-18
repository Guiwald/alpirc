//! Textes de l'interface (menus, boîtes de dialogue), dans les sept langues d'AlpIRC (ADR 0012).
//!
//! Chaque entrée : clé, puis anglais, français, espagnol, allemand, italien, valencien, irlandais. Une langue
//! vide se replie sur l'anglais.

use alpirc_core::i18n::Lang;

/// (clé, [en, fr, es, de, it, va, ga])
pub(crate) const TABLE: &[(&str, [&str; 7])] = &[
    // Menus.
    ("menu.file", ["&File", "&Fichier", "&Archivo", "&Datei", "&File", "&Fitxer", "&Comhad"]),
    ("menu.view", ["&View", "&Affichage", "&Ver", "&Ansicht", "&Visualizza", "&Visualització", "&Amharc"]),
    ("menu.favorites", ["F&avorites", "Fa&voris", "&Favoritos", "Fa&voriten", "&Preferiti", "&Preferits", "C&eanáin"]),
    ("menu.tools", ["&Tools", "&Outils", "&Herramientas", "E&xtras", "S&trumenti", "&Eines", "&Uirlisí"]),
    ("menu.commands", ["&Commands", "&Commandes", "&Comandos", "&Befehle", "&Comandi", "&Ordres", "&Orduithe"]),
    ("menu.window", ["&Window", "Fe&nêtre", "Ve&ntana", "&Fenster", "F&inestra", "Fi&nestra", "&Fuinneog"]),
    ("menu.help", ["&Help", "Ai&de", "Ay&uda", "&Hilfe", "&Aiuto", "&Ajuda", "Ca&bhair"]),
    (
        "file.new_status",
        [
            "&New Server Window",
            "&Nouvelle fenêtre de serveur",
            "&Nueva ventana de servidor",
            "&Neues Serverfenster",
            "&Nuova finestra server",
            "&Nova finestra de servidor",
            "Fuinneog &nua freastalaí",
        ],
    ),
    ("file.connect", ["&Connect", "Se &connecter", "&Conectar", "&Verbinden", "&Connetti", "&Connectar", "&Ceangail"]),
    (
        "file.disconnect",
        ["&Disconnect", "Se &déconnecter", "&Desconectar", "&Trennen", "&Disconnetti", "&Desconnectar", "&Dícheangail"],
    ),
    (
        "file.select_server",
        [
            "Select &Server...",
            "Choisir un &serveur...",
            "Elegir &servidor...",
            "&Server auswählen...",
            "Scegli &server...",
            "Triar &servidor...",
            "Roghnaigh &freastalaí...",
        ],
    ),
    ("file.away", ["&Away", "&Absent", "&Ausente", "&Abwesend", "&Assente", "&Absent", "&As láthair"]),
    (
        "file.options",
        ["&Options...", "&Options...", "&Opciones...", "&Optionen...", "&Opzioni...", "&Opcions...", "R&oghanna..."],
    ),
    ("file.exit", ["E&xit", "&Quitter", "Sa&lir", "&Beenden", "&Esci", "&Eixir", "&Scoir"]),
    (
        "view.toolbar",
        [
            "&Toolbar",
            "Barre d'&outils",
            "&Barra de herramientas",
            "&Symbolleiste",
            "&Barra degli strumenti",
            "&Barra d'eines",
            "&Barra uirlisí",
        ],
    ),
    (
        "view.switchbar",
        ["&Switchbar", "&Switchbar", "&Switchbar", "S&witchbar", "&Switchbar", "&Switchbar", "&Switchbar"],
    ),
    ("view.treebar", ["T&reebar", "T&reebar", "T&reebar", "T&reebar", "T&reebar", "T&reebar", "T&reebar"]),
    (
        "view.nicklist",
        [
            "&Nickname List",
            "&Liste des pseudos",
            "Lista de &nicks",
            "&Nickliste",
            "Lista dei &nick",
            "&Llista de sobrenoms",
            "&Liosta leasainmneacha",
        ],
    ),
    ("view.theme", ["T&heme", "T&hème", "&Tema", "&Design", "&Tema", "&Tema", "&Téama"]),
    (
        "view.theme.classic",
        ["&Classic", "&Classique", "&Clásico", "&Klassisch", "&Classico", "&Clàssic", "&Clasaiceach"],
    ),
    ("view.theme.dark", ["&Dark", "&Sombre", "&Oscuro", "&Dunkel", "&Scuro", "&Fosc", "&Dorcha"]),
    (
        "view.font_bigger",
        [
            "&Larger Font",
            "Police plus &grande",
            "Fuente más &grande",
            "&Größere Schrift",
            "Carattere più &grande",
            "Lletra més &gran",
            "Cló níos &mó",
        ],
    ),
    (
        "view.font_smaller",
        [
            "S&maller Font",
            "Police plus &petite",
            "Fuente más &pequeña",
            "&Kleinere Schrift",
            "Carattere più &piccolo",
            "Lletra més &menuda",
            "Cló &níos lú",
        ],
    ),
    (
        "view.find",
        [
            "&Find Text...",
            "Re&chercher...",
            "Bus&car texto...",
            "Text s&uchen...",
            "Tro&va testo...",
            "B&uscar text...",
            "&Aimsigh téacs...",
        ],
    ),
    (
        "view.clear",
        [
            "Cl&ear Window",
            "&Effacer la fenêtre",
            "&Limpiar ventana",
            "Fenster &leeren",
            "&Cancella finestra",
            "&Netejar la finestra",
            "&Glan an fhuinneog",
        ],
    ),
    (
        "fav.folder",
        [
            "&Channels Folder...",
            "&Dossier des canaux...",
            "&Carpeta de canales...",
            "&Channel-Ordner...",
            "&Cartella dei canali...",
            "&Carpeta de canals...",
            "&Fillteán cainéal...",
        ],
    ),
    (
        "fav.add",
        [
            "&Add to Favorites",
            "&Ajouter aux favoris",
            "&Añadir a favoritos",
            "Zu &Favoriten hinzufügen",
            "&Aggiungi ai preferiti",
            "&Afegir als preferits",
            "&Cuir leis na ceanáin",
        ],
    ),
    (
        "tools.abook",
        [
            "&Address Book...",
            "&Carnet d'adresses...",
            "&Libreta de direcciones...",
            "&Adressbuch...",
            "&Rubrica...",
            "&Llibreta d'adreces...",
            "&Leabhar seoltaí...",
        ],
    ),
    (
        "tools.colors",
        ["C&olors...", "C&ouleurs...", "C&olores...", "&Farben...", "C&olori...", "C&olors...", "&Dathanna..."],
    ),
    (
        "tools.scripts",
        [
            "Scripts &Editor...",
            "Éditeur de &scripts...",
            "&Editor de scripts...",
            "Script-&Editor...",
            "&Editor di script...",
            "&Editor de scripts...",
            "&Eagarthóir scripteanna...",
        ],
    ),
    (
        "tools.chanlist",
        [
            "Channels &List...",
            "&Liste des canaux...",
            "Lista de &canales...",
            "&Channelliste...",
            "&Lista dei canali...",
            "Llista de &canals...",
            "Liosta &cainéal...",
        ],
    ),
    (
        "tools.urls",
        [
            "&URL Catcher",
            "Adresses &attrapées",
            "Capturador de &URL",
            "&URL-Sammler",
            "&URL catturati",
            "Capturador d'&URL",
            "Bailitheoir &URL",
        ],
    ),
    (
        "tools.notify",
        [
            "&Notify List",
            "Liste &Notify",
            "Lista &Notify",
            "&Notify-Liste",
            "Lista &Notify",
            "Llista &Notify",
            "Liosta &Notify",
        ],
    ),
    (
        "tools.dcc_send",
        [
            "DCC &Send...",
            "DCC : &envoyer un fichier...",
            "DCC: en&viar archivo...",
            "DCC &Send...",
            "DCC: in&via file...",
            "DCC: en&viar fitxer...",
            "DCC: &seol comhad...",
        ],
    ),
    (
        "tools.dcc_chat",
        [
            "DCC &Chat...",
            "DCC : &discussion...",
            "DCC: c&hat...",
            "DCC C&hat...",
            "DCC: &chat...",
            "DCC: &xat...",
            "DCC: c&omhrá...",
        ],
    ),
    (
        "tools.transfers",
        [
            "DCC &Transfers",
            "DCC : &transferts",
            "DCC: &transferencias",
            "DCC-Ü&bertragungen",
            "DCC: &trasferimenti",
            "DCC: &transferències",
            "DCC: &aistrithe",
        ],
    ),
    (
        "tools.logs",
        [
            "Log &Files...",
            "&Journaux...",
            "&Archivos de registro...",
            "&Logdateien...",
            "&File di log...",
            "&Fitxers de registre...",
            "Lo&gchomhaid...",
        ],
    ),
    ("win.cascade", ["&Cascade", "&Cascade", "&Cascada", "Ü&berlappend", "&Sovrapponi", "&Cascada", "&Cascáid"]),
    (
        "win.tile_h",
        [
            "Tile &Horizontal",
            "Mosaïque &horizontale",
            "Mosaico &horizontal",
            "&Untereinander",
            "Affianca &orizzontalmente",
            "Mosaic &horitzontal",
            "Tíleáil c&hothrománach",
        ],
    ),
    (
        "win.tile_v",
        [
            "Tile &Vertical",
            "Mosaïque &verticale",
            "Mosaico &vertical",
            "&Nebeneinander",
            "Affianca &verticalmente",
            "Mosaic &vertical",
            "Tíleáil &ingearach",
        ],
    ),
    (
        "win.arrange",
        [
            "&Arrange Icons",
            "&Réorganiser les icônes",
            "Organizar &iconos",
            "Symbole &anordnen",
            "Disponi &icone",
            "Organitzar &icones",
            "Socraigh &deilbhíní",
        ],
    ),
    (
        "win.next",
        [
            "&Next Window",
            "Fenêtre &suivante",
            "Ventana &siguiente",
            "Nä&chstes Fenster",
            "Finestra s&uccessiva",
            "Finestra &següent",
            "An chéad fhuinneog &eile",
        ],
    ),
    (
        "win.close",
        [
            "Cl&ose Window",
            "&Fermer la fenêtre",
            "C&errar ventana",
            "Fenster &schließen",
            "&Chiudi finestra",
            "&Tancar la finestra",
            "Dú&n an fhuinneog",
        ],
    ),
    (
        "help.readme",
        [
            "AlpIRC &Help",
            "&Aide d'AlpIRC",
            "&Ayuda de AlpIRC",
            "AlpIRC-&Hilfe",
            "&Guida di AlpIRC",
            "&Ajuda d'AlpIRC",
            "&Cabhair AlpIRC",
        ],
    ),
    (
        "help.about",
        [
            "&About AlpIRC...",
            "À &propos d'AlpIRC...",
            "Acerca &de AlpIRC...",
            "Ü&ber AlpIRC...",
            "&Informazioni su AlpIRC...",
            "&Quant a AlpIRC...",
            "&Maidir le AlpIRC...",
        ],
    ),
    ("menu.empty", ["(none)", "(aucun)", "(ninguno)", "(keine)", "(nessuno)", "(cap)", "(dada)"]),
    // Menu de la switchbar et de la fenêtre.
    ("swb.close", ["&Close", "&Fermer", "&Cerrar", "&Schließen", "&Chiudi", "Tan&car", "&Dún"]),
    (
        "swb.minimize",
        ["Mi&nimize", "&Réduire", "Mi&nimizar", "Mi&nimieren", "Ri&duci a icona", "Mi&nimitzar", "Ío&slaghdaigh"],
    ),
    (
        "swb.maximize",
        ["Ma&ximize", "&Agrandir", "Ma&ximizar", "Ma&ximieren", "I&ngrandisci", "Ma&ximitzar", "&Uasmhéadaigh"],
    ),
    (
        "swb.restore",
        ["&Restore", "Res&taurer", "&Restaurar", "&Wiederherstellen", "&Ripristina", "&Restaurar", "&Athchóirigh"],
    ),
    ("swb.log", ["&Logging", "&Journal", "Re&gistro", "L&ogging", "&Log", "Re&gistre", "&Logáil"]),
    ("swb.clear", ["Cl&ear", "&Effacer", "&Limpiar", "&Leeren", "&Svuota", "Ne&tejar", "&Glan"]),
    (
        "swb.save",
        [
            "&Save Buffer...",
            "Enre&gistrer le texte...",
            "Guar&dar el texto...",
            "Text s&peichern...",
            "Sal&va il testo...",
            "Guar&dar el text...",
            "Sá&bháil an téacs...",
        ],
    ),
    ("swb.copy", ["&Copy", "&Copier", "&Copiar", "&Kopieren", "&Copia", "&Copiar", "&Cóipeáil"]),
    (
        "swb.select_all",
        [
            "Select &All",
            "&Tout sélectionner",
            "Seleccionar &todo",
            "&Alles auswählen",
            "Seleziona &tutto",
            "Seleccionar-ho &tot",
            "Roghnaigh &gach rud",
        ],
    ),
    // Barre d'outils.
    ("tb.connect", ["Connect", "Se connecter", "Conectar", "Verbinden", "Connetti", "Connectar", "Ceangail"]),
    (
        "tb.disconnect",
        ["Disconnect", "Se déconnecter", "Desconectar", "Trennen", "Disconnetti", "Desconnectar", "Dícheangail"],
    ),
    ("tb.away", ["Away", "Absent", "Ausente", "Abwesend", "Assente", "Absent", "As láthair"]),
    ("tb.options", ["Options", "Options", "Opciones", "Optionen", "Opzioni", "Opcions", "Roghanna"]),
    (
        "tb.favorites",
        [
            "Channels Folder",
            "Dossier des canaux",
            "Carpeta de canales",
            "Channel-Ordner",
            "Cartella dei canali",
            "Carpeta de canals",
            "Fillteán cainéal",
        ],
    ),
    (
        "tb.chanlist",
        [
            "List Channels",
            "Liste des canaux",
            "Lista de canales",
            "Channelliste",
            "Lista dei canali",
            "Llista de canals",
            "Liosta cainéal",
        ],
    ),
    (
        "tb.scripts",
        [
            "Scripts Editor",
            "Éditeur de scripts",
            "Editor de scripts",
            "Script-Editor",
            "Editor di script",
            "Editor de scripts",
            "Eagarthóir scripteanna",
        ],
    ),
    (
        "tb.abook",
        [
            "Address Book",
            "Carnet d'adresses",
            "Libreta de direcciones",
            "Adressbuch",
            "Rubrica",
            "Llibreta d'adreces",
            "Leabhar seoltaí",
        ],
    ),
    (
        "tb.dcc_send",
        [
            "DCC Send",
            "Envoyer un fichier",
            "Enviar archivo (DCC)",
            "Datei senden (DCC)",
            "Invia file (DCC)",
            "Enviar fitxer (DCC)",
            "Seol comhad (DCC)",
        ],
    ),
    ("tb.dcc_chat", ["DCC Chat", "Discussion DCC", "Chat DCC", "DCC-Chat", "Chat DCC", "Xat DCC", "Comhrá DCC"]),
    (
        "tb.transfers",
        [
            "DCC Transfers",
            "Transferts DCC",
            "Transferencias DCC",
            "DCC-Übertragungen",
            "Trasferimenti DCC",
            "Transferències DCC",
            "Aistrithe DCC",
        ],
    ),
    (
        "tb.notify",
        [
            "Notify List",
            "Liste Notify",
            "Lista Notify",
            "Notify-Liste",
            "Lista Notify",
            "Llista Notify",
            "Liosta Notify",
        ],
    ),
    (
        "tb.urls",
        [
            "URL Catcher",
            "Adresses attrapées",
            "Capturador de URL",
            "URL-Sammler",
            "URL catturati",
            "Capturador d'URL",
            "Bailitheoir URL",
        ],
    ),
    ("tb.cascade", ["Cascade", "Cascade", "Cascada", "Überlappend", "Sovrapponi", "Cascada", "Cascáid"]),
    (
        "tb.tile_h",
        [
            "Tile Horizontal",
            "Mosaïque horizontale",
            "Mosaico horizontal",
            "Untereinander",
            "Affianca orizzontalmente",
            "Mosaic horitzontal",
            "Tíleáil chothrománach",
        ],
    ),
    (
        "tb.tile_v",
        [
            "Tile Vertical",
            "Mosaïque verticale",
            "Mosaico vertical",
            "Nebeneinander",
            "Affianca verticalmente",
            "Mosaic vertical",
            "Tíleáil ingearach",
        ],
    ),
    ("tb.colors", ["Colors", "Couleurs", "Colores", "Farben", "Colori", "Colors", "Dathanna"]),
    ("tb.help", ["Help", "Aide", "Ayuda", "Hilfe", "Guida", "Ajuda", "Cabhair"]),
    // Boîtes.
    ("dlg.ok", ["OK", "OK", "Aceptar", "OK", "OK", "D'acord", "OK"]),
    ("dlg.cancel", ["Cancel", "Annuler", "Cancelar", "Abbrechen", "Annulla", "Cancel·lar", "Cealaigh"]),
    ("dlg.close", ["Close", "Fermer", "Cerrar", "Schließen", "Chiudi", "Tancar", "Dún"]),
    ("dlg.yes", ["&Yes", "&Oui", "&Sí", "&Ja", "&Sì", "&Sí", "&Tá"]),
    ("dlg.no", ["&No", "&Non", "&No", "&Nein", "&No", "&No", "&Níl"]),
    ("paste.title", ["Paste", "Coller", "Pegar", "Einfügen", "Incolla", "Apegar", "Greamaigh"]),
    (
        "paste.text",
        [
            "You are about to paste {n} lines into {win}. Send them?",
            "Vous allez coller {n} lignes dans {win}. Les envoyer ?",
            "Vas a pegar {n} líneas en {win}. ¿Enviarlas?",
            "Sie fügen {n} Zeilen in {win} ein. Senden?",
            "Stai per incollare {n} righe in {win}. Inviarle?",
            "Esteu a punt d'apegar {n} línies en {win}. Voleu enviar-les?",
            "Tá tú ar tí {n} líne a ghreamú i {win}. Iad a sheoladh?",
        ],
    ),
    ("paste.send", ["&Send", "&Envoyer", "&Enviar", "&Senden", "&Invia", "&Enviar", "&Seol"]),
    (
        "paste.edit",
        [
            "Send as &one line",
            "En &une seule ligne",
            "En &una línea",
            "Als &eine Zeile",
            "In &una riga",
            "En &una línia",
            "Mar &líne amháin",
        ],
    ),
    (
        "join.title",
        [
            "Join Channel",
            "Rejoindre un canal",
            "Entrar en un canal",
            "Channel betreten",
            "Entra in un canale",
            "Entrar en un canal",
            "Téigh isteach i gcainéal",
        ],
    ),
    (
        "join.prompt",
        [
            "Enter the name of the channel to join:",
            "Nom du canal à rejoindre :",
            "Nombre del canal al que entrar:",
            "Name des Channels, den Sie betreten möchten:",
            "Nome del canale in cui entrare:",
            "Nom del canal on voleu entrar:",
            "Ainm an chainéil le dul isteach ann:",
        ],
    ),
    (
        "about.title",
        [
            "About AlpIRC",
            "À propos d'AlpIRC",
            "Acerca de AlpIRC",
            "Über AlpIRC",
            "Informazioni su AlpIRC",
            "Quant a AlpIRC",
            "Maidir le AlpIRC",
        ],
    ),
    (
        "about.text",
        [
            "AlpIRC {v}\nThe IRC client of the 2000s, redone from scratch in Rust.\n\nMIT License — https://github.com/Guiwald/alpirc\nNot affiliated with mIRC or mIRC Co. Ltd.\n\nFonts: Fixedsys Excelsior (public domain), Pixel Operator (CC0).",
            "AlpIRC {v}\nLe client IRC des années 2000, refait de zéro en Rust.\n\nLicence MIT — https://github.com/Guiwald/alpirc\nSans lien avec mIRC ni mIRC Co. Ltd.\n\nPolices : Fixedsys Excelsior (domaine public), Pixel Operator (CC0).",
            "AlpIRC {v}\nEl cliente IRC de los años 2000, rehecho desde cero en Rust.\n\nLicencia MIT: \
             https://github.com/Guiwald/alpirc\nSin relación con mIRC ni con mIRC Co. Ltd.\n\nFuentes: Fixedsys \
             Excelsior (dominio público), Pixel Operator (CC0).",
            "AlpIRC {v}\nDer IRC-Client der 2000er, in Rust von Grund auf neu geschrieben.\n\nMIT-Lizenz — \
             https://github.com/Guiwald/alpirc\nKeine Verbindung zu mIRC oder mIRC Co. Ltd.\n\nSchriften: Fixedsys \
             Excelsior (gemeinfrei), Pixel Operator (CC0).",
            "AlpIRC {v}\nIl client IRC degli anni 2000, rifatto da zero in Rust.\n\nLicenza MIT — \
             https://github.com/Guiwald/alpirc\nNon affiliato a mIRC né a mIRC Co. Ltd.\n\nCaratteri: Fixedsys \
             Excelsior (pubblico dominio), Pixel Operator (CC0).",
            "AlpIRC {v}\nEl client IRC dels anys 2000, refet des de zero en Rust.\n\nLlicència MIT \
             https://github.com/Guiwald/alpirc\nSense relació amb mIRC ni amb mIRC Co. Ltd.\n\nTipus de lletra: \
             Fixedsys Excelsior (domini públic), Pixel Operator (CC0).",
            "AlpIRC {v}\nCliant IRC na 2000idí, athdhéanta ón tús in Rust.\n\nCeadúnas MIT \
             https://github.com/Guiwald/alpirc\nGan aon bhaint le mIRC ná le mIRC Co. Ltd.\n\nClónna: Fixedsys \
             Excelsior (fearann poiblí), Pixel Operator (CC0).",
        ],
    ),
    (
        "find.title",
        ["Find Text", "Rechercher", "Buscar texto", "Text suchen", "Trova testo", "Buscar text", "Aimsigh téacs"],
    ),
    ("find.prompt", ["Find:", "Rechercher :", "Buscar:", "Suchen:", "Trova:", "Buscar:", "Aimsigh:"]),
    ("find.next", ["&Next", "&Suivant", "&Siguiente", "&Weiter", "&Successivo", "&Següent", "Ar &aghaidh"]),
    ("find.prev", ["&Previous", "&Précédent", "&Anterior", "&Zurück", "&Precedente", "&Anterior", "&Siar"]),
    (
        "quit.title",
        [
            "Exit AlpIRC",
            "Quitter AlpIRC",
            "Salir de AlpIRC",
            "AlpIRC beenden",
            "Esci da AlpIRC",
            "Eixir d'AlpIRC",
            "Scoir AlpIRC",
        ],
    ),
    // Menu AlpIRC de macOS : « Quitter » selon les usages du Mac (et non « Exit »).
    (
        "mac.quit",
        [
            "Quit AlpIRC",
            "Quitter AlpIRC",
            "Salir de AlpIRC",
            "AlpIRC beenden",
            "Esci da AlpIRC",
            "Eixir d'AlpIRC",
            "Scoir AlpIRC",
        ],
    ),
    (
        "quit.text",
        [
            "You are connected to {n} server(s). Quit anyway?",
            "Vous êtes connecté à {n} serveur(s). Quitter quand même ?",
            "Estás conectado a {n} servidor(es). ¿Salir de todos modos?",
            "Sie sind mit {n} Server(n) verbunden. Trotzdem beenden?",
            "Sei connesso a {n} server. Uscire comunque?",
            "Esteu connectat a {n} servidor(s). Voleu eixir igualment?",
            "Tá tú ceangailte le {n} freastalaí. Scoir mar sin féin?",
        ],
    ),
    ("chanlist.name", ["Channel", "Canal", "Canal", "Channel", "Canale", "Canal", "Cainéal"]),
    ("chanlist.users", ["Users", "Pers.", "Usuarios", "Nutzer", "Utenti", "Usuaris", "Daoine"]),
    ("chanlist.topic", ["Topic", "Sujet", "Tema", "Topic", "Topic", "Tema", "Ábhar"]),
    ("urls.url", ["Address", "Adresse", "Dirección", "Adresse", "Indirizzo", "Adreça", "Seoladh"]),
    ("urls.nick", ["Nick", "Pseudo", "Nick", "Nick", "Nick", "Sobrenom", "Leasainm"]),
    ("urls.where", ["Window", "Fenêtre", "Ventana", "Fenster", "Finestra", "Finestra", "Fuinneog"]),
    ("urls.time", ["Time", "Heure", "Hora", "Zeit", "Ora", "Hora", "Am"]),
    ("notify.nick", ["Nick", "Pseudo", "Nick", "Nick", "Nick", "Sobrenom", "Leasainm"]),
    ("notify.state", ["State", "État", "Estado", "Status", "Stato", "Estat", "Staid"]),
    ("notify.note", ["Note", "Note", "Nota", "Notiz", "Nota", "Nota", "Nóta"]),
    ("notify.online", ["online", "en ligne", "conectado", "online", "online", "connectat", "ar líne"]),
    ("notify.offline", ["offline", "absent", "desconectado", "offline", "offline", "desconnectat", "as líne"]),
    (
        "win.chanlist",
        [
            "Channels List",
            "Liste des canaux",
            "Lista de canales",
            "Channelliste",
            "Lista dei canali",
            "Llista de canals",
            "Liosta cainéal",
        ],
    ),
    (
        "win.urls",
        [
            "URL Catcher",
            "Adresses attrapées",
            "Capturador de URL",
            "URL-Sammler",
            "URL catturati",
            "Capturador d'URL",
            "Bailitheoir URL",
        ],
    ),
    (
        "win.notify",
        [
            "Notify List",
            "Liste Notify",
            "Lista Notify",
            "Notify-Liste",
            "Lista Notify",
            "Llista Notify",
            "Liosta Notify",
        ],
    ),
];

fn column(lang: Lang) -> usize {
    match lang {
        Lang::En => 0,
        Lang::Fr => 1,
        Lang::Es => 2,
        Lang::De => 3,
        Lang::It => 4,
        Lang::Va => 5,
        Lang::Ga => 6,
    }
}

/// Une table de textes : (clé, [en, fr, es, de, it, va, ga]).
pub type Table = &'static [(&'static str, [&'static str; 7])];

/// Toutes les tables : celle-ci, puis celles des modules (boîtes de dialogue, DCC, téléphone).
pub(crate) const TABLES: &[Table] = &[TABLE, crate::dialogs::TR, crate::dcc::TR, crate::mobile::TR, crate::notify::TR, crate::lists::TR];

fn find(key: &str) -> Option<&'static [&'static str; 7]> {
    TABLES.iter().flat_map(|t| t.iter()).find(|(k, _)| *k == key).map(|(_, v)| v)
}

/// Texte d'une clé dans une langue (anglais si absent ; la clé elle-même si inconnue).
pub fn t(lang: Lang, key: &str) -> String {
    match find(key) {
        Some(v) => {
            let s = v[column(lang)];
            if s.is_empty() { v[0].to_string() } else { s.to_string() }
        }
        None => key.to_string(),
    }
}

/// Texte avec remplacements `{nom}`.
pub fn tf(lang: Lang, key: &str, args: &[(&str, &str)]) -> String {
    let mut s = t(lang, key);
    for (k, v) in args {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    s
}

/// Retire les `&` de raccourci (titres de boîtes, infobulles).
pub fn plain(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            if chars.peek() == Some(&'&') {
                out.push('&');
                chars.next();
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_unique_and_complete() {
        let mut keys: Vec<&str> = TABLES.iter().flat_map(|t| t.iter()).map(|(k, _)| *k).collect();
        keys.sort();
        let n = keys.len();
        keys.dedup();
        assert_eq!(n, keys.len(), "clés en double");
        for (k, v) in TABLES.iter().flat_map(|t| t.iter()) {
            assert!(!v[0].is_empty() && !v[1].is_empty(), "{k} : anglais et français obligatoires");
        }
    }

    #[test]
    fn fallback() {
        assert_eq!(t(Lang::De, "file.exit"), "&Beenden");
        assert_eq!(t(Lang::Ga, "pas.de.cle"), "pas.de.cle");
        assert_eq!(t(Lang::Fr, "file.exit"), "&Quitter");
        assert_eq!(plain("&Quitter"), "Quitter");
        assert_eq!(tf(Lang::Fr, "paste.text", &[("n", "3"), ("win", "#lyon")]), "Vous allez coller 3 lignes dans #lyon. Les envoyer ?");
    }

    /// Les marques `{nom}` d'un texte, triées.
    fn marks(s: &str) -> Vec<&str> {
        let mut v: Vec<&str> = s.split('{').skip(1).filter_map(|p| p.split_once('}').map(|(a, _)| a)).collect();
        v.sort();
        v
    }

    #[test]
    fn translations_keep_marks_lines_and_shortcuts() {
        // Toute traduction remplie garde les marques, les retours à la ligne et le raccourci & de l'anglais.
        let shortcut = |s: &str| s.replace("&&", "").contains('&');
        for (k, v) in TABLES.iter().flat_map(|t| t.iter()) {
            for s in v[1..].iter().filter(|s| !s.is_empty()) {
                assert_eq!(marks(v[0]), marks(s), "{k} : « {s} »");
                assert_eq!(v[0].matches('\n').count(), s.matches('\n').count(), "{k} : « {s} »");
                assert_eq!(shortcut(v[0]), shortcut(s), "{k} : raccourci & dans « {s} »");
            }
        }
    }

    /// Clés qui restent identiques à l'anglais dans les cinq autres langues : sigles, noms de mIRC, exemples.
    const SAME_AS_ENGLISH: &[&str] = &[
        "view.treebar",
        "logs.size.b",
        "logs.size.kb",
        "logs.size.mb",
        "opt.page.irc",
        "opt.unit.ms",
        "opt.ctcp.version",
        "opt.ctcp.ping",
        "opt.ctcp.time",
        "opt.ctcp.finger",
        "opt.ctcp.clientinfo",
        "opt.ctcp.userinfo",
        "opt.ctcp.source",
        "opt.disp.switchbar",
        "opt.disp.treebar",
        "opt.page.dcc",
        "net.tab.perform",
        "net.col.tls",
        "net.enc.utf8",
        "net.enc.latin1",
        "ab.tab.notify",
        "ab.tab.control",
        "ab.ign.ctcp",
        "ab.ign.dcc",
        "cc.title",
        "uc.title",
        "uc.whois",
        "se.tab.aliases",
        "se.tab.popups",
        "se.tab.remote",
        "se.tab.users",
        "se.view.status",
        "col.editbox_sample",
        "dcc.col.progress",
        "dcc.detail.error",
        "dcc.error.title",
        "mobile.tab",
        "notify.menu.whois",
    ];

    // Toute nouvelle clé doit être traduite dans les 7 langues (voir l'ADR 0012).
    #[test]
    fn every_key_in_seven_languages() {
        for (k, v) in TABLES.iter().flat_map(|t| t.iter()) {
            assert!(v.iter().all(|s| !s.is_empty()), "{k} : traduction manquante");
            // Les cinq langues identiques à l'anglais : sans doute un copier-coller.
            let copied = v[2..].iter().all(|s| *s == v[0]);
            assert!(SAME_AS_ENGLISH.contains(k) || !copied, "{k} : identique à l'anglais partout");
        }
    }
}
