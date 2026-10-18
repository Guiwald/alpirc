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
    // --- échecs du réseau (alpirc-net), voir net_text.rs ---
    NetConnectTimeout => "connection timed out ({secs} s)", "délai de connexion dépassé ({secs} s)";
    NetTlsTimeout => "TLS handshake timed out ({secs} s)", "délai dépassé pendant la négociation TLS ({secs} s)";
    NetDnsNotFound => "address of '{host}' not found", "adresse de « {host} » introuvable";
    NetDnsTemporary => "temporary failure resolving '{host}'", "échec passager de la résolution de « {host} »";
    NetNoAddress => "no address for '{host}'", "aucune adresse pour « {host} »";
    NetNoAddressForBind => "no address of '{host}' in the same family as the local address {bind}",
        "aucune adresse de « {host} » de la même famille que l'adresse locale {bind}";
    NetUnreachable => "{target}: {cause}", "{target} : {cause}";
    NetUnreachableVia => "{target} ({addr}): {cause}", "{target} ({addr}) : {cause}";
    NetBadTlsName => "invalid host name for TLS: '{host}'", "nom d'hôte invalide pour TLS : « {host} »";
    NetCertRejected => "certificate rejected: {problem}", "certificat refusé : {problem}";
    NetTlsAlert => "the server aborted the TLS handshake (alert {alert})",
        "le serveur a interrompu la négociation TLS (alerte {alert})";
    NetNoServerCert => "the server presented no certificate", "le serveur n'a présenté aucun certificat";
    NetTls => "TLS handshake: {detail}", "négociation TLS : {detail}";
    NetBadPin => "invalid SHA-256 fingerprint '{pin}' (64 hexadecimal digits expected)",
        "empreinte SHA-256 invalide « {pin} » (64 chiffres hexadécimaux attendus)";
    NetClientCert => "client certificate: {problem}", "certificat client : {problem}";
    NetPemBadCert => "unreadable PEM certificate ({detail})", "certificat PEM illisible ({detail})";
    NetPemNoCert => "no certificate in the PEM file", "aucun certificat dans le fichier PEM";
    NetPemBadKey => "unreadable PEM private key ({detail})", "clé privée PEM illisible ({detail})";
    NetTlsSetup => "TLS setup: {detail}", "configuration TLS : {detail}";
    NetStopped => "network layer stopped", "couche réseau arrêtée";
    NetDccListen => "cannot listen for DCC on {ip}: {cause}", "écoute DCC impossible sur {ip} : {cause}";
    NetTimeout => "timed out ({secs} s)", "délai dépassé ({secs} s)";
    NetCancelled => "cancelled", "annulé";
    NetPeerClosed => "connection closed by the other side at {pos} of {size} bytes",
        "connexion fermée par l'autre à {pos} octets sur {size}";
    NetFileRead => "reading {path}: {cause}", "lecture de {path} : {cause}";
    NetFileShrunk => "{path} got shorter during the transfer", "{path} a raccourci pendant l'envoi";
    NetFileWrite => "writing {path}: {cause}", "écriture de {path} : {cause}";
    IoRefused => "connection refused", "connexion refusée";
    IoReset => "connection reset by peer", "connexion réinitialisée par le pair";
    IoAborted => "connection aborted", "connexion interrompue";
    IoBrokenPipe => "connection broken", "connexion rompue";
    IoTimedOut => "timed out", "délai dépassé";
    IoHostUnreachable => "host unreachable", "hôte injoignable";
    IoNetworkUnreachable => "network unreachable", "réseau injoignable";
    IoAddrNotAvailable => "local address not available", "adresse locale indisponible";
    IoAddrInUse => "address already in use", "adresse déjà utilisée";
    IoNotFound => "no such file or folder", "fichier ou dossier introuvable";
    IoPermissionDenied => "permission denied", "permission refusée";
    IoStorageFull => "disk full", "disque plein";
    CertExpired => "certificate expired", "certificat expiré";
    CertExpiredOn => "certificate expired (valid until {date})", "certificat expiré (valable jusqu'au {date})";
    CertNotYetValid => "certificate not valid yet", "certificat pas encore valable";
    CertNotYetValidFrom => "certificate not valid yet (valid from {date})",
        "certificat pas encore valable (valable à partir du {date})";
    CertUnknownIssuer => "unknown issuer (self-signed certificate or unrecognised authority)",
        "émetteur inconnu (certificat autosigné ou autorité non reconnue)";
    CertNameMismatch => "the certificate does not match the name '{host}'",
        "le certificat ne correspond pas au nom « {host} »";
    CertNameMismatchNames => "the certificate does not match the name '{host}' (certificate names: {names})",
        "le certificat ne correspond pas au nom « {host} » (noms du certificat : {names})";
    CertBadSignature => "invalid certificate signature", "signature du certificat invalide";
    CertRevoked => "certificate revoked", "certificat révoqué";
    CertBadEncoding => "malformed certificate", "certificat mal formé";
    CertWrongPurpose => "certificate not intended for a server", "certificat non prévu pour un serveur";
    CertUnsupportedAlgorithm => "unsupported certificate signature algorithm",
        "algorithme de signature du certificat non pris en charge";
    CertPinMismatch => "the certificate fingerprint does not match the stored fingerprint",
        "l'empreinte du certificat ne correspond pas à l'empreinte retenue";
    PingTimeoutReason => "Ping timeout", "Délai de ping dépassé";
    // --- messages du moteur de scripts (alpirc-script), voir script_text.rs ; sans l'astérisque du début ---
    ScrInvalidParams => "/{cmd}: invalid parameters", "/{cmd} : paramètres invalides";
    ScrCannotOpen => "/{cmd}: unable to open '{file}'", "/{cmd} : impossible d'ouvrir « {file} »";
    ScrCannotWrite => "/{cmd}: unable to write '{file}'", "/{cmd} : impossible d'écrire « {file} »";
    ScrCannotRemove => "/{cmd}: unable to remove '{file}'", "/{cmd} : impossible de supprimer « {file} »";
    ScrCannotRename => "/{cmd}: unable to rename '{file}'", "/{cmd} : impossible de renommer « {file} »";
    ScrCannotCopy => "/{cmd}: unable to copy '{file}'", "/{cmd} : impossible de copier « {file} »";
    ScrCannotCreate => "/{cmd}: unable to create '{file}'", "/{cmd} : impossible de créer « {file} »";
    ScrFileExists => "/{cmd}: '{file}' exists", "/{cmd} : « {file} » existe déjà";
    ScrNoSuchDialog => "/{cmd}: no such dialog '{name}'", "/{cmd} : pas de dialogue « {name} »";
    ScrNoSuchTable => "/{cmd}: no such table '{name}'", "/{cmd} : pas de table « {name} »";
    ScrNoSuchScript => "/{cmd}: no such script '{name}'", "/{cmd} : pas de script « {name} »";
    ScrTableExists => "/{cmd}: table '{name}' exists", "/{cmd} : la table « {name} » existe déjà";
    ScrNameInUse => "/{cmd}: '{name}' name in use", "/{cmd} : le nom « {name} » est déjà pris";
    ScrNoAddress => "/{cmd}: unable to find address of {nick}", "/{cmd} : adresse de {nick} introuvable";
    ScrLabelNotFound => "/goto: '{label}' not found", "/goto : « {label} » introuvable";
    ScrCommandSandboxed => "/{cmd}: not allowed by the script sandbox",
        "/{cmd} : interdit par le bac à sable des scripts";
    ScrIdentSandboxed => "${ident}: not allowed by the script sandbox",
        "${ident} : interdit par le bac à sable des scripts";
    ScrIdentParams => "Invalid parameters: ${ident}", "Paramètres invalides : ${ident}";
    ScrIdentParamsDetail => "Invalid parameters: ${ident} ({detail})", "Paramètres invalides : ${ident} ({detail})";
    ScrIdentOnlyMime => "Invalid parameters: ${ident} (only 'm' is supported)",
        "Paramètres invalides : ${ident} (seule l'option « m » est prise en charge)";
    ScrTooDeep => "Error: script too deep / infinite loop", "Erreur : script trop profond / boucle infinie";
    ScrNoSuchMenuItem => "popup: no such menu item", "popup : élément de menu inconnu";
    ScrSandbox => "access denied outside the scripts folder: {path}",
        "accès refusé hors du dossier des scripts : {path}";
    ScrParse => "{file}:{line}: {problem}", "{file}:{line} : {problem}";
    ScrMissingClose => "missing '}'", "« } » manquant";
    ScrMissingOpen => "missing '{'", "« { » manquant";
    ScrAt => "{msg} (line {line}, {script})", "{msg} (ligne {line}, {script})";
    ScrMadeTable => "Made hash table '{name}' ({size})", "Table de hachage « {name} » créée ({size})";
    ScrAddedItem => "Added item '{item}' to hash table '{table}'",
        "Élément « {item} » ajouté à la table de hachage « {table} »";
    ScrTimerNotActive => "Timer {name} not active", "Timer {name} inactif";
    ScrTimerHalted => "Timer {name} halted", "Timer {name} arrêté";
    ScrTimerActivated => "Timer {name} activated", "Timer {name} activé";
    ScrAllTimersHalted => "All timers halted", "Tous les timers sont arrêtés";
    ScrNoActiveTimers => "No active timers", "Aucun timer actif";
    ScrActiveTimers => "Active timers:", "Timers actifs :";
    ScrTimerLine => "Timer {name} {delay} delay {command}", "Timer {name}, délai {delay} : {command}";
    ScrTimerLineLeft => "Timer {name} {delay} delay {command} ({left} left)",
        "Timer {name}, délai {delay} : {command} (encore {left})";
    ScrVarSet => "Set {name} to {value}", "{name} vaut maintenant {value}";
    ScrVarUnsetAll => "Unset all variables", "Toutes les variables sont effacées";
    ScrVarUnset => "Unset {name}", "{name} effacée";
    ScrVarInc => "Inc {name} to {value}", "{name} augmentée à {value}";
    ScrVarDec => "Dec {name} to {value}", "{name} diminuée à {value}";
    // --- textes par défaut d'une nouvelle installation (réglages), voir Client::localize_new_install ---
    DefaultRealname => "AlpIRC user", "Utilisateur d'AlpIRC";
    DefaultAway => "Away", "Absent";
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
    (
        Key::NetConnectTimeout,
        [
            "tiempo de conexión agotado ({secs} s)",
            "Zeitüberschreitung beim Verbindungsaufbau ({secs} s)",
            "tempo di connessione scaduto ({secs} s)",
            "temps de connexió esgotat ({secs} s)",
            "teorainn ama an cheangail sáraithe ({secs} s)",
        ],
    ),
    (
        Key::NetTlsTimeout,
        [
            "tiempo agotado en la negociación TLS ({secs} s)",
            "Zeitüberschreitung beim TLS-Handshake ({secs} s)",
            "tempo scaduto durante la negoziazione TLS ({secs} s)",
            "temps esgotat en la negociació TLS ({secs} s)",
            "teorainn ama sáraithe le linn chaibidlíocht TLS ({secs} s)",
        ],
    ),
    (
        Key::NetDnsNotFound,
        [
            "no se encuentra la dirección de «{host}»",
            "Adresse von „{host}“ nicht gefunden",
            "indirizzo di «{host}» non trovato",
            "no es troba l'adreça de «{host}»",
            "níor aimsíodh seoladh '{host}'",
        ],
    ),
    (
        Key::NetDnsTemporary,
        [
            "fallo temporal al resolver «{host}»",
            "vorübergehender Fehler bei der Namensauflösung von „{host}“",
            "errore temporaneo nella risoluzione di «{host}»",
            "error temporal en resoldre «{host}»",
            "teip shealadach ar '{host}' a réiteach",
        ],
    ),
    (
        Key::NetNoAddress,
        [
            "ninguna dirección para «{host}»",
            "keine Adresse für „{host}“",
            "nessun indirizzo per «{host}»",
            "cap adreça per a «{host}»",
            "níl aon seoladh ag '{host}'",
        ],
    ),
    (
        Key::NetNoAddressForBind,
        [
            "ninguna dirección de «{host}» de la misma familia que la dirección local {bind}",
            "keine Adresse von „{host}“ in derselben Familie wie die lokale Adresse {bind}",
            "nessun indirizzo di «{host}» della stessa famiglia dell'indirizzo locale {bind}",
            "cap adreça de «{host}» de la mateixa família que l'adreça local {bind}",
            "níl aon seoladh ag '{host}' den teaghlach céanna leis an seoladh áitiúil {bind}",
        ],
    ),
    (
        Key::NetUnreachable,
        [
            "{target}: {cause}",
            "{target}: {cause}",
            "{target}: {cause}",
            "{target}: {cause}",
            "{target}: {cause}",
        ],
    ),
    (
        Key::NetUnreachableVia,
        [
            "{target} ({addr}): {cause}",
            "{target} ({addr}): {cause}",
            "{target} ({addr}): {cause}",
            "{target} ({addr}): {cause}",
            "{target} ({addr}): {cause}",
        ],
    ),
    (
        Key::NetBadTlsName,
        [
            "nombre de host no válido para TLS: «{host}»",
            "ungültiger Hostname für TLS: „{host}“",
            "nome host non valido per TLS: «{host}»",
            "nom d'amfitrió no vàlid per a TLS: «{host}»",
            "óstainm neamhbhailí do TLS: '{host}'",
        ],
    ),
    (
        Key::NetCertRejected,
        [
            "certificado rechazado: {problem}",
            "Zertifikat abgelehnt: {problem}",
            "certificato rifiutato: {problem}",
            "certificat rebutjat: {problem}",
            "diúltaíodh don teastas: {problem}",
        ],
    ),
    (
        Key::NetTlsAlert,
        [
            "el servidor interrumpió la negociación TLS (alerta {alert})",
            "der Server hat den TLS-Handshake abgebrochen (Alert {alert})",
            "il server ha interrotto la negoziazione TLS (avviso {alert})",
            "el servidor ha interromput la negociació TLS (alerta {alert})",
            "chuir an freastalaí deireadh le caibidlíocht TLS (foláireamh {alert})",
        ],
    ),
    (
        Key::NetNoServerCert,
        [
            "el servidor no presentó ningún certificado",
            "der Server hat kein Zertifikat vorgelegt",
            "il server non ha presentato alcun certificato",
            "el servidor no ha presentat cap certificat",
            "níor thaispeáin an freastalaí aon teastas",
        ],
    ),
    (
        Key::NetTls,
        [
            "negociación TLS: {detail}",
            "TLS-Handshake: {detail}",
            "negoziazione TLS: {detail}",
            "negociació TLS: {detail}",
            "caibidlíocht TLS: {detail}",
        ],
    ),
    (
        Key::NetBadPin,
        [
            "huella SHA-256 no válida «{pin}» (se esperan 64 dígitos hexadecimales)",
            "ungültiger SHA-256-Fingerabdruck „{pin}“ (64 Hexadezimalziffern erwartet)",
            "impronta SHA-256 non valida «{pin}» (attese 64 cifre esadecimali)",
            "empremta SHA-256 no vàlida «{pin}» (s'esperen 64 dígits hexadecimals)",
            "méarlorg SHA-256 neamhbhailí '{pin}' (64 digit heicsidheachúlach ag teastáil)",
        ],
    ),
    (
        Key::NetClientCert,
        [
            "certificado de cliente: {problem}",
            "Client-Zertifikat: {problem}",
            "certificato client: {problem}",
            "certificat de client: {problem}",
            "teastas cliaint: {problem}",
        ],
    ),
    (
        Key::NetPemBadCert,
        [
            "certificado PEM ilegible ({detail})",
            "PEM-Zertifikat nicht lesbar ({detail})",
            "certificato PEM illeggibile ({detail})",
            "certificat PEM il·legible ({detail})",
            "teastas PEM nach féidir a léamh ({detail})",
        ],
    ),
    (
        Key::NetPemNoCert,
        [
            "ningún certificado en el archivo PEM",
            "kein Zertifikat in der PEM-Datei",
            "nessun certificato nel file PEM",
            "cap certificat en el fitxer PEM",
            "níl aon teastas sa chomhad PEM",
        ],
    ),
    (
        Key::NetPemBadKey,
        [
            "clave privada PEM ilegible ({detail})",
            "privater PEM-Schlüssel nicht lesbar ({detail})",
            "chiave privata PEM illeggibile ({detail})",
            "clau privada PEM il·legible ({detail})",
            "eochair phríobháideach PEM nach féidir a léamh ({detail})",
        ],
    ),
    (
        Key::NetTlsSetup,
        [
            "configuración TLS: {detail}",
            "TLS-Konfiguration: {detail}",
            "configurazione TLS: {detail}",
            "configuració TLS: {detail}",
            "socrú TLS: {detail}",
        ],
    ),
    (
        Key::NetStopped,
        [
            "capa de red detenida",
            "Netzwerkschicht angehalten",
            "livello di rete arrestato",
            "capa de xarxa aturada",
            "stopadh an ciseal líonra",
        ],
    ),
    (
        Key::NetDccListen,
        [
            "no se puede escuchar DCC en {ip}: {cause}",
            "DCC-Empfang auf {ip} nicht möglich: {cause}",
            "impossibile restare in ascolto DCC su {ip}: {cause}",
            "no es pot escoltar DCC en {ip}: {cause}",
            "ní féidir éisteacht le haghaidh DCC ar {ip}: {cause}",
        ],
    ),
    (
        Key::NetTimeout,
        [
            "tiempo agotado ({secs} s)",
            "Zeitüberschreitung ({secs} s)",
            "tempo scaduto ({secs} s)",
            "temps esgotat ({secs} s)",
            "thar am ({secs} s)",
        ],
    ),
    (Key::NetCancelled, ["cancelado", "abgebrochen", "annullato", "cancel·lat", "cealaithe"]),
    (
        Key::NetPeerClosed,
        [
            "conexión cerrada por el otro extremo en {pos} de {size} bytes",
            "Verbindung von der Gegenseite bei {pos} von {size} Bytes geschlossen",
            "connessione chiusa dall'altro capo a {pos} byte su {size}",
            "connexió tancada per l'altre extrem a {pos} de {size} bytes",
            "dhún an taobh eile an ceangal ag {pos} as {size} beart",
        ],
    ),
    (
        Key::NetFileRead,
        [
            "lectura de {path}: {cause}",
            "Lesen von {path}: {cause}",
            "lettura di {path}: {cause}",
            "lectura de {path}: {cause}",
            "{path} á léamh: {cause}",
        ],
    ),
    (
        Key::NetFileShrunk,
        [
            "{path} se ha acortado durante la transferencia",
            "{path} wurde während der Übertragung kürzer",
            "{path} si è accorciato durante il trasferimento",
            "{path} s'ha escurçat durant la transferència",
            "d'éirigh {path} níos giorra le linn an aistrithe",
        ],
    ),
    (
        Key::NetFileWrite,
        [
            "escritura de {path}: {cause}",
            "Schreiben von {path}: {cause}",
            "scrittura di {path}: {cause}",
            "escriptura de {path}: {cause}",
            "{path} á scríobh: {cause}",
        ],
    ),
    (
        Key::IoRefused,
        [
            "conexión rechazada",
            "Verbindung abgelehnt",
            "connessione rifiutata",
            "connexió rebutjada",
            "diúltaíodh don cheangal",
        ],
    ),
    (
        Key::IoReset,
        [
            "conexión restablecida por el otro extremo",
            "Verbindung von der Gegenseite zurückgesetzt",
            "connessione reimpostata dall'altro capo",
            "connexió restablida per l'altre extrem",
            "d'athshocraigh an taobh eile an ceangal",
        ],
    ),
    (
        Key::IoAborted,
        [
            "conexión interrumpida",
            "Verbindung abgebrochen",
            "connessione interrotta",
            "connexió interrompuda",
            "tobscoireadh an ceangal",
        ],
    ),
    (
        Key::IoBrokenPipe,
        [
            "conexión rota",
            "Verbindung unterbrochen",
            "connessione caduta",
            "connexió trencada",
            "briseadh an ceangal",
        ],
    ),
    (Key::IoTimedOut, ["tiempo agotado", "Zeitüberschreitung", "tempo scaduto", "temps esgotat", "thar am"]),
    (
        Key::IoHostUnreachable,
        [
            "host inaccesible",
            "Host nicht erreichbar",
            "host irraggiungibile",
            "amfitrió inaccessible",
            "níl an t-óstríomhaire inrochtana",
        ],
    ),
    (
        Key::IoNetworkUnreachable,
        [
            "red inaccesible",
            "Netzwerk nicht erreichbar",
            "rete irraggiungibile",
            "xarxa inaccessible",
            "níl an líonra inrochtana",
        ],
    ),
    (
        Key::IoAddrNotAvailable,
        [
            "dirección local no disponible",
            "lokale Adresse nicht verfügbar",
            "indirizzo locale non disponibile",
            "adreça local no disponible",
            "níl an seoladh áitiúil ar fáil",
        ],
    ),
    (
        Key::IoAddrInUse,
        [
            "dirección ya en uso",
            "Adresse bereits in Verwendung",
            "indirizzo già in uso",
            "adreça ja en ús",
            "tá an seoladh in úsáid cheana",
        ],
    ),
    (
        Key::IoNotFound,
        [
            "no existe el archivo o la carpeta",
            "Datei oder Ordner nicht vorhanden",
            "file o cartella inesistente",
            "no existix el fitxer o la carpeta",
            "níl a leithéid de chomhad ná d'fhillteán ann",
        ],
    ),
    (
        Key::IoPermissionDenied,
        [
            "permiso denegado",
            "Zugriff verweigert",
            "permesso negato",
            "permís denegat",
            "cead diúltaithe",
        ],
    ),
    (Key::IoStorageFull, ["disco lleno", "Datenträger voll", "disco pieno", "disc ple", "tá an diosca lán"]),
    (
        Key::CertExpired,
        [
            "certificado caducado",
            "Zertifikat abgelaufen",
            "certificato scaduto",
            "certificat caducat",
            "teastas as feidhm",
        ],
    ),
    (
        Key::CertExpiredOn,
        [
            "certificado caducado (válido hasta el {date})",
            "Zertifikat abgelaufen (gültig bis {date})",
            "certificato scaduto (valido fino al {date})",
            "certificat caducat (vàlid fins al {date})",
            "teastas as feidhm (bailí go dtí {date})",
        ],
    ),
    (
        Key::CertNotYetValid,
        [
            "certificado aún no válido",
            "Zertifikat noch nicht gültig",
            "certificato non ancora valido",
            "certificat encara no vàlid",
            "níl an teastas bailí fós",
        ],
    ),
    (
        Key::CertNotYetValidFrom,
        [
            "certificado aún no válido (válido a partir del {date})",
            "Zertifikat noch nicht gültig (gültig ab {date})",
            "certificato non ancora valido (valido dal {date})",
            "certificat encara no vàlid (vàlid a partir del {date})",
            "níl an teastas bailí fós (bailí ó {date})",
        ],
    ),
    (
        Key::CertUnknownIssuer,
        [
            "emisor desconocido (certificado autofirmado o autoridad no reconocida)",
            "unbekannter Aussteller (selbstsigniertes Zertifikat oder nicht anerkannte Zertifizierungsstelle)",
            "emittente sconosciuto (certificato autofirmato o autorità non riconosciuta)",
            "emissor desconegut (certificat autosignat o autoritat no reconeguda)",
            "eisitheoir anaithnid (teastas féinsínithe nó údarás nach n-aithnítear)",
        ],
    ),
    (
        Key::CertNameMismatch,
        [
            "el certificado no corresponde al nombre «{host}»",
            "das Zertifikat passt nicht zum Namen „{host}“",
            "il certificato non corrisponde al nome «{host}»",
            "el certificat no correspon al nom «{host}»",
            "níl an teastas ag teacht leis an ainm '{host}'",
        ],
    ),
    (
        Key::CertNameMismatchNames,
        [
            "el certificado no corresponde al nombre «{host}» (nombres del certificado: {names})",
            "das Zertifikat passt nicht zum Namen „{host}“ (Namen im Zertifikat: {names})",
            "il certificato non corrisponde al nome «{host}» (nomi del certificato: {names})",
            "el certificat no correspon al nom «{host}» (noms del certificat: {names})",
            "níl an teastas ag teacht leis an ainm '{host}' (ainmneacha an teastais: {names})",
        ],
    ),
    (
        Key::CertBadSignature,
        [
            "firma del certificado no válida",
            "ungültige Zertifikatssignatur",
            "firma del certificato non valida",
            "signatura del certificat no vàlida",
            "síniú neamhbhailí ar an teastas",
        ],
    ),
    (
        Key::CertRevoked,
        [
            "certificado revocado",
            "Zertifikat widerrufen",
            "certificato revocato",
            "certificat revocat",
            "teastas cúlghairthe",
        ],
    ),
    (
        Key::CertBadEncoding,
        [
            "certificado mal formado",
            "fehlerhaft codiertes Zertifikat",
            "certificato malformato",
            "certificat mal format",
            "teastas míchumtha",
        ],
    ),
    (
        Key::CertWrongPurpose,
        [
            "certificado no destinado a un servidor",
            "Zertifikat nicht für einen Server bestimmt",
            "certificato non destinato a un server",
            "certificat no destinat a un servidor",
            "níl an teastas ceaptha d'fhreastalaí",
        ],
    ),
    (
        Key::CertUnsupportedAlgorithm,
        [
            "algoritmo de firma del certificado no admitido",
            "Signaturalgorithmus des Zertifikats nicht unterstützt",
            "algoritmo di firma del certificato non supportato",
            "algorisme de signatura del certificat no admés",
            "algartam sínithe an teastais gan tacaíocht",
        ],
    ),
    (
        Key::CertPinMismatch,
        [
            "la huella del certificado no coincide con la huella guardada",
            "der Fingerabdruck des Zertifikats stimmt nicht mit dem gespeicherten überein",
            "l'impronta del certificato non corrisponde a quella memorizzata",
            "l'empremta del certificat no coincidix amb l'empremta guardada",
            "níl méarlorg an teastais mar an gcéanna leis an méarlorg a stóráladh",
        ],
    ),
    (
        Key::PingTimeoutReason,
        [
            "Tiempo de ping agotado",
            "Ping-Timeout",
            "Timeout del ping",
            "Temps d'espera del ping esgotat",
            "Teorainn ama ping sáraithe",
        ],
    ),
    (
        Key::ScrInvalidParams,
        [
            "/{cmd}: parámetros no válidos",
            "/{cmd}: ungültige Parameter",
            "/{cmd}: parametri non validi",
            "/{cmd}: paràmetres no vàlids",
            "/{cmd}: paraiméadair neamhbhailí",
        ],
    ),
    (
        Key::ScrCannotOpen,
        [
            "/{cmd}: no se puede abrir «{file}»",
            "/{cmd}: „{file}“ kann nicht geöffnet werden",
            "/{cmd}: impossibile aprire «{file}»",
            "/{cmd}: no es pot obrir «{file}»",
            "/{cmd}: ní féidir '{file}' a oscailt",
        ],
    ),
    (
        Key::ScrCannotWrite,
        [
            "/{cmd}: no se puede escribir «{file}»",
            "/{cmd}: „{file}“ kann nicht geschrieben werden",
            "/{cmd}: impossibile scrivere «{file}»",
            "/{cmd}: no es pot escriure «{file}»",
            "/{cmd}: ní féidir '{file}' a scríobh",
        ],
    ),
    (
        Key::ScrCannotRemove,
        [
            "/{cmd}: no se puede eliminar «{file}»",
            "/{cmd}: „{file}“ kann nicht gelöscht werden",
            "/{cmd}: impossibile eliminare «{file}»",
            "/{cmd}: no es pot eliminar «{file}»",
            "/{cmd}: ní féidir '{file}' a bhaint",
        ],
    ),
    (
        Key::ScrCannotRename,
        [
            "/{cmd}: no se puede cambiar el nombre de «{file}»",
            "/{cmd}: „{file}“ kann nicht umbenannt werden",
            "/{cmd}: impossibile rinominare «{file}»",
            "/{cmd}: no es pot canviar el nom de «{file}»",
            "/{cmd}: ní féidir '{file}' a athainmniú",
        ],
    ),
    (
        Key::ScrCannotCopy,
        [
            "/{cmd}: no se puede copiar «{file}»",
            "/{cmd}: „{file}“ kann nicht kopiert werden",
            "/{cmd}: impossibile copiare «{file}»",
            "/{cmd}: no es pot copiar «{file}»",
            "/{cmd}: ní féidir '{file}' a chóipeáil",
        ],
    ),
    (
        Key::ScrCannotCreate,
        [
            "/{cmd}: no se puede crear «{file}»",
            "/{cmd}: „{file}“ kann nicht erstellt werden",
            "/{cmd}: impossibile creare «{file}»",
            "/{cmd}: no es pot crear «{file}»",
            "/{cmd}: ní féidir '{file}' a chruthú",
        ],
    ),
    (
        Key::ScrFileExists,
        [
            "/{cmd}: «{file}» ya existe",
            "/{cmd}: „{file}“ existiert bereits",
            "/{cmd}: «{file}» esiste già",
            "/{cmd}: «{file}» ja existix",
            "/{cmd}: tá '{file}' ann cheana",
        ],
    ),
    (
        Key::ScrNoSuchDialog,
        [
            "/{cmd}: no existe el diálogo «{name}»",
            "/{cmd}: kein Dialog „{name}“",
            "/{cmd}: nessuna finestra di dialogo «{name}»",
            "/{cmd}: no hi ha cap diàleg «{name}»",
            "/{cmd}: níl dialóg ann darb ainm '{name}'",
        ],
    ),
    (
        Key::ScrNoSuchTable,
        [
            "/{cmd}: no existe la tabla «{name}»",
            "/{cmd}: keine Tabelle „{name}“",
            "/{cmd}: nessuna tabella «{name}»",
            "/{cmd}: no hi ha cap taula «{name}»",
            "/{cmd}: níl tábla ann darb ainm '{name}'",
        ],
    ),
    (
        Key::ScrNoSuchScript,
        [
            "/{cmd}: no existe el script «{name}»",
            "/{cmd}: kein Script „{name}“",
            "/{cmd}: nessuno script «{name}»",
            "/{cmd}: no hi ha cap script «{name}»",
            "/{cmd}: níl script ann darb ainm '{name}'",
        ],
    ),
    (
        Key::ScrTableExists,
        [
            "/{cmd}: la tabla «{name}» ya existe",
            "/{cmd}: Tabelle „{name}“ existiert bereits",
            "/{cmd}: la tabella «{name}» esiste già",
            "/{cmd}: la taula «{name}» ja existix",
            "/{cmd}: tá an tábla '{name}' ann cheana",
        ],
    ),
    (
        Key::ScrNameInUse,
        [
            "/{cmd}: el nombre «{name}» ya está en uso",
            "/{cmd}: Name „{name}“ wird bereits verwendet",
            "/{cmd}: il nome «{name}» è già in uso",
            "/{cmd}: el nom «{name}» ja està en ús",
            "/{cmd}: tá an t-ainm '{name}' in úsáid cheana",
        ],
    ),
    (
        Key::ScrNoAddress,
        [
            "/{cmd}: no se encuentra la dirección de {nick}",
            "/{cmd}: Adresse von {nick} nicht gefunden",
            "/{cmd}: impossibile trovare l'indirizzo di {nick}",
            "/{cmd}: no es troba l'adreça de {nick}",
            "/{cmd}: ní féidir seoladh {nick} a aimsiú",
        ],
    ),
    (
        Key::ScrLabelNotFound,
        [
            "/goto: no se encuentra «{label}»",
            "/goto: „{label}“ nicht gefunden",
            "/goto: etichetta «{label}» non trovata",
            "/goto: no es troba «{label}»",
            "/goto: níor aimsíodh '{label}'",
        ],
    ),
    (
        Key::ScrCommandSandboxed,
        [
            "/{cmd}: no permitido por el entorno aislado de los scripts",
            "/{cmd}: von der Script-Sandbox nicht erlaubt",
            "/{cmd}: non consentito dalla sandbox degli script",
            "/{cmd}: no es permet en l'entorn aïllat dels scripts",
            "/{cmd}: ní cheadaíonn bosca gainimh na scripteanna é",
        ],
    ),
    (
        Key::ScrIdentSandboxed,
        [
            "${ident}: no permitido por el entorno aislado de los scripts",
            "${ident}: von der Script-Sandbox nicht erlaubt",
            "${ident}: non consentito dalla sandbox degli script",
            "${ident}: no es permet en l'entorn aïllat dels scripts",
            "${ident}: ní cheadaíonn bosca gainimh na scripteanna é",
        ],
    ),
    (
        Key::ScrIdentParams,
        [
            "Parámetros no válidos: ${ident}",
            "Ungültige Parameter: ${ident}",
            "Parametri non validi: ${ident}",
            "Paràmetres no vàlids: ${ident}",
            "Paraiméadair neamhbhailí: ${ident}",
        ],
    ),
    (
        Key::ScrIdentParamsDetail,
        [
            "Parámetros no válidos: ${ident} ({detail})",
            "Ungültige Parameter: ${ident} ({detail})",
            "Parametri non validi: ${ident} ({detail})",
            "Paràmetres no vàlids: ${ident} ({detail})",
            "Paraiméadair neamhbhailí: ${ident} ({detail})",
        ],
    ),
    (
        Key::ScrIdentOnlyMime,
        [
            "Parámetros no válidos: ${ident} (solo se admite «m»)",
            "Ungültige Parameter: ${ident} (nur „m“ wird unterstützt)",
            "Parametri non validi: ${ident} (è supportata solo «m»)",
            "Paràmetres no vàlids: ${ident} (només s'admet «m»)",
            "Paraiméadair neamhbhailí: ${ident} (ní thacaítear ach le 'm')",
        ],
    ),
    (
        Key::ScrTooDeep,
        [
            "Error: script demasiado profundo / bucle infinito",
            "Fehler: Script zu tief verschachtelt / Endlosschleife",
            "Errore: script troppo annidato / ciclo infinito",
            "Error: script massa profund / bucle infinit",
            "Earráid: script ró-dhomhain / lúb gan deireadh",
        ],
    ),
    (
        Key::ScrNoSuchMenuItem,
        [
            "popup: elemento de menú inexistente",
            "Popup: Menüeintrag nicht vorhanden",
            "popup: voce di menu inesistente",
            "popup: element de menú inexistent",
            "popup: níl a leithéid de mhír roghchláir ann",
        ],
    ),
    (
        Key::ScrSandbox,
        [
            "acceso denegado fuera de la carpeta de scripts: {path}",
            "Zugriff außerhalb des Script-Ordners verweigert: {path}",
            "accesso negato fuori dalla cartella degli script: {path}",
            "accés denegat fora de la carpeta dels scripts: {path}",
            "rochtain diúltaithe lasmuigh d'fhillteán na scripteanna: {path}",
        ],
    ),
    (
        Key::ScrParse,
        [
            "{file}:{line}: {problem}",
            "{file}:{line}: {problem}",
            "{file}:{line}: {problem}",
            "{file}:{line}: {problem}",
            "{file}:{line}: {problem}",
        ],
    ),
    (Key::ScrMissingClose, ["falta «}»", "„}“ fehlt", "manca «}»", "falta «}»", "'}' ar iarraidh"]),
    (Key::ScrMissingOpen, ["falta «{»", "„{“ fehlt", "manca «{»", "falta «{»", "'{' ar iarraidh"]),
    (
        Key::ScrAt,
        [
            "{msg} (línea {line}, {script})",
            "{msg} (Zeile {line}, {script})",
            "{msg} (riga {line}, {script})",
            "{msg} (línia {line}, {script})",
            "{msg} (líne {line}, {script})",
        ],
    ),
    (
        Key::ScrMadeTable,
        [
            "Tabla hash «{name}» creada ({size})",
            "Hashtabelle „{name}“ erstellt ({size})",
            "Tabella hash «{name}» creata ({size})",
            "Taula hash «{name}» creada ({size})",
            "Cruthaíodh an tábla haiseála '{name}' ({size})",
        ],
    ),
    (
        Key::ScrAddedItem,
        [
            "Elemento «{item}» añadido a la tabla hash «{table}»",
            "Eintrag „{item}“ zur Hashtabelle „{table}“ hinzugefügt",
            "Elemento «{item}» aggiunto alla tabella hash «{table}»",
            "Element «{item}» afegit a la taula hash «{table}»",
            "Cuireadh an mhír '{item}' leis an tábla haiseála '{table}'",
        ],
    ),
    (
        Key::ScrTimerNotActive,
        [
            "Timer {name} inactivo",
            "Timer {name} nicht aktiv",
            "Timer {name} non attivo",
            "Temporitzador {name} inactiu",
            "Níl an lasc ama {name} gníomhach",
        ],
    ),
    (
        Key::ScrTimerHalted,
        [
            "Timer {name} detenido",
            "Timer {name} angehalten",
            "Timer {name} fermato",
            "Temporitzador {name} aturat",
            "Stopadh an lasc ama {name}",
        ],
    ),
    (
        Key::ScrTimerActivated,
        [
            "Timer {name} activado",
            "Timer {name} aktiviert",
            "Timer {name} attivato",
            "Temporitzador {name} activat",
            "Gníomhachtaíodh an lasc ama {name}",
        ],
    ),
    (
        Key::ScrAllTimersHalted,
        [
            "Todos los timers detenidos",
            "Alle Timer angehalten",
            "Tutti i timer fermati",
            "Tots els temporitzadors aturats",
            "Stopadh na lasca ama go léir",
        ],
    ),
    (
        Key::ScrNoActiveTimers,
        [
            "Ningún timer activo",
            "Keine aktiven Timer",
            "Nessun timer attivo",
            "Cap temporitzador actiu",
            "Níl aon lasc ama gníomhach",
        ],
    ),
    (
        Key::ScrActiveTimers,
        [
            "Timers activos:",
            "Aktive Timer:",
            "Timer attivi:",
            "Temporitzadors actius:",
            "Lasca ama gníomhacha:",
        ],
    ),
    (
        Key::ScrTimerLine,
        [
            "Timer {name}, retardo {delay}: {command}",
            "Timer {name}, Verzögerung {delay}: {command}",
            "Timer {name}, ritardo {delay}: {command}",
            "Temporitzador {name}, retard {delay}: {command}",
            "Lasc ama {name}, moill {delay}: {command}",
        ],
    ),
    (
        Key::ScrTimerLineLeft,
        [
            "Timer {name}, retardo {delay}: {command} ({left} restantes)",
            "Timer {name}, Verzögerung {delay}: {command} (noch {left})",
            "Timer {name}, ritardo {delay}: {command} (ancora {left})",
            "Temporitzador {name}, retard {delay}: {command} (en queden {left})",
            "Lasc ama {name}, moill {delay}: {command} ({left} fágtha)",
        ],
    ),
    (
        Key::ScrVarSet,
        [
            "{name} ahora vale {value}",
            "{name} auf {value} gesetzt",
            "{name} impostata a {value}",
            "{name} ara val {value}",
            "Socraíodh {name} go {value}",
        ],
    ),
    (
        Key::ScrVarUnsetAll,
        [
            "Todas las variables borradas",
            "Alle Variablen gelöscht",
            "Tutte le variabili cancellate",
            "Totes les variables esborrades",
            "Glanadh na hathróga go léir",
        ],
    ),
    (
        Key::ScrVarUnset,
        [
            "{name} borrada",
            "{name} gelöscht",
            "{name} cancellata",
            "{name} esborrada",
            "Glanadh {name}",
        ],
    ),
    (
        Key::ScrVarInc,
        [
            "{name} incrementada a {value}",
            "{name} auf {value} erhöht",
            "{name} incrementata a {value}",
            "{name} incrementada a {value}",
            "Méadaíodh {name} go {value}",
        ],
    ),
    (
        Key::ScrVarDec,
        [
            "{name} decrementada a {value}",
            "{name} auf {value} verringert",
            "{name} decrementata a {value}",
            "{name} decrementada a {value}",
            "Laghdaíodh {name} go {value}",
        ],
    ),
    (
        Key::DefaultRealname,
        [
            "Usuario de AlpIRC",
            "AlpIRC-Benutzer",
            "Utente di AlpIRC",
            "Usuari d'AlpIRC",
            "Úsáideoir AlpIRC",
        ],
    ),
    (Key::DefaultAway, ["Ausente", "Abwesend", "Assente", "Absent", "As láthair"]),
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
    const SAME_AS_ENGLISH: &[Key] =
        &[Key::MsgSent, Key::NoticeSent, Key::CtcpSent, Key::NetUnreachable, Key::NetUnreachableVia, Key::ScrParse];

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
