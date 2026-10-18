//! Textes de l'interface du moteur, en sept langues (ADR 0012) : anglais, français, espagnol, allemand, italien,
//! valencien, irlandais. L'anglais et le français sont dans `keys!`, les cinq autres dans `OTHER` ; une clé qui
//! n'y est pas encore retombe sur l'anglais ([`coverage`] dit où on en est).
//!
//! Les textes ont des marques nommées (`{nick}`, `{chan}`…) remplacées par [`trf`]. Les phrases anglaises suivent
//! mIRC 6 (`* Joins: nick (user@host)`).

/// Langue de l'interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Lang {
    #[default]
    En,
    Fr,
    Es,
    De,
    It,
    /// Valencien.
    Va,
    /// Irlandais.
    Ga,
}

impl Lang {
    pub const ALL: [Lang; 7] = [Lang::En, Lang::Fr, Lang::Es, Lang::De, Lang::It, Lang::Va, Lang::Ga];

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Fr => "fr",
            Lang::Es => "es",
            Lang::De => "de",
            Lang::It => "it",
            Lang::Va => "va",
            Lang::Ga => "ga",
        }
    }

    /// Nom de la langue dans la langue elle-même (pour les Options).
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Fr => "Français",
            Lang::Es => "Español",
            Lang::De => "Deutsch",
            Lang::It => "Italiano",
            Lang::Va => "Valencià",
            Lang::Ga => "Gaeilge",
        }
    }

    /// Langue d'un code (`fr`) ou d'une locale du système (`fr_FR.UTF-8`, `ca-ES-valencia`, `ga_IE`).
    pub fn from_locale(s: &str) -> Option<Lang> {
        let l = s.trim().to_ascii_lowercase().replace('-', "_");
        if l.contains("valencia") || l.starts_with("va") {
            return Some(Lang::Va);
        }
        let base = l.split(['_', '.', '@']).next().unwrap_or("");
        Some(match base {
            "en" => Lang::En,
            "fr" => Lang::Fr,
            "es" => Lang::Es,
            "de" => Lang::De,
            "it" => Lang::It,
            // Le catalan du système : le valencien en est la variante que nous avons.
            "ca" => Lang::Va,
            "ga" => Lang::Ga,
            _ => return None,
        })
    }
}

macro_rules! keys {
    ($($key:ident => $en:expr, $fr:expr;)*) => {
        /// Clé d'un texte de l'interface.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Key { $($key),* }

        impl Key {
            pub const ALL: &'static [Key] = &[$(Key::$key),*];

            fn en(self) -> &'static str {
                match self { $(Key::$key => $en),* }
            }

            fn fr(self) -> &'static str {
                match self { $(Key::$key => $fr),* }
            }
        }
    };
}

