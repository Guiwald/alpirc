//! Textes de l'interface du moteur, en sept langues (ADR 0012) : anglais, français, espagnol, allemand, italien,
//! valencien, irlandais. L'anglais et le français sont complets ; les autres langues renvoient à l'anglais tant
//! qu'une passe de traduction ne les a pas remplies ([`coverage`] dit où on en est).
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
    UnknownServer => "/server: unknown network or server '{name}'", "/server : réseau ou serveur inconnu « {name} »";
    ScriptLoadError => "* Unable to load script {file}: {error}", "* Impossible de charger le script {file} : {error}";
    // --- titres, notifications ---
    TitleStatus => "Status: {nick} [{modes}] on {network} ({server}:{port})", "Status : {nick} [{modes}] sur {network} ({server}:{port})";
    TitleStatusOffline => "Status: {nick} (not connected)", "Status : {nick} (non connecté)";
    TitleChanList => "Channels List", "Liste des canaux";
    NotifHighlight => "{nick} on {chan}", "{nick} sur {chan}";
    NotifQuery => "Private message from {nick}", "Message privé de {nick}";
}

/// Traductions des autres langues : à remplir par une passe de traduction (repli sur l'anglais).
fn other(_lang: Lang, _key: Key) -> Option<&'static str> {
    None
}

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
        assert_eq!(tr(Lang::Ga, Key::Connected), "* Connected");
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
    fn coverage_report() {
        // Rapport, pas un échec : les langues non encore traduites retombent sur l'anglais.
        for (l, n, total) in coverage() {
            println!("{}: {n}/{total}", l.code());
        }
        assert_eq!(coverage()[1], (Lang::Fr, Key::ALL.len(), Key::ALL.len()));
    }
}
