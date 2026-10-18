//! L'aide intégrée : `/help` (liste des commandes) et `/help commande` (usage et rôle), dans les sept langues. Les
//! langues autres que le français gardent l'usage anglais et ne traduisent que le rôle ; une langue vide retombe sur
//! l'anglais.

use crate::i18n::Lang;

/// Aide des commandes : (commande, [en, fr, es, de, it, va, ga]), chaque texte étant « usage — rôle ».
pub const HELP: &[(&str, [&str; 7])] = &[
    (
        "abook",
        [
            "/abook — open the Address Book (Info, Notify, Ignore, Control)",
            "/abook — ouvrir le carnet d'adresses (Infos, Notify, Ignore, Control)",
            "/abook — abrir la libreta de direcciones (Info, Notify, Ignore, Control)",
            "/abook — das Adressbuch öffnen (Info, Notify, Ignore, Control)",
            "/abook — apri la rubrica (Info, Notify, Ignore, Control)",
            "/abook — obrir la llibreta d'adreces (Info, Notify, Ignore, Control)",
            "/abook — oscail an leabhar seoltaí (Eolas, Notify, Ignore, Control)",
        ],
    ),
    (
        "about",
        [
            "/about — about AlpIRC",
            "/about — à propos d'AlpIRC",
            "/about — acerca de AlpIRC",
            "/about — über AlpIRC",
            "/about — informazioni su AlpIRC",
            "/about — quant a AlpIRC",
            "/about — maidir le AlpIRC",
        ],
    ),
    (
        "action",
        [
            "/action text — same as /me",
            "/action texte — comme /me",
            "/action text — igual que /me",
            "/action text — wie /me",
            "/action text — come /me",
            "/action text — com /me",
            "/action text — mar /me",
        ],
    ),
    (
        "admin",
        [
            "/admin [server] — server administrator information",
            "/admin [serveur] — informations sur l'administrateur du serveur",
            "/admin [server] — información sobre el administrador del servidor",
            "/admin [server] — Informationen über den Serveradministrator",
            "/admin [server] — informazioni sull'amministratore del server",
            "/admin [server] — informació sobre l'administrador del servidor",
            "/admin [server] — eolas faoi riarthóir an fhreastalaí",
        ],
    ),
    (
        "alias",
        [
            "/alias [-l] name commands — add an alias (without commands: remove it)",
            "/alias [-l] nom commandes — ajouter un alias (sans commandes : le retirer)",
            "/alias [-l] name commands — añadir un alias (sin comandos: quitarlo)",
            "/alias [-l] name commands — einen Alias hinzufügen (ohne Befehle: entfernen)",
            "/alias [-l] name commands — aggiungi un alias (senza comandi: rimuovilo)",
            "/alias [-l] name commands — afegir un àlies (sense ordres: llevar-lo)",
            "/alias [-l] name commands — cuir ailias leis (gan orduithe: bain é)",
        ],
    ),
    (
        "aline",
        [
            "/aline [-l] [colour] @window text — add a line to a custom window",
            "/aline [-l] [couleur] @fenêtre texte — ajouter une ligne à une fenêtre @",
            "/aline [-l] [colour] @window text — añadir una línea a una ventana @",
            "/aline [-l] [colour] @window text — eine Zeile zu einem @-Fenster hinzufügen",
            "/aline [-l] [colour] @window text — aggiungi una riga a una finestra @",
            "/aline [-l] [colour] @window text — afegir una línia a una finestra @",
            "/aline [-l] [colour] @window text — cuir líne le fuinneog @",
        ],
    ),
    (
        "ame",
        [
            "/ame text — action on every channel you are on",
            "/ame texte — action sur tous vos canaux",
            "/ame text — acción en todos tus canales",
            "/ame text — Aktion in allen Ihren Channels",
            "/ame text — azione in tutti i tuoi canali",
            "/ame text — acció en tots els vostres canals",
            "/ame text — gníomh ar do chainéil go léir",
        ],
    ),
    (
        "amsg",
        [
            "/amsg text — message to every channel you are on",
            "/amsg texte — message sur tous vos canaux",
            "/amsg text — mensaje en todos tus canales",
            "/amsg text — Nachricht in allen Ihren Channels",
            "/amsg text — messaggio in tutti i tuoi canali",
            "/amsg text — missatge en tots els vostres canals",
            "/amsg text — teachtaireacht ar do chainéil go léir",
        ],
    ),
    (
        "away",
        [
            "/away [message] — set yourself away (without message: back)",
            "/away [message] — vous marquer absent (sans message : de retour)",
            "/away [message] — marcarte como ausente (sin mensaje: de vuelta)",
            "/away [message] — sich als abwesend markieren (ohne Nachricht: zurück)",
            "/away [message] — segnati come assente (senza messaggio: di ritorno)",
            "/away [message] — marcar-vos com a absent (sense missatge: de tornada)",
            "/away [message] — marcáil thú féin as láthair (gan teachtaireacht: ar ais)",
        ],
    ),
    (
        "back",
        [
            "/back — you are back",
            "/back — vous êtes de retour",
            "/back — estás de vuelta",
            "/back — Sie sind zurück",
            "/back — sei di ritorno",
            "/back — ja heu tornat",
            "/back — tá tú ar ais",
        ],
    ),
    (
        "ban",
        [
            "/ban [-kru#] [#channel] nick|mask [type] [reason] — ban (and kick with -k)",
            "/ban [-kru#] [#canal] pseudo|masque [type] [raison] — bannir (et expulser avec -k)",
            "/ban [-kru#] [#channel] nick|mask [type] [reason] — banear (y expulsar con -k)",
            "/ban [-kru#] [#channel] nick|mask [type] [reason] — bannen (und mit -k kicken)",
            "/ban [-kru#] [#channel] nick|mask [type] [reason] — banna (ed espelli con -k)",
            "/ban [-kru#] [#channel] nick|mask [type] [reason] — bandejar (i expulsar amb -k)",
            "/ban [-kru#] [#channel] nick|mask [type] [reason] — toirmisc (agus caith amach le -k)",
        ],
    ),
    (
        "beep",
        [
            "/beep [n] — beep",
            "/beep [n] — bip",
            "/beep [n] — pitido",
            "/beep [n] — Piepton",
            "/beep [n] — bip",
            "/beep [n] — bip",
            "/beep [n] — bíp",
        ],
    ),
    (
        "channel",
        [
            "/channel [#channel] — open Channel Central",
            "/channel [#canal] — ouvrir Channel Central",
            "/channel [#channel] — abrir Channel Central",
            "/channel [#channel] — Channel Central öffnen",
            "/channel [#channel] — apri Channel Central",
            "/channel [#channel] — obrir Channel Central",
            "/channel [#channel] — oscail Channel Central",
        ],
    ),
    (
        "clear",
        [
            "/clear [-as] [window] — clear a window",
            "/clear [-as] [fenêtre] — effacer une fenêtre",
            "/clear [-as] [window] — limpiar una ventana",
            "/clear [-as] [window] — ein Fenster leeren",
            "/clear [-as] [window] — svuota una finestra",
            "/clear [-as] [window] — netejar una finestra",
            "/clear [-as] [window] — glan fuinneog",
        ],
    ),
    (
        "clearall",
        [
            "/clearall — clear every window",
            "/clearall — effacer toutes les fenêtres",
            "/clearall — limpiar todas las ventanas",
            "/clearall — alle Fenster leeren",
            "/clearall — svuota tutte le finestre",
            "/clearall — netejar totes les finestres",
            "/clearall — glan gach fuinneog",
        ],
    ),
    (
        "cline",
        [
            "/cline [colour] @window N — change the colour of a line",
            "/cline [couleur] @fenêtre N — changer la couleur d'une ligne",
            "/cline [colour] @window N — cambiar el color de una línea",
            "/cline [colour] @window N — die Farbe einer Zeile ändern",
            "/cline [colour] @window N — cambia il colore di una riga",
            "/cline [colour] @window N — canviar el color d'una línia",
            "/cline [colour] @window N — athraigh dath líne",
        ],
    ),
    (
        "close",
        [
            "/close [-m|-c|-@] [names] — close windows (queries, chats, custom)",
            "/close [-m|-c|-@] [noms] — fermer des fenêtres (privés, discussions, @)",
            "/close [-m|-c|-@] [names] — cerrar ventanas (privados, chats, @)",
            "/close [-m|-c|-@] [names] — Fenster schließen (Queries, Chats, @)",
            "/close [-m|-c|-@] [names] — chiudi finestre (query, chat, @)",
            "/close [-m|-c|-@] [names] — tancar finestres (privats, xats, @)",
            "/close [-m|-c|-@] [names] — dún fuinneoga (comhráite, DCC Chat, @)",
        ],
    ),
    (
        "closemsg",
        [
            "/closemsg [nick] — close one query window, or all of them",
            "/closemsg [pseudo] — fermer un privé, ou tous",
            "/closemsg [nick] — cerrar un privado, o todos",
            "/closemsg [nick] — eine Query schließen, oder alle",
            "/closemsg [nick] — chiudi una query, o tutte",
            "/closemsg [nick] — tancar un privat, o tots",
            "/closemsg [nick] — dún comhrá amháin, nó iad go léir",
        ],
    ),
    (
        "colors",
        [
            "/colors — open the colours dialog",
            "/colors — ouvrir la boîte des couleurs",
            "/colors — abrir el cuadro de colores",
            "/colors — den Farbdialog öffnen",
            "/colors — apri la finestra dei colori",
            "/colors — obrir el quadre de colors",
            "/colors — oscail bosca na ndathanna",
        ],
    ),
    (
        "ctcp",
        [
            "/ctcp nick|#channel command [text] — send a CTCP request",
            "/ctcp pseudo|#canal commande [texte] — envoyer une requête CTCP",
            "/ctcp nick|#channel command [text] — enviar una petición CTCP",
            "/ctcp nick|#channel command [text] — eine CTCP-Anfrage senden",
            "/ctcp nick|#channel command [text] — invia una richiesta CTCP",
            "/ctcp nick|#channel command [text] — enviar una petició CTCP",
            "/ctcp nick|#channel command [text] — seol iarratas CTCP",
        ],
    ),
    (
        "ctcpreply",
        [
            "/ctcpreply nick command [text] — send a CTCP reply",
            "/ctcpreply pseudo commande [texte] — envoyer une réponse CTCP",
            "/ctcpreply nick command [text] — enviar una respuesta CTCP",
            "/ctcpreply nick command [text] — eine CTCP-Antwort senden",
            "/ctcpreply nick command [text] — invia una risposta CTCP",
            "/ctcpreply nick command [text] — enviar una resposta CTCP",
            "/ctcpreply nick command [text] — seol freagra CTCP",
        ],
    ),
    (
        "cycle",
        [
            "/cycle [#channel] — part and rejoin a channel",
            "/cycle [#canal] — quitter et rejoindre un canal",
            "/cycle [#channel] — salir de un canal y volver a entrar",
            "/cycle [#channel] — einen Channel verlassen und wieder betreten",
            "/cycle [#channel] — esci da un canale e rientra",
            "/cycle [#channel] — eixir d'un canal i tornar a entrar",
            "/cycle [#channel] — fág cainéal agus téigh ar ais isteach ann",
        ],
    ),
    (
        "dcc",
        [
            "/dcc send [-c] nick [file] | chat nick | get nick | close send|get|chat nick | list — files, direct chats",
            "/dcc send [-c] pseudo [fichier] | chat pseudo | get pseudo | close send|get|chat pseudo | list \
             — fichiers, discussions directes",
            "/dcc send [-c] nick [file] | chat nick | get nick | close send|get|chat nick | list — archivos, chats \
             directos",
            "/dcc send [-c] nick [file] | chat nick | get nick | close send|get|chat nick | list — Dateien, direkte \
             Chats",
            "/dcc send [-c] nick [file] | chat nick | get nick | close send|get|chat nick | list — file, chat diretti",
            "/dcc send [-c] nick [file] | chat nick | get nick | close send|get|chat nick | list — fitxers, xats \
             directes",
            "/dcc send [-c] nick [file] | chat nick | get nick | close send|get|chat nick | list — comhaid, comhráite \
             díreacha",
        ],
    ),
    (
        "dehalfop",
        [
            "/dehalfop [#channel] nicks — remove half-op",
            "/dehalfop [#canal] pseudos — retirer le demi-op",
            "/dehalfop [#channel] nicks — quitar el semi-op",
            "/dehalfop [#channel] nicks — Halfop entziehen",
            "/dehalfop [#channel] nicks — togli l'halfop",
            "/dehalfop [#channel] nicks — llevar el semiop",
            "/dehalfop [#channel] nicks — bain leath-op",
        ],
    ),
    (
        "deop",
        [
            "/deop [#channel] nicks — remove op",
            "/deop [#canal] pseudos — retirer l'op",
            "/deop [#channel] nicks — quitar el op",
            "/deop [#channel] nicks — Op entziehen",
            "/deop [#channel] nicks — togli l'op",
            "/deop [#channel] nicks — llevar l'op",
            "/deop [#channel] nicks — bain op",
        ],
    ),
    (
        "describe",
        [
            "/describe nick|#channel text — action to a nick or channel",
            "/describe pseudo|#canal texte — action vers un pseudo ou un canal",
            "/describe nick|#channel text — acción hacia un nick o un canal",
            "/describe nick|#channel text — Aktion an einen Nick oder Channel",
            "/describe nick|#channel text — azione verso un nick o un canale",
            "/describe nick|#channel text — acció cap a un sobrenom o un canal",
            "/describe nick|#channel text — gníomh chuig leasainm nó cainéal",
        ],
    ),
    (
        "devoice",
        [
            "/devoice [#channel] nicks — remove voice",
            "/devoice [#canal] pseudos — retirer le voice",
            "/devoice [#channel] nicks — quitar la voz",
            "/devoice [#channel] nicks — Voice entziehen",
            "/devoice [#channel] nicks — togli il voice",
            "/devoice [#channel] nicks — llevar la veu",
            "/devoice [#channel] nicks — bain guth",
        ],
    ),
    (
        "disable",
        [
            "/disable #group — turn a script group off",
            "/disable #groupe — éteindre un groupe de script",
            "/disable #group — desactivar un grupo de script",
            "/disable #group — eine Script-Gruppe ausschalten",
            "/disable #group — disattiva un gruppo di script",
            "/disable #group — desactivar un grup de script",
            "/disable #group — múch grúpa scripte",
        ],
    ),
    (
        "disconnect",
        [
            "/disconnect [reason] — disconnect from the server",
            "/disconnect [raison] — se déconnecter du serveur",
            "/disconnect [reason] — desconectarse del servidor",
            "/disconnect [reason] — vom Server trennen",
            "/disconnect [reason] — disconnettiti dal server",
            "/disconnect [reason] — desconnectar-se del servidor",
            "/disconnect [reason] — dícheangail ón bhfreastalaí",
        ],
    ),
    (
        "dline",
        [
            "/dline [-l] @window N[-M] — delete lines of a custom window",
            "/dline [-l] @fenêtre N[-M] — supprimer des lignes d'une fenêtre @",
            "/dline [-l] @window N[-M] — borrar líneas de una ventana @",
            "/dline [-l] @window N[-M] — Zeilen eines @-Fensters löschen",
            "/dline [-l] @window N[-M] — elimina righe di una finestra @",
            "/dline [-l] @window N[-M] — esborrar línies d'una finestra @",
            "/dline [-l] @window N[-M] — scrios línte as fuinneog @",
        ],
    ),
    (
        "dns",
        [
            "/dns — not supported by AlpIRC",
            "/dns — non pris en charge par AlpIRC",
            "/dns — no admitido por AlpIRC",
            "/dns — von AlpIRC nicht unterstützt",
            "/dns — non supportato da AlpIRC",
            "/dns — no admés per AlpIRC",
            "/dns — ní thacaíonn AlpIRC leis",
        ],
    ),
    (
        "echo",
        [
            "/echo [colour] [-acdegst] [window] text — print text without sending it",
            "/echo [couleur] [-acdegst] [fenêtre] texte — afficher un texte sans l'envoyer",
            "/echo [colour] [-acdegst] [window] text — mostrar un texto sin enviarlo",
            "/echo [colour] [-acdegst] [window] text — Text anzeigen, ohne ihn zu senden",
            "/echo [colour] [-acdegst] [window] text — mostra un testo senza inviarlo",
            "/echo [colour] [-acdegst] [window] text — mostrar un text sense enviar-lo",
            "/echo [colour] [-acdegst] [window] text — taispeáin téacs gan é a sheoladh",
        ],
    ),
    (
        "enable",
        [
            "/enable #group — turn a script group on",
            "/enable #groupe — allumer un groupe de script",
            "/enable #group — activar un grupo de script",
            "/enable #group — eine Script-Gruppe einschalten",
            "/enable #group — attiva un gruppo di script",
            "/enable #group — activar un grup de script",
            "/enable #group — cuir grúpa scripte ar siúl",
        ],
    ),
    (
        "exit",
        [
            "/exit — quit AlpIRC",
            "/exit — quitter AlpIRC",
            "/exit — salir de AlpIRC",
            "/exit — AlpIRC beenden",
            "/exit — esci da AlpIRC",
            "/exit — eixir d'AlpIRC",
            "/exit — scoir AlpIRC",
        ],
    ),
    (
        "flash",
        [
            "/flash [window] — flash a window",
            "/flash [fenêtre] — faire clignoter une fenêtre",
            "/flash [window] — hacer parpadear una ventana",
            "/flash [window] — ein Fenster blinken lassen",
            "/flash [window] — fai lampeggiare una finestra",
            "/flash [window] — fer parpellejar una finestra",
            "/flash [window] — splanc fuinneog",
        ],
    ),
    (
        "folder",
        [
            "/folder — open the Channels Folder",
            "/folder — ouvrir le dossier des canaux",
            "/folder — abrir la carpeta de canales",
            "/folder — den Channel-Ordner öffnen",
            "/folder — apri la cartella dei canali",
            "/folder — obrir la carpeta de canals",
            "/folder — oscail an fillteán cainéal",
        ],
    ),
    (
        "font",
        [
            "/font — display options (font)",
            "/font — options d'affichage (police)",
            "/font — opciones de pantalla (fuente)",
            "/font — Anzeigeoptionen (Schrift)",
            "/font — opzioni di aspetto (carattere)",
            "/font — opcions d'aparença (lletra)",
            "/font — roghanna taispeána (cló)",
        ],
    ),
    (
        "hadd",
        [
            "/hadd [-m] table item value — set an item of a hash table",
            "/hadd [-m] table élément valeur — remplir un élément d'une table",
            "/hadd [-m] table item value — dar valor a un elemento de una tabla hash",
            "/hadd [-m] table item value — einen Eintrag einer Hashtabelle setzen",
            "/hadd [-m] table item value — imposta un elemento di una tabella hash",
            "/hadd [-m] table item value — donar valor a un element d'una taula hash",
            "/hadd [-m] table item value — socraigh mír de thábla haiseála",
        ],
    ),
    (
        "halfop",
        [
            "/halfop [#channel] nicks — give half-op",
            "/halfop [#canal] pseudos — donner le demi-op",
            "/halfop [#channel] nicks — dar semi-op",
            "/halfop [#channel] nicks — Halfop geben",
            "/halfop [#channel] nicks — dai l'halfop",
            "/halfop [#channel] nicks — donar semiop",
            "/halfop [#channel] nicks — tabhair leath-op",
        ],
    ),
    (
        "help",
        [
            "/help [command] — this help",
            "/help [commande] — cette aide",
            "/help [command] — esta ayuda",
            "/help [command] — diese Hilfe",
            "/help [command] — questa guida",
            "/help [command] — esta ajuda",
            "/help [command] — an chabhair seo",
        ],
    ),
    (
        "hmake",
        [
            "/hmake table [size] — create a hash table",
            "/hmake table [taille] — créer une table de hachage",
            "/hmake table [size] — crear una tabla hash",
            "/hmake table [size] — eine Hashtabelle erstellen",
            "/hmake table [size] — crea una tabella hash",
            "/hmake table [size] — crear una taula hash",
            "/hmake table [size] — cruthaigh tábla haiseála",
        ],
    ),
    (
        "hop",
        [
            "/hop [#channel] — part and rejoin a channel",
            "/hop [#canal] — quitter et rejoindre un canal",
            "/hop [#channel] — salir de un canal y volver a entrar",
            "/hop [#channel] — einen Channel verlassen und wieder betreten",
            "/hop [#channel] — esci da un canale e rientra",
            "/hop [#channel] — eixir d'un canal i tornar a entrar",
            "/hop [#channel] — fág cainéal agus téigh ar ais isteach ann",
        ],
    ),
    (
        "ial",
        [
            "/ial on|off — the internal address list (always on in AlpIRC)",
            "/ial on|off — liste interne des adresses (toujours active dans AlpIRC)",
            "/ial on|off — la lista interna de direcciones (siempre activa en AlpIRC)",
            "/ial on|off — die interne Adressliste (in AlpIRC immer aktiv)",
            "/ial on|off — la lista interna degli indirizzi (sempre attiva in AlpIRC)",
            "/ial on|off — la llista interna d'adreces (sempre activa en AlpIRC)",
            "/ial on|off — an liosta inmheánach seoltaí (ar siúl i gcónaí in AlpIRC)",
        ],
    ),
    (
        "ignore",
        [
            "/ignore [-rpcntikdu#] nick|mask [type] [network] — ignore someone (-r: stop)",
            "/ignore [-rpcntikdu#] pseudo|masque [type] [réseau] — ignorer quelqu'un (-r : arrêter)",
            "/ignore [-rpcntikdu#] nick|mask [type] [network] — ignorar a alguien (-r: dejar de ignorarlo)",
            "/ignore [-rpcntikdu#] nick|mask [type] [network] — jemanden ignorieren (-r: aufheben)",
            "/ignore [-rpcntikdu#] nick|mask [type] [network] — ignora qualcuno (-r: smetti di ignorarlo)",
            "/ignore [-rpcntikdu#] nick|mask [type] [network] — ignorar algú (-r: deixar d'ignorar-lo)",
            "/ignore [-rpcntikdu#] nick|mask [type] [network] — déan neamhaird de dhuine (-r: stad)",
        ],
    ),
    (
        "iline",
        [
            "/iline [-l] [colour] @window N text — insert a line in a custom window",
            "/iline [-l] [couleur] @fenêtre N texte — insérer une ligne dans une fenêtre @",
            "/iline [-l] [colour] @window N text — insertar una línea en una ventana @",
            "/iline [-l] [colour] @window N text — eine Zeile in ein @-Fenster einfügen",
            "/iline [-l] [colour] @window N text — inserisci una riga in una finestra @",
            "/iline [-l] [colour] @window N text — inserir una línia en una finestra @",
            "/iline [-l] [colour] @window N text — ionsáigh líne i bhfuinneog @",
        ],
    ),
    (
        "inc",
        [
            "/inc %var [n] — add to a variable",
            "/inc %var [n] — ajouter à une variable",
            "/inc %var [n] — sumar a una variable",
            "/inc %var [n] — zu einer Variablen addieren",
            "/inc %var [n] — aggiungi a una variabile",
            "/inc %var [n] — sumar a una variable",
            "/inc %var [n] — cuir le hathróg",
        ],
    ),
    (
        "info",
        [
            "/info [server] — server information",
            "/info [serveur] — informations sur le serveur",
            "/info [server] — información sobre el servidor",
            "/info [server] — Informationen über den Server",
            "/info [server] — informazioni sul server",
            "/info [server] — informació sobre el servidor",
            "/info [server] — eolas faoin bhfreastalaí",
        ],
    ),
    (
        "invite",
        [
            "/invite nick [#channel] — invite someone to a channel",
            "/invite pseudo [#canal] — inviter quelqu'un sur un canal",
            "/invite nick [#channel] — invitar a alguien a un canal",
            "/invite nick [#channel] — jemanden in einen Channel einladen",
            "/invite nick [#channel] — invita qualcuno in un canale",
            "/invite nick [#channel] — convidar algú a un canal",
            "/invite nick [#channel] — tabhair cuireadh do dhuine chuig cainéal",
        ],
    ),
    (
        "join",
        [
            "/join #channel[,#other] [key] — join channels",
            "/join #canal[,#autre] [clé] — rejoindre des canaux",
            "/join #channel[,#other] [key] — entrar en canales",
            "/join #channel[,#other] [key] — Channels betreten",
            "/join #channel[,#other] [key] — entra nei canali",
            "/join #channel[,#other] [key] — entrar en canals",
            "/join #channel[,#other] [key] — téigh isteach i gcainéil",
        ],
    ),
    (
        "kb",
        [
            "/kb nick [reason] — kick and ban from the current channel (default alias)",
            "/kb pseudo [raison] — expulser et bannir du canal courant (alias par défaut)",
            "/kb nick [reason] — expulsar y banear del canal actual (alias predeterminado)",
            "/kb nick [reason] — aus dem aktuellen Channel kicken und bannen (Standard-Alias)",
            "/kb nick [reason] — espelli e banna dal canale corrente (alias predefinito)",
            "/kb nick [reason] — expulsar i bandejar del canal actual (àlies predeterminat)",
            "/kb nick [reason] — caith amach agus toirmisc ón gcainéal reatha (ailias réamhshocraithe)",
        ],
    ),
    (
        "kick",
        [
            "/kick [#channel] nick [reason] — kick someone",
            "/kick [#canal] pseudo [raison] — expulser quelqu'un",
            "/kick [#channel] nick [reason] — expulsar a alguien",
            "/kick [#channel] nick [reason] — jemanden kicken",
            "/kick [#channel] nick [reason] — espelli qualcuno",
            "/kick [#channel] nick [reason] — expulsar algú",
            "/kick [#channel] nick [reason] — caith duine amach",
        ],
    ),
    (
        "lag",
        [
            "/lag — measure the lag to the server",
            "/lag — mesurer la latence avec le serveur",
            "/lag — medir el lag con el servidor",
            "/lag — den Lag zum Server messen",
            "/lag — misura il lag verso il server",
            "/lag — mesurar la latència amb el servidor",
            "/lag — tomhais an mhoill go dtí an freastalaí",
        ],
    ),
    (
        "linesep",
        [
            "/linesep [window] — print a separator line",
            "/linesep [fenêtre] — afficher une ligne de séparation",
            "/linesep [window] — mostrar una línea de separación",
            "/linesep [window] — eine Trennlinie anzeigen",
            "/linesep [window] — mostra una riga di separazione",
            "/linesep [window] — mostrar una línia de separació",
            "/linesep [window] — taispeáin líne scartha",
        ],
    ),
    (
        "links",
        [
            "/links [mask] — list the servers of the network",
            "/links [masque] — lister les serveurs du réseau",
            "/links [mask] — listar los servidores de la red",
            "/links [mask] — die Server des Netzwerks auflisten",
            "/links [mask] — elenca i server della rete",
            "/links [mask] — llistar els servidors de la xarxa",
            "/links [mask] — liostaigh freastalaithe an líonra",
        ],
    ),
    (
        "list",
        [
            "/list [-min N] [-max N] [mask] — list channels",
            "/list [-min N] [-max N] [masque] — lister les canaux",
            "/list [-min N] [-max N] [mask] — listar los canales",
            "/list [-min N] [-max N] [mask] — Channels auflisten",
            "/list [-min N] [-max N] [mask] — elenca i canali",
            "/list [-min N] [-max N] [mask] — llistar els canals",
            "/list [-min N] [-max N] [mask] — liostaigh na cainéil",
        ],
    ),
    (
        "load",
        [
            "/load -rs|-a|-pc|-pn|-pq|-ps|-pm file — load a script (remote, aliases, popups)",
            "/load -rs|-a|-pc|-pn|-pq|-ps|-pm fichier — charger un script (remote, alias, popups)",
            "/load -rs|-a|-pc|-pn|-pq|-ps|-pm file — cargar un script (remote, alias, popups)",
            "/load -rs|-a|-pc|-pn|-pq|-ps|-pm file — ein Script laden (Remote, Aliases, Popups)",
            "/load -rs|-a|-pc|-pn|-pq|-ps|-pm file — carica uno script (remote, alias, popup)",
            "/load -rs|-a|-pc|-pn|-pq|-ps|-pm file — carregar un script (remote, àlies, popups)",
            "/load -rs|-a|-pc|-pn|-pq|-ps|-pm file — lódáil script (remote, ailiasanna, popups)",
        ],
    ),
    (
        "log",
        [
            "/log on|off [window] — log a window to disk",
            "/log on|off [fenêtre] — enregistrer une fenêtre dans son journal",
            "/log on|off [window] — guardar una ventana en su registro",
            "/log on|off [window] — ein Fenster in seine Logdatei schreiben",
            "/log on|off [window] — registra una finestra nel suo log",
            "/log on|off [window] — guardar una finestra en el seu registre",
            "/log on|off [window] — logáil fuinneog chuig a loga",
        ],
    ),
    (
        "lusers",
        [
            "/lusers — network statistics",
            "/lusers — statistiques du réseau",
            "/lusers — estadísticas de la red",
            "/lusers — Netzwerkstatistik",
            "/lusers — statistiche della rete",
            "/lusers — estadístiques de la xarxa",
            "/lusers — staitisticí an líonra",
        ],
    ),
    (
        "me",
        [
            "/me text — action in the current channel or query",
            "/me texte — action dans le canal ou le privé courant",
            "/me text — acción en el canal o privado actual",
            "/me text — Aktion im aktuellen Channel oder in der Query",
            "/me text — azione nel canale o nella query corrente",
            "/me text — acció en el canal o privat actual",
            "/me text — gníomh sa chainéal nó sa chomhrá reatha",
        ],
    ),
    (
        "mode",
        [
            "/mode [#channel|nick] modes [parameters] — change modes",
            "/mode [#canal|pseudo] modes [paramètres] — changer des modes",
            "/mode [#channel|nick] modes [parameters] — cambiar modos",
            "/mode [#channel|nick] modes [parameters] — Modi ändern",
            "/mode [#channel|nick] modes [parameters] — cambia i modi",
            "/mode [#channel|nick] modes [parameters] — canviar modes",
            "/mode [#channel|nick] modes [parameters] — athraigh móid",
        ],
    ),
    (
        "motd",
        [
            "/motd [server] — message of the day",
            "/motd [serveur] — message du jour",
            "/motd [server] — mensaje del día",
            "/motd [server] — Nachricht des Tages",
            "/motd [server] — messaggio del giorno",
            "/motd [server] — missatge del dia",
            "/motd [server] — teachtaireacht an lae",
        ],
    ),
    (
        "msg",
        [
            "/msg nick|#channel|=chat text — send a message",
            "/msg pseudo|#canal|=discussion texte — envoyer un message",
            "/msg nick|#channel|=chat text — enviar un mensaje",
            "/msg nick|#channel|=chat text — eine Nachricht senden",
            "/msg nick|#channel|=chat text — invia un messaggio",
            "/msg nick|#channel|=chat text — enviar un missatge",
            "/msg nick|#channel|=chat text — seol teachtaireacht",
        ],
    ),
    (
        "names",
        [
            "/names [#channel] — list the nicks of a channel",
            "/names [#canal] — lister les pseudos d'un canal",
            "/names [#channel] — listar los nicks de un canal",
            "/names [#channel] — die Nicks eines Channels auflisten",
            "/names [#channel] — elenca i nick di un canale",
            "/names [#channel] — llistar els sobrenoms d'un canal",
            "/names [#channel] — liostaigh leasainmneacha cainéil",
        ],
    ),
    (
        "nick",
        [
            "/nick newnick — change your nickname",
            "/nick nouveau — changer de pseudo",
            "/nick newnick — cambiar de nick",
            "/nick newnick — den Nick ändern",
            "/nick newnick — cambia nick",
            "/nick newnick — canviar de sobrenom",
            "/nick newnick — athraigh do leasainm",
        ],
    ),
    (
        "notice",
        [
            "/notice nick|#channel text — send a notice",
            "/notice pseudo|#canal texte — envoyer une notice",
            "/notice nick|#channel text — enviar un notice",
            "/notice nick|#channel text — eine Notice senden",
            "/notice nick|#channel text — invia una notice",
            "/notice nick|#channel text — enviar un avís",
            "/notice nick|#channel text — seol fógra",
        ],
    ),
    (
        "notify",
        [
            "/notify [-r] [nick [note]] — notify list (no argument: open the Notify window)",
            "/notify [-r] [pseudo [note]] — liste Notify (sans argument : ouvrir la fenêtre Notify)",
            "/notify [-r] [nick [note]] — lista Notify (sin argumento: abrir la ventana Notify)",
            "/notify [-r] [nick [note]] — Notify-Liste (ohne Argument: Notify-Fenster öffnen)",
            "/notify [-r] [nick [note]] — lista Notify (senza argomenti: apri la finestra Notify)",
            "/notify [-r] [nick [note]] — llista Notify (sense arguments: obrir la finestra Notify)",
            "/notify [-r] [nick [note]] — liosta Notify (gan argóint: oscail an fhuinneog Notify)",
        ],
    ),
    (
        "omsg",
        [
            "/omsg [#channel] text — message to the channel operators",
            "/omsg [#canal] texte — message aux opérateurs du canal",
            "/omsg [#channel] text — mensaje a los operadores del canal",
            "/omsg [#channel] text — Nachricht an die Channel-Operatoren",
            "/omsg [#channel] text — messaggio agli operatori del canale",
            "/omsg [#channel] text — missatge als operadors del canal",
            "/omsg [#channel] text — teachtaireacht chuig oibreoirí an chainéil",
        ],
    ),
    (
        "onotice",
        [
            "/onotice [#channel] text — notice to the channel operators",
            "/onotice [#canal] texte — notice aux opérateurs du canal",
            "/onotice [#channel] text — notice a los operadores del canal",
            "/onotice [#channel] text — Notice an die Channel-Operatoren",
            "/onotice [#channel] text — notice agli operatori del canale",
            "/onotice [#channel] text — avís als operadors del canal",
            "/onotice [#channel] text — fógra chuig oibreoirí an chainéil",
        ],
    ),
    (
        "op",
        [
            "/op [#channel] nicks — give op",
            "/op [#canal] pseudos — donner l'op",
            "/op [#channel] nicks — dar op",
            "/op [#channel] nicks — Op geben",
            "/op [#channel] nicks — dai l'op",
            "/op [#channel] nicks — donar op",
            "/op [#channel] nicks — tabhair op",
        ],
    ),
    (
        "options",
        [
            "/options [page] — open the Options dialog",
            "/options [page] — ouvrir la boîte Options",
            "/options [page] — abrir el cuadro de opciones",
            "/options [page] — den Optionen-Dialog öffnen",
            "/options [page] — apri la finestra delle opzioni",
            "/options [page] — obrir el quadre d'opcions",
            "/options [page] — oscail bosca na roghanna",
        ],
    ),
    (
        "part",
        [
            "/part [#channel] [reason] — leave a channel",
            "/part [#canal] [raison] — quitter un canal",
            "/part [#channel] [reason] — salir de un canal",
            "/part [#channel] [reason] — einen Channel verlassen",
            "/part [#channel] [reason] — esci da un canale",
            "/part [#channel] [reason] — eixir d'un canal",
            "/part [#channel] [reason] — fág cainéal",
        ],
    ),
    (
        "partall",
        [
            "/partall [reason] — leave every channel",
            "/partall [raison] — quitter tous les canaux",
            "/partall [reason] — salir de todos los canales",
            "/partall [reason] — alle Channels verlassen",
            "/partall [reason] — esci da tutti i canali",
            "/partall [reason] — eixir de tots els canals",
            "/partall [reason] — fág gach cainéal",
        ],
    ),
    (
        "ping",
        [
            "/ping [server] — ping the server",
            "/ping [serveur] — envoyer un PING au serveur",
            "/ping [server] — enviar un PING al servidor",
            "/ping [server] — einen PING an den Server senden",
            "/ping [server] — invia un PING al server",
            "/ping [server] — enviar un PING al servidor",
            "/ping [server] — seol PING chuig an bhfreastalaí",
        ],
    ),
    (
        "query",
        [
            "/query nick [text] — open a query window",
            "/query pseudo [texte] — ouvrir une fenêtre de privé",
            "/query nick [text] — abrir una ventana de privado",
            "/query nick [text] — ein Query-Fenster öffnen",
            "/query nick [text] — apri una finestra di query",
            "/query nick [text] — obrir una finestra de privat",
            "/query nick [text] — oscail fuinneog chomhrá",
        ],
    ),
    (
        "quit",
        [
            "/quit [reason] — disconnect from the server",
            "/quit [raison] — se déconnecter du serveur",
            "/quit [reason] — desconectarse del servidor",
            "/quit [reason] — vom Server trennen",
            "/quit [reason] — disconnettiti dal server",
            "/quit [reason] — desconnectar-se del servidor",
            "/quit [reason] — dícheangail ón bhfreastalaí",
        ],
    ),
    (
        "quote",
        [
            "/quote [-q] line — send a raw line to the server",
            "/quote [-q] ligne — envoyer une ligne brute au serveur",
            "/quote [-q] line — enviar una línea en bruto al servidor",
            "/quote [-q] line — eine Rohzeile an den Server senden",
            "/quote [-q] line — invia una riga grezza al server",
            "/quote [-q] line — enviar una línia en brut al servidor",
            "/quote [-q] line — seol líne amh chuig an bhfreastalaí",
        ],
    ),
    (
        "raw",
        [
            "/raw [-q] line — send a raw line to the server",
            "/raw [-q] ligne — envoyer une ligne brute au serveur",
            "/raw [-q] line — enviar una línea en bruto al servidor",
            "/raw [-q] line — eine Rohzeile an den Server senden",
            "/raw [-q] line — invia una riga grezza al server",
            "/raw [-q] line — enviar una línia en brut al servidor",
            "/raw [-q] line — seol líne amh chuig an bhfreastalaí",
        ],
    ),
    (
        "reload",
        [
            "/reload -rs file — reload a script without on LOAD",
            "/reload -rs fichier — relire un script sans on LOAD",
            "/reload -rs file — recargar un script sin on LOAD",
            "/reload -rs file — ein Script ohne on LOAD neu laden",
            "/reload -rs file — ricarica uno script senza on LOAD",
            "/reload -rs file — tornar a carregar un script sense on LOAD",
            "/reload -rs file — athlódáil script gan on LOAD",
        ],
    ),
    (
        "rline",
        [
            "/rline [-l] [colour] @window N text — replace a line of a custom window",
            "/rline [-l] [couleur] @fenêtre N texte — remplacer une ligne d'une fenêtre @",
            "/rline [-l] [colour] @window N text — sustituir una línea de una ventana @",
            "/rline [-l] [colour] @window N text — eine Zeile eines @-Fensters ersetzen",
            "/rline [-l] [colour] @window N text — sostituisci una riga di una finestra @",
            "/rline [-l] [colour] @window N text — substituir una línia d'una finestra @",
            "/rline [-l] [colour] @window N text — cuir líne eile in ionad líne i bhfuinneog @",
        ],
    ),
    (
        "run",
        [
            "/run — disabled in AlpIRC",
            "/run — désactivé dans AlpIRC",
            "/run — desactivado en AlpIRC",
            "/run — in AlpIRC deaktiviert",
            "/run — disattivato in AlpIRC",
            "/run — desactivat en AlpIRC",
            "/run — díchumasaithe in AlpIRC",
        ],
    ),
    (
        "say",
        [
            "/say text — message to the current window",
            "/say texte — message dans la fenêtre courante",
            "/say text — mensaje en la ventana actual",
            "/say text — Nachricht im aktuellen Fenster",
            "/say text — messaggio nella finestra corrente",
            "/say text — missatge en la finestra actual",
            "/say text — teachtaireacht san fhuinneog reatha",
        ],
    ),
    (
        "scripts",
        [
            "/scripts — open the Scripts Editor",
            "/scripts — ouvrir l'éditeur de scripts",
            "/scripts — abrir el editor de scripts",
            "/scripts — den Script-Editor öffnen",
            "/scripts — apri l'editor di script",
            "/scripts — obrir l'editor de scripts",
            "/scripts — oscail an t-eagarthóir scripteanna",
        ],
    ),
    (
        "server",
        [
            "/server [-m] [network|host [port] [password]] — connect (-m: new window)",
            "/server [-m] [réseau|hôte [port] [mot de passe]] — se connecter (-m : nouvelle fenêtre)",
            "/server [-m] [network|host [port] [password]] — conectar (-m: ventana nueva)",
            "/server [-m] [network|host [port] [password]] — verbinden (-m: neues Fenster)",
            "/server [-m] [network|host [port] [password]] — connetti (-m: nuova finestra)",
            "/server [-m] [network|host [port] [password]] — connectar (-m: finestra nova)",
            "/server [-m] [network|host [port] [password]] — ceangail (-m: fuinneog nua)",
        ],
    ),
    (
        "set",
        [
            "/set [-u N] %var value — set a global variable",
            "/set [-u N] %var valeur — donner une valeur à une variable globale",
            "/set [-u N] %var value — dar valor a una variable global",
            "/set [-u N] %var value — eine globale Variable setzen",
            "/set [-u N] %var value — imposta una variabile globale",
            "/set [-u N] %var value — donar valor a una variable global",
            "/set [-u N] %var value — socraigh athróg dhomhanda",
        ],
    ),
    (
        "setname",
        [
            "/setname real name — change your real name",
            "/setname vrai nom — changer votre vrai nom",
            "/setname real name — cambiar tu nombre real",
            "/setname real name — Ihren Realnamen ändern",
            "/setname real name — cambia il tuo nome reale",
            "/setname real name — canviar el vostre nom real",
            "/setname real name — athraigh d'fhíor-ainm",
        ],
    ),
    (
        "sline",
        [
            "/sline [-a|-r] @window N — select a line of a custom window list",
            "/sline [-a|-r] @fenêtre N — choisir une ligne de la liste d'une fenêtre @",
            "/sline [-a|-r] @window N — seleccionar una línea de la lista de una ventana @",
            "/sline [-a|-r] @window N — eine Zeile der Liste eines @-Fensters auswählen",
            "/sline [-a|-r] @window N — seleziona una riga dell'elenco di una finestra @",
            "/sline [-a|-r] @window N — seleccionar una línia de la llista d'una finestra @",
            "/sline [-a|-r] @window N — roghnaigh líne de liosta fuinneoige @",
        ],
    ),
    (
        "sound",
        [
            "/sound [nick|#channel] file [message] — send a sound request (the message as an action); /sound on|off — \
             accept or refuse the requests you receive",
            "/sound [pseudo|#canal] fichier [message] — envoyer une demande de son (le message comme une action) ; \
             /sound on|off — accepter ou refuser les demandes reçues",
            "/sound [nick|#channel] file [message] — enviar una petición de sonido (el mensaje como acción); /sound \
             on|off — aceptar o rechazar las peticiones que recibes",
            "/sound [nick|#channel] file [message] — eine Klanganfrage senden (die Nachricht als Aktion); /sound \
             on|off — empfangene Anfragen annehmen oder ablehnen",
            "/sound [nick|#channel] file [message] — invia una richiesta di suono (il messaggio come azione); /sound \
             on|off — accetta o rifiuta le richieste ricevute",
            "/sound [nick|#channel] file [message] — enviar una petició de so (el missatge com a acció); /sound \
             on|off — acceptar o rebutjar les peticions rebudes",
            "/sound [nick|#channel] file [message] — seol iarratas fuaime (an teachtaireacht mar ghníomh); /sound \
             on|off — glac le hiarratais a fhaigheann tú nó diúltaigh dóibh",
        ],
    ),
    (
        "splay",
        [
            "/splay file — play a sound; /splay stop — stop it",
            "/splay fichier — jouer un son ; /splay stop — l'arrêter",
            "/splay file — reproducir un sonido; /splay stop — detenerlo",
            "/splay file — einen Klang abspielen; /splay stop — ihn anhalten",
            "/splay file — riproduci un suono; /splay stop — fermalo",
            "/splay file — reproduir un so; /splay stop — aturar-lo",
            "/splay file — seinn fuaim; /splay stop — stad í",
        ],
    ),
    (
        "stats",
        [
            "/stats letter [server] — server statistics",
            "/stats lettre [serveur] — statistiques du serveur",
            "/stats letter [server] — estadísticas del servidor",
            "/stats letter [server] — Serverstatistik",
            "/stats letter [server] — statistiche del server",
            "/stats letter [server] — estadístiques del servidor",
            "/stats letter [server] — staitisticí an fhreastalaí",
        ],
    ),
    (
        "time",
        [
            "/time [server] — server time",
            "/time [serveur] — heure du serveur",
            "/time [server] — hora del servidor",
            "/time [server] — Uhrzeit des Servers",
            "/time [server] — ora del server",
            "/time [server] — hora del servidor",
            "/time [server] — am an fhreastalaí",
        ],
    ),
    (
        "timer",
        [
            "/timer[name] [-o] repetitions seconds commands — run commands later (off: stop)",
            "/timer[nom] [-o] répétitions secondes commandes — lancer des commandes plus tard (off : arrêter)",
            "/timer[name] [-o] repetitions seconds commands — ejecutar comandos más tarde (off: parar)",
            "/timer[name] [-o] repetitions seconds commands — Befehle später ausführen (off: stoppen)",
            "/timer[name] [-o] repetitions seconds commands — esegui comandi più tardi (off: ferma)",
            "/timer[name] [-o] repetitions seconds commands — executar ordres més avant (off: parar)",
            "/timer[name] [-o] repetitions seconds commands — rith orduithe níos déanaí (off: stad)",
        ],
    ),
    (
        "timers",
        [
            "/timers [off] — list (or stop) the timers",
            "/timers [off] — lister (ou arrêter) les timers",
            "/timers [off] — listar (o parar) los timers",
            "/timers [off] — die Timer auflisten (oder stoppen)",
            "/timers [off] — elenca (o ferma) i timer",
            "/timers [off] — llistar (o parar) els temporitzadors",
            "/timers [off] — liostaigh (nó stad) na lasca ama",
        ],
    ),
    (
        "timestamp",
        [
            "/timestamp [on|off] — timestamps in every window",
            "/timestamp [on|off] — horodatage dans toutes les fenêtres",
            "/timestamp [on|off] — hora en todas las ventanas",
            "/timestamp [on|off] — Zeitstempel in allen Fenstern",
            "/timestamp [on|off] — orario in tutte le finestre",
            "/timestamp [on|off] — marca horària en totes les finestres",
            "/timestamp [on|off] — stampaí ama i ngach fuinneog",
        ],
    ),
    (
        "topic",
        [
            "/topic [#channel] [text] — show or change the topic",
            "/topic [#canal] [texte] — voir ou changer le sujet",
            "/topic [#channel] [text] — ver o cambiar el tema",
            "/topic [#channel] [text] — das Topic anzeigen oder ändern",
            "/topic [#channel] [text] — mostra o cambia il topic",
            "/topic [#channel] [text] — veure o canviar el tema",
            "/topic [#channel] [text] — taispeáin nó athraigh an t-ábhar",
        ],
    ),
    (
        "unload",
        [
            "/unload -rs|-a file — unload a script",
            "/unload -rs|-a fichier — décharger un script",
            "/unload -rs|-a file — descargar un script",
            "/unload -rs|-a file — ein Script entladen",
            "/unload -rs|-a file — scarica uno script",
            "/unload -rs|-a file — descarregar un script",
            "/unload -rs|-a file — díluchtaigh script",
        ],
    ),
    (
        "unset",
        [
            "/unset %var — remove a global variable",
            "/unset %var — retirer une variable globale",
            "/unset %var — quitar una variable global",
            "/unset %var — eine globale Variable entfernen",
            "/unset %var — rimuovi una variabile globale",
            "/unset %var — llevar una variable global",
            "/unset %var — bain athróg dhomhanda",
        ],
    ),
    (
        "url",
        [
            "/url [address] — URL catcher (with an address: open it)",
            "/url [adresse] — adresses attrapées (avec une adresse : l'ouvrir)",
            "/url [address] — capturador de URL (con una dirección: abrirla)",
            "/url [address] — URL-Sammler (mit Adresse: sie öffnen)",
            "/url [address] — URL catturati (con un indirizzo: aprilo)",
            "/url [address] — capturador d'URL (amb una adreça: obrir-la)",
            "/url [address] — bailitheoir URL (le seoladh: oscail é)",
        ],
    ),
    (
        "var",
        [
            "/var %name = value — local variable of a script",
            "/var %nom = valeur — variable locale d'un script",
            "/var %name = value — variable local de un script",
            "/var %name = value — lokale Variable eines Scripts",
            "/var %name = value — variabile locale di uno script",
            "/var %name = value — variable local d'un script",
            "/var %name = value — athróg logánta scripte",
        ],
    ),
    (
        "version",
        [
            "/version [server] — server version",
            "/version [serveur] — version du serveur",
            "/version [server] — versión del servidor",
            "/version [server] — Version des Servers",
            "/version [server] — versione del server",
            "/version [server] — versió del servidor",
            "/version [server] — leagan an fhreastalaí",
        ],
    ),
    (
        "voice",
        [
            "/voice [#channel] nicks — give voice",
            "/voice [#canal] pseudos — donner le voice",
            "/voice [#channel] nicks — dar voz",
            "/voice [#channel] nicks — Voice geben",
            "/voice [#channel] nicks — dai il voice",
            "/voice [#channel] nicks — donar veu",
            "/voice [#channel] nicks — tabhair guth",
        ],
    ),
    (
        "who",
        [
            "/who mask — who is behind a mask",
            "/who masque — qui se cache derrière un masque",
            "/who mask — quién hay detrás de una máscara",
            "/who mask — wer hinter einer Maske steckt",
            "/who mask — chi si nasconde dietro una maschera",
            "/who mask — qui s'amaga darrere d'una màscara",
            "/who mask — cé atá taobh thiar de mhasc",
        ],
    ),
    (
        "uwho",
        [
            "/uwho nick — User Central: someone's whois, and your notes about them",
            "/uwho pseudo — User Central : le whois de quelqu'un, et vos notes sur lui",
            "/uwho nick — User Central: el whois de alguien y tus notas sobre esa persona",
            "/uwho nick — User Central: Whois einer Person und Ihre Notizen zu ihr",
            "/uwho nick — User Central: il whois di qualcuno e le tue note su di lui",
            "/uwho nick — User Central: el whois d'algú i les vostres notes sobre ell",
            "/uwho nick — User Central: whois duine agus do nótaí faoi",
        ],
    ),
    (
        "whois",
        [
            "/whois nick — information about someone",
            "/whois pseudo — informations sur quelqu'un",
            "/whois nick — información sobre alguien",
            "/whois nick — Informationen über jemanden",
            "/whois nick — informazioni su qualcuno",
            "/whois nick — informació sobre algú",
            "/whois nick — eolas faoi dhuine",
        ],
    ),
    (
        "whowas",
        [
            "/whowas nick — who used a nick",
            "/whowas pseudo — qui a porté ce pseudo",
            "/whowas nick — quién usó un nick",
            "/whowas nick — wer einen Nick benutzt hat",
            "/whowas nick — chi ha usato un nick",
            "/whowas nick — qui ha portat un sobrenom",
            "/whowas nick — cé a d'úsáid leasainm",
        ],
    ),
    (
        "window",
        [
            "/window [-acnrx] name — activate, close, minimize, restore, maximize a window; @name: custom window",
            "/window [-acnrx] nom — activer, fermer, réduire, restaurer, agrandir une fenêtre ; @nom : fenêtre @",
            "/window [-acnrx] name — activar, cerrar, minimizar, restaurar, maximizar una ventana; @name: ventana @",
            "/window [-acnrx] name — ein Fenster aktivieren, schließen, minimieren, wiederherstellen, maximieren; \
             @name: @-Fenster",
            "/window [-acnrx] name — attiva, chiudi, riduci, ripristina, ingrandisci una finestra; @name: finestra @",
            "/window [-acnrx] name — activar, tancar, minimitzar, restaurar, maximitzar una finestra; @name: finestra \
             @",
            "/window [-acnrx] name — gníomhachtaigh, dún, íoslaghdaigh, athchóirigh, uasmhéadaigh fuinneog; @name: \
             fuinneog @",
        ],
    ),
];