keys! {
    // --- connexion ---
    Connecting => "* Connecting to {host} ({port})", "* Connexion à {host} ({port})";
    Connected => "* Connected", "* Connecté";
    ConnectFailed => "* Unable to connect to server ({error})", "* Impossible de se connecter au serveur ({error})";
    Disconnected => "* Disconnected", "* Déconnecté";
    DisconnectedReason => "* Disconnected ({error})", "* Déconnecté ({error})";
    Retrying => "* Retrying in {secs} seconds (attempt {n})...", "* Nouvel essai dans {secs} secondes (essai {n})…";
    PingTimeout => "* Ping timeout ({secs} seconds)", "* Délai de ping dépassé ({secs} secondes)";
    NotConnected => "* Not connected to server", "* Pas connecté à un serveur";
    NotOnChannel => "* You're not on a channel", "* Vous n'êtes pas sur un canal";
    StsUpgrade => "* STS: switching to a secure connection on port {port}", "* STS : passage à une connexion sécurisée sur le port {port}";
    StsStored => "* STS: {host} requires secure connections", "* STS : {host} exige des connexions sécurisées";
    SaslFailed => "* SASL authentication failed: {reason}", "* Échec de l'authentification SASL : {reason}";
    NickRetry => "* Nickname {nick} is unavailable, trying {next}", "* Le pseudo {nick} est indisponible, essai avec {next}";
    BouncerNetworks => "* Bouncer networks: {list}", "* Réseaux du bouncer : {list}";
    Lag => "* Lag: {ms}ms", "* Latence : {ms} ms";
    NoServer => "* No server for network {network}", "* Aucun serveur pour le réseau {network}";
    // --- canaux ---
    NowTalking => "* Now talking in {chan}", "* Vous parlez maintenant dans {chan}";
    TopicIs => "* Topic is '{topic}'", "* Le sujet est « {topic} »";
    TopicSetBy => "* Set by {nick} on {date}", "* Posé par {nick} le {date}";
    NoTopic => "* No topic is set for {chan}", "* Aucun sujet sur {chan}";
    Joins => "* Joins: {nick} ({address})", "* Arrivée : {nick} ({address})";
    Parts => "* Parts: {nick} ({address})", "* Départ : {nick} ({address})";
    PartsReason => "* Parts: {nick} ({address}) ({reason})", "* Départ : {nick} ({address}) ({reason})";
    Quits => "* Quits: {nick} ({address})", "* Quitte IRC : {nick} ({address})";
    QuitsReason => "* Quits: {nick} ({address}) ({reason})", "* Quitte IRC : {nick} ({address}) ({reason})";
    Kicked => "* {nick} was kicked by {by} ({reason})", "* {nick} a été expulsé par {by} ({reason})";
    YouKicked => "* You were kicked from {chan} by {by} ({reason})", "* Vous avez été expulsé de {chan} par {by} ({reason})";
    NickChange => "* {nick} is now known as {new}", "* {nick} s'appelle maintenant {new}";
    OwnNick => "* Your nick is now {new}", "* Votre pseudo est maintenant {new}";
    SetsMode => "* {nick} sets mode: {modes}", "* {nick} change les modes : {modes}";
    ChangesTopic => "* {nick} changes topic to '{topic}'", "* {nick} change le sujet en « {topic} »";
    Invites => "* {nick} ({address}) invites you to join {chan}", "* {nick} ({address}) vous invite sur {chan}";
    InviteNotify => "* {nick} invited {target} into {chan}", "* {nick} a invité {target} sur {chan}";
    ChannelModes => "* {chan} modes: {modes}", "* Modes de {chan} : {modes}";
    CreatedOn => "* {chan} created on {date}", "* {chan} créé le {date}";
    Names => "* Names {chan}: {names}", "* Pseudos de {chan} : {names}";
    ListEntryFmt => "{chan} {mask} set by {setter} on {date}", "{chan} {mask} posé par {setter} le {date}";
    // --- messages ---
    MsgSent => "-> *{nick}* {text}", "-> *{nick}* {text}";
    NoticeSent => "-> -{nick}- {text}", "-> -{nick}- {text}";
    CtcpSent => "-> [{nick}] {cmd}", "-> [{nick}] {cmd}";
    CtcpReply => "[{nick} {cmd} reply]: {text}", "[{nick} {cmd} réponse] : {text}";
    PingSecs => "{secs}secs", "{secs} s";
    DccSendOffer => "* DCC SEND offer from {nick}: {file} ({size})", "* Offre DCC SEND de {nick} : {file} ({size})";
    DccChatOffer => "* DCC CHAT offer from {nick}", "* Offre DCC CHAT de {nick}";
    DccUnavailable => "* DCC is not available yet", "* Le DCC n'est pas encore disponible";
    AwayMsg => "{nick} is away: {msg}", "{nick} est absent : {msg}";
    // --- whois ---
    WhoisUser => "{nick} is {address} * {realname}", "{nick} est {address} * {realname}";
    WhoisChannels => "{nick} on {chans}", "{nick} sur {chans}";
    WhoisServer => "{nick} using {server} {info}", "{nick} utilise {server} {info}";
    WhoisIdle => "{nick} has been idle {idle}, signed on {signon}", "{nick} est inactif depuis {idle}, connecté le {signon}";
    WhoisAccount => "{nick} is logged in as {account}", "{nick} est identifié comme {account}";
    WhoisSecure => "{nick} is using a secure connection", "{nick} utilise une connexion sécurisée";
    WhoisEnd => "{nick} End of /WHOIS list.", "{nick} Fin de la liste /WHOIS.";
    WhowasUser => "{nick} was {address} * {realname}", "{nick} était {address} * {realname}";
    // --- carnet d'adresses ---
    NotifyOn => "* {nick} is on IRC", "* {nick} est sur IRC";
    NotifyOff => "* {nick} has left IRC", "* {nick} a quitté IRC";
    NotifyAdded => "* Added {nick} to notify list", "* {nick} ajouté à la liste Notify";
    NotifyRemoved => "* Removed {nick} from notify list", "* {nick} retiré de la liste Notify";
    NotifyList => "* Notify list: {list}", "* Liste Notify : {list}";
    IgnoreAdded => "* Ignoring {mask}", "* {mask} est ignoré";
    IgnoreRemoved => "* No longer ignoring {mask}", "* {mask} n'est plus ignoré";
    IgnoreList => "* Ignore list: {list}", "* Liste des ignorés : {list}";
    ListEmpty => "* The list is empty", "* La liste est vide";
    // --- commandes ---
    InsufficientParams => "/{cmd}: insufficient parameters", "/{cmd} : paramètres insuffisants";
    UnknownCommand => "/{cmd}: unknown command", "/{cmd} : commande inconnue";
    NoSuchWindow => "/{cmd}: no such window '{name}'", "/{cmd} : pas de fenêtre « {name} »";
    RunDisabled => "* /run is disabled in AlpIRC", "* /run est désactivé dans AlpIRC";
    DnsUnsupported => "* /dns is not supported by AlpIRC", "* /dns n'est pas pris en charge par AlpIRC";
    LoggingOn => "* Logging is on for {win}", "* Journal activé pour {win}";
    LoggingOff => "* Logging is off for {win}", "* Journal désactivé pour {win}";
    Help => "* AlpIRC help: {url}", "* Aide d'AlpIRC : {url}";
    ListStart => "* Retrieving channel list...", "* Récupération de la liste des canaux…";
    ListDone => "* Channel list: {n} channels", "* Liste des canaux : {n} canaux";
    SoundNotFound => "/splay: file not found '{file}'", "/splay : fichier introuvable « {file} »";
    SoundWants => "* {nick} wants you to hear {file}", "* {nick} veut vous faire écouter {file}";
    SoundRequestsOn => "* Sound requests are accepted", "* Demandes de sons acceptées";
    SoundRequestsOff => "* Sound requests are refused", "* Demandes de sons refusées";
    UnknownServer => "/server: unknown network or server '{name}'", "/server : réseau ou serveur inconnu « {name} »";
    ScriptLoadError => "* Unable to load script {file}: {error}", "* Impossible de charger le script {file} : {error}";
    TimestampOn => "* Timestamping is on", "* Horodatage activé";
    TimestampOff => "* Timestamping is off", "* Horodatage désactivé";
    IalAlwaysOn => "* The internal address list (IAL) is always on in AlpIRC",
        "* La liste interne des adresses (IAL) est toujours active dans AlpIRC";
    HelpHead => "* AlpIRC commands (type /help command for details):",
        "* Commandes d'AlpIRC (tapez /help commande pour le détail) :";
    HelpNone => "* No help for /{cmd}", "* Pas d'aide pour /{cmd}";
    HelpMore => "* Full guide: {url}", "* Guide complet : {url}";
    TitleNotify => "Notify List", "Liste Notify";
    // --- titres, notifications ---
    TitleStatus => "Status: {nick} [{modes}] on {network} ({server}:{port})", "Status : {nick} [{modes}] sur {network} ({server}:{port})";
    TitleStatusOffline => "Status: {nick} (not connected)", "Status : {nick} (non connecté)";
    TitleChanList => "Channels List", "Liste des canaux";
    NotifHighlight => "{nick} on {chan}", "{nick} sur {chan}";
    NotifQuery => "Private message from {nick}", "Message privé de {nick}";
}

/// Traductions des autres langues : (clé, [es, de, it, va, ga]). Une clé absente (ou vide) retombe sur l'anglais.
fn other(lang: Lang, key: Key) -> Option<&'static str> {
    let col = match lang {
        Lang::En | Lang::Fr => return None,
        Lang::Es => 0,
        Lang::De => 1,
        Lang::It => 2,
        Lang::Va => 3,
        Lang::Ga => 4,
    };
    OTHER.iter().find(|(k, _)| *k == key).map(|(_, v)| v[col]).filter(|s| !s.is_empty())
}

/// (clé, [es, de, it, va, ga]) ; même ordre que [`keys!`].
const OTHER: &[(Key, [&str; 5])] = &[
    (
        Key::Connecting,
        [
            "* Conectando con {host} ({port})",
            "* Verbinde mit {host} ({port})",
            "* Connessione a {host} ({port})",
            "* Connectant amb {host} ({port})",
            "* Ag ceangal le {host} ({port})",
        ],
    ),
    (Key::Connected, ["* Conectado", "* Verbunden", "* Connesso", "* Connectat", "* Ceangailte"]),
    (
        Key::ConnectFailed,
        [
            "* No se puede conectar con el servidor ({error})",
            "* Verbindung zum Server nicht möglich ({error})",
            "* Impossibile connettersi al server ({error})",
            "* No es pot connectar amb el servidor ({error})",
            "* Ní féidir ceangal leis an bhfreastalaí ({error})",
        ],
    ),
    (Key::Disconnected, ["* Desconectado", "* Getrennt", "* Disconnesso", "* Desconnectat", "* Dícheangailte"]),
    (
        Key::DisconnectedReason,
        [
            "* Desconectado ({error})",
            "* Getrennt ({error})",
            "* Disconnesso ({error})",
            "* Desconnectat ({error})",
            "* Dícheangailte ({error})",
        ],
    ),
    (
        Key::Retrying,
        [
            "* Reintentando en {secs} segundos (intento {n})...",
            "* Neuer Versuch in {secs} Sekunden ({n}. Versuch)...",
            "* Nuovo tentativo tra {secs} secondi (tentativo {n})...",
            "* Nou intent d'ací a {secs} segons (intent {n})...",
            "* Triail eile i gceann {secs} soicind (iarracht {n})...",
        ],
    ),
    (
        Key::PingTimeout,
        [
            "* Tiempo de ping agotado ({secs} segundos)",
            "* Ping-Timeout ({secs} Sekunden)",
            "* Timeout del ping ({secs} secondi)",
            "* Temps d'espera del ping esgotat ({secs} segons)",
            "* Teorainn ama ping sáraithe ({secs} soicind)",
        ],
    ),
    (
        Key::NotConnected,
        [
            "* No hay conexión con ningún servidor",
            "* Nicht mit einem Server verbunden",
            "* Non connesso a un server",
            "* No connectat a cap servidor",
            "* Gan cheangal le freastalaí",
        ],
    ),
    (
        Key::NotOnChannel,
        [
            "* No estás en ningún canal",
            "* Sie sind in keinem Channel",
            "* Non sei in un canale",
            "* No esteu en cap canal",
            "* Níl tú ar chainéal",
        ],
    ),
    (
        Key::StsUpgrade,
        [
            "* STS: cambiando a una conexión segura en el puerto {port}",
            "* STS: Wechsel zu einer sicheren Verbindung auf Port {port}",
            "* STS: passaggio a una connessione sicura sulla porta {port}",
            "* STS: canvi a una connexió segura pel port {port}",
            "* STS: ag aistriú go ceangal slán ar phort {port}",
        ],
    ),
    (
        Key::StsStored,
        [
            "* STS: {host} exige conexiones seguras",
            "* STS: {host} verlangt sichere Verbindungen",
            "* STS: {host} richiede connessioni sicure",
            "* STS: {host} exigix connexions segures",
            "* STS: éilíonn {host} ceangail shlána",
        ],
    ),
    (
        Key::SaslFailed,
        [
            "* Falló la autenticación SASL: {reason}",
            "* SASL-Authentifizierung fehlgeschlagen: {reason}",
            "* Autenticazione SASL non riuscita: {reason}",
            "* Ha fallat l'autenticació SASL: {reason}",
            "* Theip ar fhíordheimhniú SASL: {reason}",
        ],
    ),
    (
        Key::NickRetry,
        [
            "* El nick {nick} no está disponible, probando con {next}",
            "* Nickname {nick} ist nicht verfügbar, versuche {next}",
            "* Il nick {nick} non è disponibile, provo con {next}",
            "* El sobrenom {nick} no està disponible, prove amb {next}",
            "* Níl an leasainm {nick} ar fáil, ag triail {next}",
        ],
    ),
    (
        Key::BouncerNetworks,
        [
            "* Redes del bouncer: {list}",
            "* Bouncer-Netzwerke: {list}",
            "* Reti del bouncer: {list}",
            "* Xarxes del bouncer: {list}",
            "* Líonraí bouncer: {list}",
        ],
    ),
    (Key::Lag, ["* Lag: {ms} ms", "* Lag: {ms} ms", "* Lag: {ms} ms", "* Latència: {ms} ms", "* Moill: {ms} ms"]),
    (
        Key::NoServer,
        [
            "* No hay ningún servidor para la red {network}",
            "* Kein Server für das Netzwerk {network}",
            "* Nessun server per la rete {network}",
            "* No hi ha cap servidor per a la xarxa {network}",
            "* Níl freastalaí ar bith don líonra {network}",
        ],
    ),
    (
        Key::NowTalking,
        [
            "* Ahora hablas en {chan}",
            "* Sie sprechen jetzt in {chan}",
            "* Ora stai parlando in {chan}",
            "* Ara parleu en {chan}",
            "* Tá tú ag caint i {chan} anois",
        ],
    ),
    (
        Key::TopicIs,
        [
            "* El tema es «{topic}»",
            "* Das Topic lautet „{topic}“",
            "* Il topic è «{topic}»",
            "* El tema és «{topic}»",
            "* An t-ábhar: '{topic}'",
        ],
    ),
    (
        Key::TopicSetBy,
        [
            "* Establecido por {nick} el {date}",
            "* Gesetzt von {nick} am {date}",
            "* Impostato da {nick} il {date}",
            "* Establit per {nick} el {date}",
            "* Socraithe ag {nick} ar {date}",
        ],
    ),
    (
        Key::NoTopic,
        [
            "* {chan} no tiene tema",
            "* Für {chan} ist kein Topic gesetzt",
            "* Nessun topic impostato per {chan}",
            "* {chan} no té cap tema",
            "* Níl ábhar socraithe do {chan}",
        ],
    ),
    (
        Key::Joins,
        [
            "* Entra: {nick} ({address})",
            "* Betritt: {nick} ({address})",
            "* Entra: {nick} ({address})",
            "* Entra: {nick} ({address})",
            "* Isteach: {nick} ({address})",
        ],
    ),
    (
        Key::Parts,
        [
            "* Sale: {nick} ({address})",
            "* Verlässt: {nick} ({address})",
            "* Esce: {nick} ({address})",
            "* Ix: {nick} ({address})",
            "* Amach: {nick} ({address})",
        ],
    ),
    (
        Key::PartsReason,
        [
            "* Sale: {nick} ({address}) ({reason})",
            "* Verlässt: {nick} ({address}) ({reason})",
            "* Esce: {nick} ({address}) ({reason})",
            "* Ix: {nick} ({address}) ({reason})",
            "* Amach: {nick} ({address}) ({reason})",
        ],
    ),
    (
        Key::Quits,
        [
            "* Sale de IRC: {nick} ({address})",
            "* Verlässt IRC: {nick} ({address})",
            "* Esce da IRC: {nick} ({address})",
            "* Ix d'IRC: {nick} ({address})",
            "* Fágann IRC: {nick} ({address})",
        ],
    ),
    (
        Key::QuitsReason,
        [
            "* Sale de IRC: {nick} ({address}) ({reason})",
            "* Verlässt IRC: {nick} ({address}) ({reason})",
            "* Esce da IRC: {nick} ({address}) ({reason})",
            "* Ix d'IRC: {nick} ({address}) ({reason})",
            "* Fágann IRC: {nick} ({address}) ({reason})",
        ],
    ),
    (
        Key::Kicked,
        [
            "* {nick} ha sido expulsado por {by} ({reason})",
            "* {nick} wurde von {by} gekickt ({reason})",
            "* {nick} è stato espulso da {by} ({reason})",
            "* {nick} ha sigut expulsat per {by} ({reason})",
            "* Chaith {by} {nick} amach ({reason})",
        ],
    ),
    (
        Key::YouKicked,
        [
            "* {by} te ha expulsado de {chan} ({reason})",
            "* Sie wurden von {by} aus {chan} gekickt ({reason})",
            "* {by} ti ha espulso da {chan} ({reason})",
            "* {by} vos ha expulsat de {chan} ({reason})",
            "* Chaith {by} amach thú as {chan} ({reason})",
        ],
    ),
    (
        Key::NickChange,
        [
            "* {nick} ahora se llama {new}",
            "* {nick} heißt jetzt {new}",
            "* {nick} ora si chiama {new}",
            "* {nick} ara es diu {new}",
            "* Is é {new} an leasainm atá ar {nick} anois",
        ],
    ),
    (
        Key::OwnNick,
        [
            "* Tu nick es ahora {new}",
            "* Ihr Nick ist jetzt {new}",
            "* Il tuo nick ora è {new}",
            "* El vostre sobrenom ara és {new}",
            "* Is é {new} do leasainm anois",
        ],
    ),
    (
        Key::SetsMode,
        [
            "* {nick} pone modo: {modes}",
            "* {nick} setzt Modus: {modes}",
            "* {nick} imposta il modo: {modes}",
            "* {nick} posa el mode: {modes}",
            "* Socraíonn {nick} mód: {modes}",
        ],
    ),
    (
        Key::ChangesTopic,
        [
            "* {nick} cambia el tema a «{topic}»",
            "* {nick} ändert das Topic in „{topic}“",
            "* {nick} cambia il topic in «{topic}»",
            "* {nick} canvia el tema a «{topic}»",
            "* Athraíonn {nick} an t-ábhar go '{topic}'",
        ],
    ),
    (
        Key::Invites,
        [
            "* {nick} ({address}) te invita a entrar en {chan}",
            "* {nick} ({address}) lädt Sie in {chan} ein",
            "* {nick} ({address}) ti invita su {chan}",
            "* {nick} ({address}) vos convida a entrar en {chan}",
            "* Tugann {nick} ({address}) cuireadh duit chuig {chan}",
        ],
    ),
    (
        Key::InviteNotify,
        [
            "* {nick} ha invitado a {target} a {chan}",
            "* {nick} hat {target} in {chan} eingeladen",
            "* {nick} ha invitato {target} su {chan}",
            "* {nick} ha convidat {target} a {chan}",
            "* Thug {nick} cuireadh do {target} chuig {chan}",
        ],
    ),
    (
        Key::ChannelModes,
        [
            "* Modos de {chan}: {modes}",
            "* Modi von {chan}: {modes}",
            "* Modi di {chan}: {modes}",
            "* Modes de {chan}: {modes}",
            "* Móid {chan}: {modes}",
        ],
    ),
    (
        Key::CreatedOn,
        [
            "* {chan} creado el {date}",
            "* {chan} erstellt am {date}",
            "* {chan} creato il {date}",
            "* {chan} creat el {date}",
            "* Cruthaíodh {chan} ar {date}",
        ],
    ),
    (
        Key::Names,
        [
            "* Nicks en {chan}: {names}",
            "* Nicks in {chan}: {names}",
            "* Nick in {chan}: {names}",
            "* Sobrenoms de {chan}: {names}",
            "* Leasainmneacha {chan}: {names}",
        ],
    ),
    (
        Key::ListEntryFmt,
        [
            "{chan} {mask} puesto por {setter} el {date}",
            "{chan} {mask} gesetzt von {setter} am {date}",
            "{chan} {mask} impostato da {setter} il {date}",
            "{chan} {mask} posat per {setter} el {date}",
            "{chan} {mask} socraithe ag {setter} ar {date}",
        ],
    ),
    (
        Key::MsgSent,
        ["-> *{nick}* {text}", "-> *{nick}* {text}", "-> *{nick}* {text}", "-> *{nick}* {text}", "-> *{nick}* {text}"],
    ),
    (
        Key::NoticeSent,
        ["-> -{nick}- {text}", "-> -{nick}- {text}", "-> -{nick}- {text}", "-> -{nick}- {text}", "-> -{nick}- {text}"],
    ),
    (
        Key::CtcpSent,
        ["-> [{nick}] {cmd}", "-> [{nick}] {cmd}", "-> [{nick}] {cmd}", "-> [{nick}] {cmd}", "-> [{nick}] {cmd}"],
    ),
    (
        Key::CtcpReply,
        [
            "[{nick} {cmd} respuesta]: {text}",
            "[{nick} {cmd}-Antwort]: {text}",
            "[{nick} risposta {cmd}]: {text}",
            "[{nick} resposta {cmd}]: {text}",
            "[{nick} freagra {cmd}]: {text}",
        ],
    ),
    (Key::PingSecs, ["{secs} s", "{secs} s", "{secs} s", "{secs} s", "{secs} s"]),
    (
        Key::DccSendOffer,
        [
            "* Oferta de DCC SEND de {nick}: {file} ({size})",
            "* DCC-SEND-Angebot von {nick}: {file} ({size})",
            "* Offerta DCC SEND da {nick}: {file} ({size})",
            "* Oferta DCC SEND de {nick}: {file} ({size})",
            "* Tairiscint DCC SEND ó {nick}: {file} ({size})",
        ],
    ),
    (
        Key::DccChatOffer,
        [
            "* Oferta de DCC CHAT de {nick}",
            "* DCC-CHAT-Angebot von {nick}",
            "* Offerta DCC CHAT da {nick}",
            "* Oferta DCC CHAT de {nick}",
            "* Tairiscint DCC CHAT ó {nick}",
        ],
    ),
    (
        Key::DccUnavailable,
        [
            "* El DCC aún no está disponible",
            "* DCC ist noch nicht verfügbar",
            "* Il DCC non è ancora disponibile",
            "* El DCC encara no està disponible",
            "* Níl DCC ar fáil fós",
        ],
    ),
    (
        Key::AwayMsg,
        [
            "{nick} está ausente: {msg}",
            "{nick} ist abwesend: {msg}",
            "{nick} è assente: {msg}",
            "{nick} està absent: {msg}",
            "Tá {nick} as láthair: {msg}",
        ],
    ),
    (
        Key::WhoisUser,
        [
            "{nick} es {address} * {realname}",
            "{nick} ist {address} * {realname}",
            "{nick} è {address} * {realname}",
            "{nick} és {address} * {realname}",
            "{nick}: {address} * {realname}",
        ],
    ),
    (
        Key::WhoisChannels,
        ["{nick} en {chans}", "{nick} in {chans}", "{nick} su {chans}", "{nick} en {chans}", "{nick} ar {chans}"],
    ),
    (
        Key::WhoisServer,
        [
            "{nick} usa {server} {info}",
            "{nick} benutzt {server} {info}",
            "{nick} usa {server} {info}",
            "{nick} usa {server} {info}",
            "{nick} ag úsáid {server} {info}",
        ],
    ),
    (
        Key::WhoisIdle,
        [
            "{nick} lleva inactivo {idle}, conectado desde {signon}",
            "{nick} ist seit {idle} inaktiv, angemeldet seit {signon}",
            "{nick} è inattivo da {idle}, connesso dal {signon}",
            "{nick} està inactiu des de fa {idle}, connectat el {signon}",
            "Tá {nick} díomhaoin le {idle}, ceangailte ó {signon}",
        ],
    ),
    (
        Key::WhoisAccount,
        [
            "{nick} está identificado como {account}",
            "{nick} ist eingeloggt als {account}",
            "{nick} è identificato come {account}",
            "{nick} està identificat com a {account}",
            "Tá {nick} logáilte isteach mar {account}",
        ],
    ),
    (
        Key::WhoisSecure,
        [
            "{nick} usa una conexión segura",
            "{nick} benutzt eine sichere Verbindung",
            "{nick} usa una connessione sicura",
            "{nick} usa una connexió segura",
            "Tá ceangal slán ag {nick}",
        ],
    ),
    (
        Key::WhoisEnd,
        [
            "{nick} Fin de la lista /WHOIS.",
            "{nick} Ende der /WHOIS-Liste.",
            "{nick} Fine della lista /WHOIS.",
            "{nick} Fi de la llista /WHOIS.",
            "{nick} Deireadh an liosta /WHOIS.",
        ],
    ),
    (
        Key::WhowasUser,
        [
            "{nick} era {address} * {realname}",
            "{nick} war {address} * {realname}",
            "{nick} era {address} * {realname}",
            "{nick} era {address} * {realname}",
            "{nick} (roimhe seo): {address} * {realname}",
        ],
    ),
    (
        Key::NotifyOn,
        [
            "* {nick} está en IRC",
            "* {nick} ist im IRC",
            "* {nick} è su IRC",
            "* {nick} està en IRC",
            "* Tá {nick} ar IRC",
        ],
    ),
    (
        Key::NotifyOff,
        [
            "* {nick} ha salido de IRC",
            "* {nick} hat IRC verlassen",
            "* {nick} ha lasciato IRC",
            "* {nick} ha eixit d'IRC",
            "* D'fhág {nick} IRC",
        ],
    ),
    (
        Key::NotifyAdded,
        [
            "* {nick} añadido a la lista Notify",
            "* {nick} zur Notify-Liste hinzugefügt",
            "* {nick} aggiunto alla lista Notify",
            "* {nick} afegit a la llista Notify",
            "* Cuireadh {nick} leis an liosta Notify",
        ],
    ),
    (
        Key::NotifyRemoved,
        [
            "* {nick} quitado de la lista Notify",
            "* {nick} aus der Notify-Liste entfernt",
            "* {nick} rimosso dalla lista Notify",
            "* {nick} llevat de la llista Notify",
            "* Baineadh {nick} den liosta Notify",
        ],
    ),
    (
        Key::NotifyList,
        [
            "* Lista Notify: {list}",
            "* Notify-Liste: {list}",
            "* Lista Notify: {list}",
            "* Llista Notify: {list}",
            "* Liosta Notify: {list}",
        ],
    ),
    (
        Key::IgnoreAdded,
        [
            "* Ignorando a {mask}",
            "* {mask} wird ignoriert",
            "* {mask} viene ignorato",
            "* S'ignora {mask}",
            "* Ag déanamh neamhaird de {mask}",
        ],
    ),
    (
        Key::IgnoreRemoved,
        [
            "* Ya no se ignora a {mask}",
            "* {mask} wird nicht mehr ignoriert",
            "* {mask} non viene più ignorato",
            "* Ja no s'ignora {mask}",
            "* Níltear ag déanamh neamhaird de {mask} a thuilleadh",
        ],
    ),
    (
        Key::IgnoreList,
        [
            "* Lista de ignorados: {list}",
            "* Ignore-Liste: {list}",
            "* Lista degli ignorati: {list}",
            "* Llista d'ignorats: {list}",
            "* Liosta neamhairde: {list}",
        ],
    ),
    (
        Key::ListEmpty,
        [
            "* La lista está vacía",
            "* Die Liste ist leer",
            "* La lista è vuota",
            "* La llista està buida",
            "* Tá an liosta folamh",
        ],
    ),
    (
        Key::InsufficientParams,
        [
            "/{cmd}: faltan parámetros",
            "/{cmd}: zu wenige Parameter",
            "/{cmd}: parametri insufficienti",
            "/{cmd}: falten paràmetres",
            "/{cmd}: easpa paraiméadar",
        ],
    ),
    (
        Key::UnknownCommand,
        [
            "/{cmd}: comando desconocido",
            "/{cmd}: unbekannter Befehl",
            "/{cmd}: comando sconosciuto",
            "/{cmd}: ordre desconeguda",
            "/{cmd}: ordú anaithnid",
        ],
    ),
    (
        Key::NoSuchWindow,
        [
            "/{cmd}: no existe la ventana «{name}»",
            "/{cmd}: kein Fenster „{name}“",
            "/{cmd}: nessuna finestra «{name}»",
            "/{cmd}: no hi ha cap finestra «{name}»",
            "/{cmd}: níl fuinneog ann darb ainm '{name}'",
        ],
    ),
    (
        Key::RunDisabled,
        [
            "* /run está desactivado en AlpIRC",
            "* /run ist in AlpIRC deaktiviert",
            "* /run è disattivato in AlpIRC",
            "* /run està desactivat en AlpIRC",
            "* Tá /run díchumasaithe in AlpIRC",
        ],
    ),
    (
        Key::DnsUnsupported,
        [
            "* AlpIRC no admite /dns",
            "* /dns wird von AlpIRC nicht unterstützt",
            "* /dns non è supportato da AlpIRC",
            "* AlpIRC no admet /dns",
            "* Ní thacaíonn AlpIRC le /dns",
        ],
    ),
    (
        Key::LoggingOn,
        [
            "* Registro activado para {win}",
            "* Logging für {win} aktiviert",
            "* Log attivato per {win}",
            "* Registre activat per a {win}",
            "* Tá logáil ar siúl do {win}",
        ],
    ),
    (
        Key::LoggingOff,
        [
            "* Registro desactivado para {win}",
            "* Logging für {win} deaktiviert",
            "* Log disattivato per {win}",
            "* Registre desactivat per a {win}",
            "* Tá logáil múchta do {win}",
        ],
    ),
    (
        Key::Help,
        [
            "* Ayuda de AlpIRC: {url}",
            "* AlpIRC-Hilfe: {url}",
            "* Guida di AlpIRC: {url}",
            "* Ajuda d'AlpIRC: {url}",
            "* Cabhair AlpIRC: {url}",
        ],
    ),
    (
        Key::ListStart,
        [
            "* Obteniendo la lista de canales...",
            "* Channelliste wird abgerufen...",
            "* Recupero della lista dei canali...",
            "* S'està obtenint la llista de canals...",
            "* Ag fáil liosta na gcainéal...",
        ],
    ),
    (
        Key::ListDone,
        [
            "* Lista de canales: {n} canales",
            "* Channelliste: {n} Channels",
            "* Lista dei canali: {n} canali",
            "* Llista de canals: {n} canals",
            "* Liosta cainéal: {n} cainéal",
        ],
    ),
    (
        Key::SoundNotFound,
        [
            "/splay: archivo no encontrado «{file}»",
            "/splay: Datei nicht gefunden „{file}“",
            "/splay: file non trovato «{file}»",
            "/splay: no s'ha trobat el fitxer «{file}»",
            "/splay: comhad gan aimsiú '{file}'",
        ],
    ),
    (
        Key::SoundWants,
        [
            "* {nick} quiere que escuches {file}",
            "* {nick} möchte, dass Sie {file} hören",
            "* {nick} vuole farti ascoltare {file}",
            "* {nick} vol que escolteu {file}",
            "* Ba mhaith le {nick} go gcloisfeá {file}",
        ],
    ),
    (
        Key::SoundRequestsOn,
        [
            "* Se aceptan las peticiones de sonido",
            "* Klanganfragen werden angenommen",
            "* Richieste di suoni accettate",
            "* S'accepten les peticions de so",
            "* Glactar le hiarratais fuaime",
        ],
    ),
    (
        Key::SoundRequestsOff,
        [
            "* Se rechazan las peticiones de sonido",
            "* Klanganfragen werden abgelehnt",
            "* Richieste di suoni rifiutate",
            "* Es rebutgen les peticions de so",
            "* Diúltaítear d'iarratais fuaime",
        ],
    ),
    (
        Key::UnknownServer,
        [
            "/server: red o servidor desconocido «{name}»",
            "/server: unbekanntes Netzwerk oder unbekannter Server „{name}“",
            "/server: rete o server sconosciuto «{name}»",
            "/server: xarxa o servidor desconegut «{name}»",
            "/server: líonra nó freastalaí anaithnid '{name}'",
        ],
    ),
    (
        Key::ScriptLoadError,
        [
            "* No se puede cargar el script {file}: {error}",
            "* Script {file} kann nicht geladen werden: {error}",
            "* Impossibile caricare lo script {file}: {error}",
            "* No es pot carregar l'script {file}: {error}",
            "* Ní féidir an script {file} a lódáil: {error}",
        ],
    ),
    (
        Key::TimestampOn,
        [
            "* Marca de hora activada",
            "* Zeitstempel aktiviert",
            "* Orario attivato",
            "* Marca horària activada",
            "* Tá stampáil ama ar siúl",
        ],
    ),
    (
        Key::TimestampOff,
        [
            "* Marca de hora desactivada",
            "* Zeitstempel deaktiviert",
            "* Orario disattivato",
            "* Marca horària desactivada",
            "* Tá stampáil ama múchta",
        ],
    ),
    (
        Key::IalAlwaysOn,
        [
            "* La lista interna de direcciones (IAL) siempre está activa en AlpIRC",
            "* Die interne Adressliste (IAL) ist in AlpIRC immer aktiv",
            "* La lista interna degli indirizzi (IAL) è sempre attiva in AlpIRC",
            "* La llista interna d'adreces (IAL) sempre està activa en AlpIRC",
            "* Bíonn an liosta inmheánach seoltaí (IAL) ar siúl i gcónaí in AlpIRC",
        ],
    ),
    (
        Key::HelpHead,
        [
            "* Comandos de AlpIRC (escribe /help comando para más detalles):",
            "* AlpIRC-Befehle (/help Befehl für Details):",
            "* Comandi di AlpIRC (scrivi /help comando per i dettagli):",
            "* Ordres d'AlpIRC (escriviu /help ordre per a més detalls):",
            "* Orduithe AlpIRC (clóscríobh /help ordú le haghaidh sonraí):",
        ],
    ),
    (
        Key::HelpNone,
        [
            "* No hay ayuda para /{cmd}",
            "* Keine Hilfe zu /{cmd}",
            "* Nessuna guida per /{cmd}",
            "* No hi ha ajuda per a /{cmd}",
            "* Níl cabhair ann do /{cmd}",
        ],
    ),
    (
        Key::HelpMore,
        [
            "* Guía completa: {url}",
            "* Vollständige Anleitung: {url}",
            "* Guida completa: {url}",
            "* Guia completa: {url}",
            "* Treoir iomlán: {url}",
        ],
    ),
    (Key::TitleNotify, ["Lista Notify", "Notify-Liste", "Lista Notify", "Llista Notify", "Liosta Notify"]),
    (
        Key::TitleStatus,
        [
            "Status: {nick} [{modes}] en {network} ({server}:{port})",
            "Status: {nick} [{modes}] auf {network} ({server}:{port})",
            "Status: {nick} [{modes}] su {network} ({server}:{port})",
            "Status: {nick} [{modes}] en {network} ({server}:{port})",
            "Status: {nick} [{modes}] ar {network} ({server}:{port})",
        ],
    ),
    (
        Key::TitleStatusOffline,
        [
            "Status: {nick} (sin conexión)",
            "Status: {nick} (nicht verbunden)",
            "Status: {nick} (non connesso)",
            "Status: {nick} (no connectat)",
            "Status: {nick} (gan cheangal)",
        ],
    ),
    (
        Key::TitleChanList,
        ["Lista de canales", "Channelliste", "Lista dei canali", "Llista de canals", "Liosta Cainéal"],
    ),
    (
        Key::NotifHighlight,
        ["{nick} en {chan}", "{nick} in {chan}", "{nick} su {chan}", "{nick} en {chan}", "{nick} ar {chan}"],
    ),
    (
        Key::NotifQuery,
        [
            "Mensaje privado de {nick}",
            "Private Nachricht von {nick}",
            "Messaggio privato da {nick}",
            "Missatge privat de {nick}",
            "Teachtaireacht phríobháideach ó {nick}",
        ],
    ),
];