/// Aide d'une commande dans une langue (anglais si la langue n'est pas encore traduite).
pub fn help_for(lang: Lang, cmd: &str) -> Option<&'static str> {
    let c = cmd.trim_start_matches('/').to_ascii_lowercase();
    let (_, v) = HELP.iter().find(|(k, _)| *k == c)?;
    let i = match lang {
        Lang::En => 0,
        Lang::Fr => 1,
        Lang::Es => 2,
        Lang::De => 3,
        Lang::It => 4,
        Lang::Va => 5,
        Lang::Ga => 6,
    };
    Some(if v[i].is_empty() { v[0] } else { v[i] })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_has_help_in_english_and_french() {
        for b in super::super::commands::BUILTINS {
            assert!(HELP.iter().any(|(k, _)| k == b), "pas d'aide pour /{b}");
        }
        for (k, v) in HELP {
            assert!(v[0].starts_with(&format!("/{k}")) && v[1].starts_with(&format!("/{k}")), "{k}");
            assert!(v[0].contains(" — ") && v[1].contains(" — "), "{k}");
        }
        let mut keys: Vec<&str> = HELP.iter().map(|(k, _)| *k).collect();
        let n = keys.len();
        keys.dedup();
        assert_eq!(n, keys.len(), "commande en double");
        assert_eq!(help_for(Lang::De, "/JOIN"), Some("/join #channel[,#other] [key] — Channels betreten"));
        assert!(help_for(Lang::Fr, "join").unwrap().contains("rejoindre"));
        assert_eq!(help_for(Lang::En, "nope"), None);
    }

    #[test]
    fn translations_keep_the_english_usage() {
        // Les cinq autres langues gardent l'usage anglais (avant « — ») et ne traduisent que le rôle.
        for (k, v) in HELP {
            let usage = v[0].split(" — ").next().unwrap_or("");
            for s in v[2..].iter().filter(|s| !s.is_empty()) {
                assert!(s.starts_with(&format!("{usage} — ")), "{k} : « {s} »");
            }
        }
    }

    // Toute nouvelle clé doit être traduite dans les 7 langues (voir l'ADR 0012).
    #[test]
    fn every_help_in_seven_languages() {
        for (k, v) in HELP {
            assert!(v.iter().all(|s| !s.is_empty()), "{k} : traduction manquante");
            assert!(v[2..].iter().any(|s| *s != v[0]), "{k} : identique à l'anglais partout");
        }
    }
}