/// Texte brut d'une clé dans une langue (repli sur l'anglais).
pub fn tr(lang: Lang, key: Key) -> &'static str {
    match lang {
        Lang::En => key.en(),
        Lang::Fr => key.fr(),
        l => other(l, key).unwrap_or_else(|| key.en()),
    }
}

/// Texte d'une clé avec ses marques remplacées (`{nick}` → valeur).
pub fn trf(lang: Lang, key: Key, args: &[(&str, &str)]) -> String {
    fill(tr(lang, key), args)
}

/// Remplace les marques `{nom}` d'un modèle ; les marques inconnues restent telles quelles.
pub fn fill(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        match after.find('}') {
            Some(j) => {
                let name = &after[..j];
                match args.iter().find(|(k, _)| *k == name) {
                    Some((_, v)) => out.push_str(v),
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
                rest = &after[j + 1..];
            }
            None => {
                out.push_str(&rest[i..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// Part des clés traduites pour chaque langue : (langue, traduites, total).
pub fn coverage() -> Vec<(Lang, usize, usize)> {
    let total = Key::ALL.len();
    Lang::ALL
        .iter()
        .map(|&l| {
            let n = match l {
                Lang::En | Lang::Fr => total,
                l => Key::ALL.iter().filter(|k| other(l, **k).is_some()).count(),
            };
            (l, n, total)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_placeholders() {
        assert_eq!(trf(Lang::En, Key::Joins, &[("nick", "Pat"), ("address", "p@h")]), "* Joins: Pat (p@h)");
        assert_eq!(trf(Lang::Fr, Key::Joins, &[("nick", "Pat"), ("address", "p@h")]), "* Arrivée : Pat (p@h)");
        assert_eq!(fill("{a} {b} {", &[("a", "1")]), "1 {b} {");
        assert_eq!(tr(Lang::Ga, Key::Connected), "* Ceangailte");
        assert_eq!(trf(Lang::Va, Key::Parts, &[("nick", "Pat"), ("address", "p@h")]), "* Ix: Pat (p@h)");
    }

    #[test]
    fn locales() {
        assert_eq!(Lang::from_locale("fr_FR.UTF-8"), Some(Lang::Fr));
        assert_eq!(Lang::from_locale("ca-ES-valencia"), Some(Lang::Va));
        assert_eq!(Lang::from_locale("ga"), Some(Lang::Ga));
        assert_eq!(Lang::from_locale("ja_JP"), None);
    }

    #[test]
    fn same_placeholders_in_fr() {
        // Chaque traduction française garde les mêmes marques que l'anglais.
        let marks = |s: &str| {
            let mut v: Vec<String> = s.split('{').skip(1).filter_map(|p| p.split_once('}').map(|(a, _)| a.to_string())).collect();
            v.sort();
            v
        };
        for k in Key::ALL {
            assert_eq!(marks(tr(Lang::En, *k)), marks(tr(Lang::Fr, *k)), "{k:?}");
        }
    }

    #[test]
    fn same_placeholders_in_every_language() {
        // Chaque traduction remplie garde les marques de l'anglais (une langue vide retombe sur l'anglais).
        let marks = |s: &str| {
            let mut v: Vec<String> =
                s.split('{').skip(1).filter_map(|p| p.split_once('}').map(|(a, _)| a.to_string())).collect();
            v.sort();
            v
        };
        for k in Key::ALL {
            for l in Lang::ALL {
                assert_eq!(marks(tr(Lang::En, *k)), marks(tr(l, *k)), "{k:?} ({})", l.code());
            }
        }
    }

    /// Clés qui restent identiques à l'anglais dans toutes les langues (rien que des marques et des signes).
    const SAME_AS_ENGLISH: &[Key] = &[Key::MsgSent, Key::NoticeSent, Key::CtcpSent];

    // Toute nouvelle clé doit être traduite dans les 7 langues (voir l'ADR 0012).
    #[test]
    fn every_key_in_seven_languages() {
        let others = [Lang::Es, Lang::De, Lang::It, Lang::Va, Lang::Ga];
        for k in Key::ALL {
            for l in others {
                assert!(other(l, *k).is_some(), "{k:?} : pas de traduction {}", l.code());
            }
            // Les cinq langues identiques à l'anglais : sans doute un copier-coller.
            let copied = others.iter().all(|l| tr(*l, *k) == tr(Lang::En, *k));
            assert!(SAME_AS_ENGLISH.contains(k) || !copied, "{k:?} : identique à l'anglais partout");
        }
    }

    #[test]
    fn coverage_report() {
        // Rapport, pas un échec : les langues non encore traduites retombent sur l'anglais.
        for (l, n, total) in coverage() {
            println!("{}: {n}/{total}", l.code());
        }
        assert_eq!(coverage()[1], (Lang::Fr, Key::ALL.len(), Key::ALL.len()));
    }
}
