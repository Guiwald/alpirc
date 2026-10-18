//! Réglages d'AlpIRC (fichier TOML) : identité, réseaux et serveurs, favoris (Channels Folder), options rangées
//! comme dans la boîte Options de mIRC, carnet d'adresses (Notify, Ignore, Control).
//!
//! [`Settings::load`] ne échoue jamais : fichier absent → réglages par défaut ; fichier illisible → copie de
//! secours `.bak` et réglages par défaut. [`Settings::save`] écrit d'un coup (fichier temporaire puis
//! renommage), avec les droits 0600 sous Unix (le fichier contient des mots de passe).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::client::LineKind;

/// Version du format du fichier (voir [`Settings::upgrade`]).
pub const SETTINGS_VERSION: u32 = 3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    /// Langue de l'interface (`en`, `fr`…) ; `None` : celle du système.
    pub language: Option<String>,
    pub identity: Identity,
    pub networks: Vec<Network>,
    pub favorites: Vec<Favorite>,
    pub connect: ConnectOptions,
    pub irc: IrcOptions,
    pub highlight: HighlightOptions,
    pub logging: LoggingOptions,
    pub flood: FloodOptions,
    pub ctcp: CtcpOptions,
    pub away: AwayOptions,
    pub catcher: CatcherOptions,
    pub display: DisplayOptions,
    pub notifications: NotificationOptions,
    /// Sons des événements et demandes de sons (Options › Sons).
    pub sounds: SoundOptions,
    pub scripts: ScriptOptions,
    pub address_book: AddressBook,
    pub dcc: DccOptions,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            version: SETTINGS_VERSION,
            language: None,
            identity: Identity::default(),
            networks: default_networks(),
            favorites: ["#lyon", "#grenoble", "#chambery"]
                .iter()
                .map(|c| Favorite {
                    channel: c.to_string(),
                    network: "Undernet".into(),
                    key: None,
                    description: String::new(),
                    join_on_connect: false,
                })
                .collect(),
            connect: ConnectOptions::default(),
            irc: IrcOptions::default(),
            highlight: HighlightOptions::default(),
            logging: LoggingOptions::default(),
            flood: FloodOptions::default(),
            ctcp: CtcpOptions::default(),
            away: AwayOptions::default(),
            catcher: CatcherOptions::default(),
            display: DisplayOptions::default(),
            notifications: NotificationOptions::default(),
            sounds: SoundOptions::default(),
            scripts: ScriptOptions::default(),
            address_book: AddressBook::default(),
            dcc: DccOptions::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Identity {
    /// Pseudo par défaut : Guiwald (ADR 0010).
    pub nick: String,
    pub alt_nick: String,
    pub realname: String,
    pub username: String,
    pub quit_message: String,
}

impl Default for Identity {
    fn default() -> Identity {
        Identity {
            nick: "Guiwald".into(),
            alt_nick: "Guiwald_".into(),
            realname: "AlpIRC user".into(),
            username: "alpirc".into(),
            quit_message: "AlpIRC — https://github.com/Guiwald/alpirc".into(),
        }
    }
}

/// Vérification du certificat TLS d'un serveur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Verify {
    /// Chaîne de confiance et nom du serveur vérifiés.
    #[default]
    Full,
    /// Certificat accepté tel quel (serveurs auto-signés).
    AcceptInvalid,
    /// Empreinte SHA-256 retenue (`fingerprint`), comme SSH.
    Pinned,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Server {
    pub host: String,
    pub port: u16,
    pub tls: bool,
    pub verify: Verify,
    /// Empreinte SHA-256 (hexadécimal) pour [`Verify::Pinned`].
    pub fingerprint: Option<String>,
    pub password: Option<String>,
    pub label: Option<String>,
}

impl Default for Server {
    fn default() -> Server {
        Server { host: String::new(), port: 6667, tls: false, verify: Verify::Full, fingerprint: None, password: None, label: None }
    }
}

impl Server {
    pub fn new(host: &str, port: u16, tls: bool) -> Server {
        Server { host: host.into(), port, tls, ..Server::default() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SaslSettings {
    /// `PLAIN`, `SCRAM-SHA-256` ou `EXTERNAL`.
    pub mechanism: String,
    pub account: String,
    pub password: String,
    /// Certificat client (EXTERNAL).
    pub client_cert: Option<PathBuf>,
}

impl Default for SaslSettings {
    fn default() -> SaslSettings {
        SaslSettings { mechanism: "PLAIN".into(), account: String::new(), password: String::new(), client_cert: None }
    }
}

/// Compte X d'Undernet (Login on Connect, ADR 0010).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UndernetX {
    pub username: String,
    pub password: String,
    /// Masquer l'adresse (`+x`).
    pub hide_host: bool,
    /// Ne se connecter que si X authentifie (`!`).
    pub require_x: bool,
}

impl Default for UndernetX {
    fn default() -> UndernetX {
        UndernetX { username: String::new(), password: String::new(), hide_host: true, require_x: true }
    }
}

/// Politique STS retenue pour un réseau.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct StsPolicy {
    pub port: u16,
    /// Fin de validité (ms depuis 1970) ; `None` : apprise sur une connexion en clair, pas encore confirmée.
    pub expires_ms: Option<i64>,
}

/// Réseau d'un bouncer (soju), découvert par `BOUNCER LISTNETWORKS`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct BouncerNetwork {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Network {
    pub name: String,
    pub servers: Vec<Server>,
    pub nick: Option<String>,
    pub alt_nick: Option<String>,
    pub realname: Option<String>,
    pub username: Option<String>,
    pub sasl: Option<SaslSettings>,
    /// Compte X d'Undernet : prime sur le mot de passe du serveur (les deux passent par `PASS`).
    pub undernet_x: Option<UndernetX>,
    pub nickserv_password: Option<String>,
    /// Commandes lancées à la connexion (après le MOTD), par le moteur de scripts.
    pub perform: Vec<String>,
    /// `auto`, `utf-8` ou `latin1`.
    pub encoding: String,
    pub auto_connect: bool,
    pub sts: Option<StsPolicy>,
    /// Si le serveur est un bouncer soju (`soju.im/bouncer-networks`) : ouvrir une connexion par réseau du bouncer.
    pub bouncer: bool,
    /// Réseaux du bouncer déjà vus.
    pub bouncer_networks: Vec<BouncerNetwork>,
}

impl Default for Network {
    fn default() -> Network {
        Network {
            name: String::new(),
            servers: Vec::new(),
            nick: None,
            alt_nick: None,
            realname: None,
            username: None,
            sasl: None,
            undernet_x: None,
            nickserv_password: None,
            perform: Vec::new(),
            encoding: "auto".into(),
            auto_connect: false,
            sts: None,
            bouncer: true,
            bouncer_networks: Vec::new(),
        }
    }
}

impl Network {
    pub fn new(name: &str, servers: Vec<Server>) -> Network {
        Network { name: name.into(), servers, ..Network::default() }
    }
}

/// Réseaux par défaut, Undernet en tête (ADR 0010). Les adresses et la prise en charge de TLS sont à vérifier
/// en vrai depuis le Mac (ADR 0010, conséquences) ; elles suivent ce que chaque réseau publie.
///
/// TLS (6697) partout où le réseau l'offre à tous ; sinon le port en clair publié. Ces valeurs ne servent qu'aux
/// nouvelles installations : un fichier de réglages existant garde ses réseaux tels quels (pas de migration).
///
/// Sources (sites officiels, relevés le 18/10/2026) :
/// - Undernet : <https://www.undernet.org/servers.php> — ports 6660-6669 et 7000, aucun port TLS publié : le
///   mot de passe du Login on Connect (X) passe donc en clair, faute de mieux.
/// - Libera.Chat : <https://libera.chat/guides/connect> — irc.libera.chat, TLS 6697 (aussi 7000, 7070).
/// - OFTC : <https://www.oftc.net/> — ircs://irc.oftc.net:6697 (aussi 9999), TLS sur tous les serveurs.
/// - IRCnet : <https://www.ircnet.com/servers> — irc.ircnet.com port 6667 (serveur ouvert à tous) ; TLS seulement
///   sur des serveurs réservés aux comptes SASL (<https://www.ircnet.com/sasl> : irc.ircnet.ca +6697).
/// - EFnet : <https://www.efnet.org/> — irc.efnet.org, 6667 ou 6697 (SSL).
/// - DALnet : <https://www.dal.net/kb/view.php?kb=189> — SSL sur le port 6697 ; <https://www.dal.net/servers/> —
///   irc.dal.net choisit un serveur.
/// - QuakeNet : <https://www.quakenet.org/servers> — irc.quakenet.org, aucun TLS publié.
/// - Rizon : <https://wiki.rizon.net/index.php?title=Servers> — irc.rizon.net, TLS 6697 ou 9999.
/// - EpiKnet : <https://www.epiknet.link/irc/> (lien du site epiknet.org) — irc.epiknet.org, 6667 ou 6697 (SSL).
pub fn default_networks() -> Vec<Network> {
    vec![
        Network::new("Undernet", vec![Server::new("irc.undernet.org", 6667, false)]),
        Network::new("Libera.Chat", vec![Server::new("irc.libera.chat", 6697, true)]),
        Network::new("OFTC", vec![Server::new("irc.oftc.net", 6697, true)]),
        Network::new("IRCnet", vec![Server::new("irc.ircnet.com", 6667, false)]),
        Network::new("EFnet", vec![Server::new("irc.efnet.org", 6697, true)]),
        Network::new("DALnet", vec![Server::new("irc.dal.net", 6697, true)]),
        Network::new("QuakeNet", vec![Server::new("irc.quakenet.org", 6667, false)]),
        Network::new("Rizon", vec![Server::new("irc.rizon.net", 6697, true)]),
        Network::new("EpiKnet", vec![Server::new("irc.epiknet.org", 6697, true)]),
    ]
}

/// Un canal du Channels Folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Favorite {
    pub channel: String,
    pub network: String,
    pub key: Option<String>,
    pub description: String,
    pub join_on_connect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConnectOptions {
    pub reconnect: bool,
    pub retry_delay_secs: u64,
    /// Plafond du délai entre deux essais (le délai double à chaque échec).
    pub max_retry_delay_secs: u64,
    /// Nombre d'essais (0 : sans fin).
    pub max_retries: u32,
    pub rejoin_channels: bool,
    pub connect_on_startup: bool,
    pub startup_networks: Vec<String>,
    pub new_window_for_server: bool,
    /// Ouvrir le dossier des canaux à la connexion (pas aux reconnexions automatiques), comme mIRC.
    pub folder_on_connect: bool,
    /// Sans rien recevoir pendant ce temps, la connexion est tenue pour morte.
    pub ping_timeout_secs: u64,
    /// Intervalle des PING de mesure de la latence.
    pub lag_interval_secs: u64,
}

impl Default for ConnectOptions {
    fn default() -> ConnectOptions {
        ConnectOptions {
            reconnect: true,
            retry_delay_secs: 10,
            max_retry_delay_secs: 60,
            max_retries: 0,
            rejoin_channels: true,
            connect_on_startup: false,
            startup_networks: Vec::new(),
            new_window_for_server: false,
            folder_on_connect: true,
            ping_timeout_secs: 240,
            lag_interval_secs: 60,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct IrcOptions {
    pub show_joins_parts: bool,
    pub show_quits: bool,
    pub show_nick_changes: bool,
    /// `<@nick>` au lieu de `<nick>`.
    pub show_mode_prefix: bool,
    pub rejoin_on_kick: bool,
    pub join_on_invite: bool,
    pub whois_in_active: bool,
    pub query_on_private: bool,
    /// Garder la fenêtre d'un canal ouverte après une expulsion ou une déconnexion.
    pub keep_channels_open: bool,
    pub timestamps: bool,
    /// Format de mIRC (`[HH:nn]`, `[HH:nn:ss]`…).
    pub timestamp_format: String,
    /// Avis privés dans la fenêtre active (sinon dans Status).
    pub notices_in_active: bool,
    /// Retirer les couleurs des messages reçus.
    pub strip_codes: bool,
    pub nick_colors: bool,
    /// Au-delà de ce nombre de lignes collées, l'interface demande confirmation.
    pub confirm_paste_lines: u32,
    /// Lignes gardées par fenêtre.
    pub max_lines: usize,
    /// Délai avant les auto-op/auto-voice (ms).
    pub auto_op_delay_ms: u64,
    /// Demander WHO à l'arrivée sur un canal (pour remplir l'IAL).
    pub who_on_join: bool,
}

impl Default for IrcOptions {
    fn default() -> IrcOptions {
        IrcOptions {
            show_joins_parts: true,
            show_quits: true,
            show_nick_changes: true,
            show_mode_prefix: false,
            rejoin_on_kick: false,
            join_on_invite: false,
            whois_in_active: false,
            query_on_private: true,
            keep_channels_open: true,
            timestamps: true,
            timestamp_format: "[HH:nn]".into(),
            notices_in_active: true,
            strip_codes: false,
            nick_colors: false,
            confirm_paste_lines: 2,
            max_lines: 5000,
            auto_op_delay_ms: 2000,
            who_on_join: false,
        }
    }
}

/// Un mot surligné.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct HighlightWord {
    pub text: String,
    pub color: Option<u8>,
    pub beep: bool,
    pub flash: bool,
    pub notify: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HighlightOptions {
    pub enabled: bool,
    /// Surligner les lignes qui contiennent notre pseudo.
    pub nick: bool,
    pub words: Vec<HighlightWord>,
    pub notify_on_highlight: bool,
    pub notify_on_query: bool,
    pub beep_on_query: bool,
    /// Bip quand notre pseudo est cité.
    pub beep: bool,
    /// Faire clignoter la fenêtre quand notre pseudo est cité.
    pub flash: bool,
}

impl Default for HighlightOptions {
    fn default() -> HighlightOptions {
        HighlightOptions {
            enabled: true,
            nick: true,
            words: Vec::new(),
            notify_on_highlight: true,
            notify_on_query: true,
            beep_on_query: false,
            beep: false,
            flash: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingOptions {
    pub enabled: bool,
    pub channels: bool,
    pub queries: bool,
    pub status: bool,
    /// Dossier des journaux (par défaut `<données>/logs`).
    pub folder: Option<PathBuf>,
    pub timestamps: bool,
    pub strip_codes: bool,
    /// Lignes du journal remises dans une fenêtre à son ouverture.
    pub reload_lines: usize,
}

impl Default for LoggingOptions {
    fn default() -> LoggingOptions {
        LoggingOptions { enabled: false, channels: true, queries: true, status: false, folder: None, timestamps: true, strip_codes: true, reload_lines: 0 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FloodOptions {
    pub enabled: bool,
    /// Lignes envoyées d'un coup avant de ralentir.
    pub burst: u32,
    /// Une ligne toutes les `rate_ms` après la rafale.
    pub rate_ms: u64,
    /// Réponses CTCP au plus par `ctcp_flood_secs`.
    pub ctcp_flood_limit: u32,
    pub ctcp_flood_secs: u64,
}

impl Default for FloodOptions {
    fn default() -> FloodOptions {
        FloodOptions { enabled: true, burst: 5, rate_ms: 2000, ctcp_flood_limit: 3, ctcp_flood_secs: 10 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CtcpOptions {
    /// Réponse à VERSION (par défaut celle d'AlpIRC).
    pub version_reply: Option<String>,
    pub reply_version: bool,
    pub reply_ping: bool,
    pub reply_time: bool,
    pub reply_finger: bool,
    pub reply_clientinfo: bool,
    pub reply_userinfo: bool,
    pub reply_source: bool,
    pub finger_text: Option<String>,
    pub userinfo_text: Option<String>,
}

impl Default for CtcpOptions {
    fn default() -> CtcpOptions {
        CtcpOptions {
            version_reply: None,
            reply_version: true,
            reply_ping: true,
            reply_time: true,
            reply_finger: true,
            reply_clientinfo: true,
            reply_userinfo: true,
            reply_source: true,
            finger_text: None,
            userinfo_text: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AwayOptions {
    /// Message de l'absence automatique (et de `/away` sans message choisi dans l'interface).
    pub default_message: String,
    /// Montrer une fois le message d'absence de l'autre dans un privé.
    pub show_in_queries: bool,
    /// Absence automatique après ce nombre de minutes sans rien taper (0 : jamais) ; retour à la saisie suivante.
    pub auto_away_minutes: u32,
}

impl Default for AwayOptions {
    fn default() -> AwayOptions {
        AwayOptions { default_message: "Away".into(), show_in_queries: true, auto_away_minutes: 0 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CatcherOptions {
    pub enabled: bool,
    pub max: usize,
}

impl Default for CatcherOptions {
    fn default() -> CatcherOptions {
        CatcherOptions { enabled: true, max: 500 }
    }
}

/// Couleurs des événements (numéros de la palette de mIRC).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EventColors {
    pub action: u8,
    pub ctcp: u8,
    pub highlight: u8,
    pub info: u8,
    pub info2: u8,
    pub invite: u8,
    pub join: u8,
    pub kick: u8,
    pub mode: u8,
    pub nick: u8,
    pub normal: u8,
    pub notice: u8,
    pub notify: u8,
    pub other: u8,
    pub own: u8,
    pub part: u8,
    pub quit: u8,
    pub topic: u8,
    pub wallops: u8,
    pub whois: u8,
    pub error: u8,
    pub background: u8,
    pub editbox_bg: u8,
    pub editbox_text: u8,
    pub listbox_bg: u8,
    pub listbox_text: u8,
}

impl Default for EventColors {
    fn default() -> EventColors {
        EventColors {
            action: 6,
            ctcp: 4,
            highlight: 4,
            info: 2,
            info2: 12,
            invite: 3,
            join: 3,
            kick: 3,
            mode: 3,
            nick: 3,
            normal: 1,
            notice: 5,
            notify: 4,
            other: 6,
            own: 1,
            part: 3,
            quit: 2,
            topic: 3,
            wallops: 5,
            whois: 1,
            error: 4,
            background: 0,
            editbox_bg: 0,
            editbox_text: 1,
            listbox_bg: 0,
            listbox_text: 1,
        }
    }
}

impl EventColors {
    /// Couleur d'une sorte de ligne.
    pub fn color_index(&self, kind: LineKind) -> u8 {
        use LineKind::*;
        match kind {
            Normal => self.normal,
            Action => self.action,
            Ctcp => self.ctcp,
            Highlight => self.highlight,
            Info => self.info,
            Info2 => self.info2,
            Invite => self.invite,
            Join => self.join,
            Kick => self.kick,
            Mode => self.mode,
            Nick => self.nick,
            Notice => self.notice,
            Notify => self.notify,
            Other => self.other,
            Own => self.own,
            Part => self.part,
            Quit => self.quit,
            Topic => self.topic,
            Wallops => self.wallops,
            Whois => self.whois,
            Error => self.error,
        }
    }
}

/// Les 16 couleurs de base de mIRC.
pub const MIRC_PALETTE: [u32; 16] = [
    0xFFFFFF, 0x000000, 0x00007F, 0x009300, 0xFF0000, 0x7F0000, 0x9C009C, 0xFC7F00, 0xFFFF00, 0x00FC00, 0x009393, 0x00FFFF,
    0x0000FC, 0xFF00FF, 0x7F7F7F, 0xD2D2D2,
];

/// Un thème : palette des 16 couleurs de base et couleurs des événements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub name: &'static str,
    pub palette: [u32; 16],
    pub colors: EventColors,
}

/// Palette du thème sombre, la même que celle de l'interface (`alpirc_ui::Theme::dark`) : comme les thèmes sombres
/// de mIRC, le blanc (0) et le noir (1) sont échangés, si bien que les numéros par défaut (fond 0, texte 1) et le
/// texte « noir » des autres restent lisibles ; les couleurs foncées sont éclaircies.
pub const DARK_PALETTE: [u32; 16] = [
    0x1C1C1C, 0xE0E0E0, 0x6C8CFF, 0x4CC24C, 0xFF5C5C, 0xD08A5C, 0xC77DE0, 0xFFA040, 0xFFE65C, 0x7CFF7C, 0x3CC0C0, 0x7CFFFF,
    0x7C9CFF, 0xFF7CFF, 0x9A9A9A, 0x3C3C3C,
];

/// Thèmes fournis : `classic` (mIRC) et `dark`. Ils ne se modifient pas : la boîte Couleurs enregistre les
/// changements sous un autre nom ([`UserTheme`]).
pub fn themes() -> Vec<Theme> {
    vec![
        Theme { name: "classic", palette: MIRC_PALETTE, colors: EventColors::default() },
        Theme { name: "dark", palette: DARK_PALETTE, colors: EventColors::default() },
    ]
}

/// Un thème fourni, par son nom (`classic`/`classique`, `dark`/`sombre`).
pub fn theme(name: &str) -> Option<Theme> {
    let name = match name.to_ascii_lowercase().as_str() {
        "classique" => "classic",
        "sombre" => "dark",
        _ => name,
    };
    themes().into_iter().find(|t| t.name.eq_ignore_ascii_case(name))
}

/// Vrai pour le nom d'un thème fourni (ces noms sont réservés : un thème de l'utilisateur ne peut pas les prendre).
pub fn is_builtin_theme(name: &str) -> bool {
    theme(name.trim()).is_some()
}

/// Un jeu de couleurs enregistré par l'utilisateur (boîte Couleurs, « Enregistrer »).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserTheme {
    pub name: String,
    /// Thème fourni dont il garde l'allure des fenêtres et des boîtes de dialogue : `classic` ou `dark`.
    pub base: String,
    pub palette: [u32; 16],
    pub colors: EventColors,
}

impl Default for UserTheme {
    fn default() -> UserTheme {
        UserTheme { name: String::new(), base: "classic".into(), palette: MIRC_PALETTE, colors: EventColors::default() }
    }
}

/// Pourquoi un thème de l'utilisateur ne peut pas être enregistré sous ce nom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeNameError {
    Empty,
    /// Nom d'un thème fourni (Classique, Sombre).
    Builtin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayOptions {
    /// Thème choisi : `classic`, `dark`, ou le nom d'un thème de l'utilisateur (`user_themes`).
    pub theme: String,
    /// Police des discussions : `Fixedsys Excelsior` (intégrée, par défaut), `Pixel Operator` (intégrée) ou une
    /// police à chasse fixe du système, par son nom de famille.
    pub chat_font: String,
    /// Taille de la police des discussions (pixels) ; pour les polices intégrées, hauteur de ligne en multiples de 16.
    pub chat_font_size: u32,
    /// `top`, `bottom`, `left`, `right` ou `off`.
    pub switchbar: String,
    pub treebar: bool,
    pub toolbar: bool,
    pub nicklist: bool,
    /// Couleurs en vigueur (chargées depuis un thème, puis éventuellement retouchées dans la boîte Couleurs).
    pub colors: EventColors,
    pub palette: [u32; 16],
    /// Thèmes enregistrés par l'utilisateur.
    pub user_themes: Vec<UserTheme>,
    /// Aperçu d'une police dans la boîte Options (famille, taille), pas encore choisie : jamais enregistré.
    #[serde(skip)]
    pub font_preview: Option<(String, u32)>,
}

impl Default for DisplayOptions {
    fn default() -> DisplayOptions {
        DisplayOptions {
            theme: "classic".into(),
            chat_font: "Fixedsys Excelsior".into(),
            chat_font_size: 16,
            switchbar: "top".into(),
            treebar: false,
            toolbar: true,
            nicklist: true,
            colors: EventColors::default(),
            palette: MIRC_PALETTE,
            user_themes: Vec::new(),
            font_preview: None,
        }
    }
}

impl DisplayOptions {
    /// Thème de l'utilisateur, par son nom (sans tenir compte de la casse).
    pub fn user_theme(&self, name: &str) -> Option<&UserTheme> {
        let name = name.trim();
        self.user_themes.iter().find(|t| t.name.eq_ignore_ascii_case(name))
    }

    /// Thème fourni qui donne l'allure des fenêtres et des boîtes (`classic` ou `dark`) : le thème choisi, ou la
    /// base du thème de l'utilisateur choisi.
    pub fn theme_base(&self) -> &'static str {
        let base = match self.user_theme(&self.theme) {
            Some(u) => u.base.as_str(),
            None => self.theme.as_str(),
        };
        match theme(base) {
            Some(t) => t.name,
            None => "classic",
        }
    }

    /// Noms des thèmes proposés : les thèmes fournis, puis ceux de l'utilisateur.
    pub fn theme_names(&self) -> Vec<String> {
        themes().iter().map(|t| t.name.to_string()).chain(self.user_themes.iter().map(|t| t.name.clone())).collect()
    }

    /// Palette et couleurs d'un thème (fourni ou de l'utilisateur).
    pub fn theme_colors(&self, name: &str) -> Option<([u32; 16], EventColors)> {
        match (self.user_theme(name), theme(name.trim())) {
            (Some(u), _) => Some((u.palette, u.colors.clone())),
            (None, Some(t)) => Some((t.palette, t.colors)),
            _ => None,
        }
    }

    /// Choisit un thème : son nom, sa palette et ses couleurs. Faux (rien ne change) s'il n'existe pas.
    pub fn select_theme(&mut self, name: &str) -> bool {
        let Some((palette, colors)) = self.theme_colors(name) else { return false };
        self.theme = match (self.user_theme(name), theme(name.trim())) {
            (Some(u), _) => u.name.clone(),
            (None, Some(t)) => t.name.to_string(),
            _ => return false,
        };
        self.palette = palette;
        self.colors = colors;
        true
    }

    /// Enregistre un thème de l'utilisateur (remplace celui du même nom) ; rend son nom tel qu'enregistré.
    pub fn store_theme(&mut self, mut t: UserTheme) -> Result<String, ThemeNameError> {
        t.name = t.name.trim().to_string();
        if t.name.is_empty() {
            return Err(ThemeNameError::Empty);
        }
        if is_builtin_theme(&t.name) {
            return Err(ThemeNameError::Builtin);
        }
        if theme(&t.base).is_none() {
            t.base = "classic".into();
        }
        let name = t.name.clone();
        match self.user_themes.iter_mut().find(|u| u.name.eq_ignore_ascii_case(&name)) {
            Some(u) => *u = t,
            None => self.user_themes.push(t),
        }
        Ok(name)
    }

    /// Supprime un thème de l'utilisateur. S'il était choisi, le thème choisi devient sa base ; les couleurs en
    /// vigueur restent. Faux s'il n'existe pas.
    pub fn delete_theme(&mut self, name: &str) -> bool {
        let Some(i) = self.user_themes.iter().position(|u| u.name.eq_ignore_ascii_case(name.trim())) else { return false };
        let t = self.user_themes.remove(i);
        if self.theme.eq_ignore_ascii_case(&t.name) {
            self.theme = theme(&t.base).map(|b| b.name.to_string()).unwrap_or_else(|| "classic".into());
        }
        true
    }

    /// Palette et couleurs à afficher. Ce sont celles des réglages, sauf si elles sont restées celles d'un thème
    /// fourni alors que le thème choisi en est un autre (le menu Affichage › Thème change le thème sans toucher aux
    /// couleurs, et les anciens réglages « sombres » gardaient la palette classique) : alors celles du thème choisi.
    pub fn effective_colors(&self) -> ([u32; 16], EventColors) {
        let preset = themes().into_iter().any(|t| t.palette == self.palette && t.colors == self.colors);
        if preset
            && self.user_theme(&self.theme).is_none()
            && let Some(t) = theme(&self.theme)
        {
            return (t.palette, t.colors);
        }
        (self.palette, self.colors.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NotificationOptions {
    pub enabled: bool,
    /// Seulement quand la fenêtre n'est pas active ou que l'appli n'a pas le focus.
    pub only_when_inactive: bool,
}

impl Default for NotificationOptions {
    fn default() -> NotificationOptions {
        NotificationOptions { enabled: true, only_when_inactive: true }
    }
}

/// Ce que joue un événement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SoundPlay {
    /// Rien.
    #[default]
    None,
    /// Le bip du système.
    Beep,
    /// Un fichier son (`file`).
    File,
}

/// Le son d'un événement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct EventSound {
    pub play: SoundPlay,
    /// Nom d'un fichier du dossier des sons (`<données>/sounds`) ou chemin complet ; gardé même quand `play`
    /// n'est pas `file`, pour le retrouver.
    pub file: String,
}

impl EventSound {
    /// Le fichier `file`, joué si `on`.
    pub fn file(file: &str, on: bool) -> EventSound {
        EventSound { play: if on { SoundPlay::File } else { SoundPlay::None }, file: file.into() }
    }
}

/// Les événements qui peuvent jouer un son (Options › Sons).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundEvent {
    /// On vous nomme (pseudo ou mot surligné), dans une fenêtre hors de vue.
    Highlight,
    /// Message privé, dans un privé hors de vue.
    Query,
    /// Un pseudo de la liste Notify arrive ou part.
    Notify,
    /// Offre DCC (envoi de fichier ou discussion) à accepter.
    Dcc,
    /// Connecté à un serveur (fin du MOTD).
    Connect,
    /// Déconnecté sans l'avoir demandé.
    Disconnect,
    /// Expulsé d'un canal.
    Kick,
    /// Invité sur un canal.
    Invite,
    /// Message d'un canal dont la fenêtre n'est pas active.
    ChannelMessage,
}

impl SoundEvent {
    pub const ALL: [SoundEvent; 9] = [
        SoundEvent::Highlight,
        SoundEvent::Query,
        SoundEvent::Notify,
        SoundEvent::Dcc,
        SoundEvent::Connect,
        SoundEvent::Disconnect,
        SoundEvent::Kick,
        SoundEvent::Invite,
        SoundEvent::ChannelMessage,
    ];

    /// Nom de l'événement (celui de la section du fichier de réglages, et des textes de l'interface).
    pub fn name(self) -> &'static str {
        match self {
            SoundEvent::Highlight => "highlight",
            SoundEvent::Query => "query",
            SoundEvent::Notify => "notify",
            SoundEvent::Dcc => "dcc",
            SoundEvent::Connect => "connect",
            SoundEvent::Disconnect => "disconnect",
            SoundEvent::Kick => "kick",
            SoundEvent::Invite => "invite",
            SoundEvent::ChannelMessage => "channel_message",
        }
    }

    /// Son fourni avec AlpIRC pour cet événement (voir [`BUILTIN_SOUNDS`]).
    pub fn default_file(self) -> &'static str {
        match self {
            SoundEvent::Highlight => "alpirc-highlight.wav",
            SoundEvent::Query => "alpirc-query.wav",
            SoundEvent::Notify => "alpirc-notify.wav",
            SoundEvent::Dcc => "alpirc-dcc.wav",
            SoundEvent::Connect => "alpirc-connect.wav",
            SoundEvent::Disconnect => "alpirc-disconnect.wav",
            SoundEvent::Kick => "alpirc-kick.wav",
            SoundEvent::Invite => "alpirc-invite.wav",
            SoundEvent::ChannelMessage => "alpirc-message.wav",
        }
    }
}

/// Sons fournis avec AlpIRC, installés dans `<données>/sounds` au démarrage s'ils manquent (voir
/// `assets/sounds/src/make_sounds.py`).
pub const BUILTIN_SOUNDS: [&str; 10] = [
    "alpirc-highlight.wav",
    "alpirc-query.wav",
    "alpirc-notify.wav",
    "alpirc-dcc.wav",
    "alpirc-connect.wav",
    "alpirc-disconnect.wav",
    "alpirc-kick.wav",
    "alpirc-invite.wav",
    "alpirc-message.wav",
    "alpirc-ding.wav",
];

/// Sons (Options › Sons de mIRC) : un choix par événement, tout couper, demandes de sons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SoundOptions {
    /// Tout couper : sons des événements, bips des alertes (surlignage, privés) et demandes de sons. `/splay` et
    /// `/beep`, demandés exprès, jouent toujours.
    pub mute: bool,
    pub highlight: EventSound,
    pub query: EventSound,
    pub notify: EventSound,
    pub dcc: EventSound,
    pub connect: EventSound,
    pub disconnect: EventSound,
    pub kick: EventSound,
    pub invite: EventSound,
    pub channel_message: EventSound,
    /// Demandes de sons (CTCP SOUND, `/sound`) : jouer le fichier demandé s'il est dans le dossier des sons
    /// (sinon : « veut vous faire écouter… »).
    pub accept_requests: bool,
}

impl Default for SoundOptions {
    /// Agréable sans être bruyant : seuls « on vous nomme » et les messages privés sonnent ; les autres
    /// événements ont leur son prêt, à activer.
    fn default() -> SoundOptions {
        let f = |e: SoundEvent, on: bool| EventSound::file(e.default_file(), on);
        SoundOptions {
            mute: false,
            highlight: f(SoundEvent::Highlight, true),
            query: f(SoundEvent::Query, true),
            notify: f(SoundEvent::Notify, false),
            dcc: f(SoundEvent::Dcc, false),
            connect: f(SoundEvent::Connect, false),
            disconnect: f(SoundEvent::Disconnect, false),
            kick: f(SoundEvent::Kick, false),
            invite: f(SoundEvent::Invite, false),
            channel_message: f(SoundEvent::ChannelMessage, false),
            accept_requests: false,
        }
    }
}

impl SoundOptions {
    /// Le son d'un événement.
    pub fn get(&self, e: SoundEvent) -> &EventSound {
        match e {
            SoundEvent::Highlight => &self.highlight,
            SoundEvent::Query => &self.query,
            SoundEvent::Notify => &self.notify,
            SoundEvent::Dcc => &self.dcc,
            SoundEvent::Connect => &self.connect,
            SoundEvent::Disconnect => &self.disconnect,
            SoundEvent::Kick => &self.kick,
            SoundEvent::Invite => &self.invite,
            SoundEvent::ChannelMessage => &self.channel_message,
        }
    }

    pub fn get_mut(&mut self, e: SoundEvent) -> &mut EventSound {
        match e {
            SoundEvent::Highlight => &mut self.highlight,
            SoundEvent::Query => &mut self.query,
            SoundEvent::Notify => &mut self.notify,
            SoundEvent::Dcc => &mut self.dcc,
            SoundEvent::Connect => &mut self.connect,
            SoundEvent::Disconnect => &mut self.disconnect,
            SoundEvent::Kick => &mut self.kick,
            SoundEvent::Invite => &mut self.invite,
            SoundEvent::ChannelMessage => &mut self.channel_message,
        }
    }
}

/// Chemin d'un son des réglages : un nom se lit dans `<données>/sounds`, un chemin complet tel quel. `None` pour
/// un nom vide ou qui remonte hors du dossier des sons (`..`).
pub fn sound_path(data_dir: &Path, file: &str) -> Option<PathBuf> {
    let file = file.trim();
    if file.is_empty() {
        return None;
    }
    let p = Path::new(file);
    if p.is_absolute() {
        return Some(p.to_path_buf());
    }
    if p.components().any(|c| !matches!(c, std::path::Component::Normal(_) | std::path::Component::CurDir)) {
        return None;
    }
    Some(data_dir.join("sounds").join(p))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ScriptOptions {
    pub enabled: bool,
    pub warn_on_load: bool,
}

impl Default for ScriptOptions {
    fn default() -> ScriptOptions {
        ScriptOptions { enabled: true, warn_on_load: true }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct NotifyEntry {
    pub nick: String,
    pub note: String,
    pub network: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct IgnoreEntry {
    /// Masque `pseudo!ident@hôte` (un pseudo seul vaut `pseudo!*@*`).
    pub mask: String,
    pub private: bool,
    pub channel: bool,
    pub notice: bool,
    pub ctcp: bool,
    pub invite: bool,
    /// Retirer les couleurs (au lieu d'ignorer).
    pub codes: bool,
    pub dcc: bool,
    pub network: Option<String>,
    /// Fin de l'ignorance (ms depuis 1970), pour `/ignore -uN`.
    pub until_ms: Option<i64>,
}

/// Sorte d'entrée de la liste Control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ControlKind {
    #[default]
    Autoop,
    Autovoice,
    Protect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ControlEntry {
    pub kind: ControlKind,
    pub mask: String,
    /// Canaux concernés (vide : tous).
    pub channels: Vec<String>,
    pub network: Option<String>,
}

/// Une personne du carnet d'adresses (section « Info » de mIRC) : ce qu'on sait d'elle, gardé d'une session à
/// l'autre et montré par User Central (`/uwho`). Rangée par pseudo, comme dans mIRC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PersonEntry {
    pub nick: String,
    /// Nom réel.
    pub name: String,
    pub email: String,
    pub website: String,
    /// Notes libres (plusieurs lignes).
    pub notes: String,
}

impl PersonEntry {
    /// Vrai si rien d'autre que le pseudo n'est rempli.
    pub fn is_blank(&self) -> bool {
        self.name.trim().is_empty() && self.email.trim().is_empty() && self.website.trim().is_empty() && self.notes.trim().is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AddressBook {
    /// Les personnes (onglet « Infos »).
    pub people: Vec<PersonEntry>,
    pub notify: Vec<NotifyEntry>,
    pub ignore: Vec<IgnoreEntry>,
    pub control: Vec<ControlEntry>,
    /// Interrupteurs des listes Control (`/aop on|off`, `/avoice`, `/protect`), comme dans mIRC.
    pub autoop_on: bool,
    pub autovoice_on: bool,
    pub protect_on: bool,
}

impl Default for AddressBook {
    fn default() -> AddressBook {
        AddressBook {
            people: Vec::new(),
            notify: Vec::new(),
            ignore: Vec::new(),
            control: Vec::new(),
            autoop_on: true,
            autovoice_on: true,
            protect_on: true,
        }
    }
}

impl AddressBook {
    /// La liste Control d'une sorte est-elle active ?
    pub fn control_on(&self, kind: ControlKind) -> bool {
        match kind {
            ControlKind::Autoop => self.autoop_on,
            ControlKind::Autovoice => self.autovoice_on,
            ControlKind::Protect => self.protect_on,
        }
    }

    /// Allume ou éteint la liste Control d'une sorte.
    pub fn set_control_on(&mut self, kind: ControlKind, on: bool) {
        match kind {
            ControlKind::Autoop => self.autoop_on = on,
            ControlKind::Autovoice => self.autovoice_on = on,
            ControlKind::Protect => self.protect_on = on,
        }
    }

    /// La fiche d'une personne, par son pseudo (sans tenir compte de la casse).
    pub fn person(&self, nick: &str) -> Option<&PersonEntry> {
        self.people.iter().find(|p| p.nick.eq_ignore_ascii_case(nick))
    }

    /// Enregistre la fiche d'une personne (la remplace si le pseudo y est déjà) ; une fiche vide la retire.
    pub fn set_person(&mut self, e: PersonEntry) {
        let i = self.people.iter().position(|p| p.nick.eq_ignore_ascii_case(&e.nick));
        match (i, e.is_blank()) {
            (Some(i), true) => {
                self.people.remove(i);
            }
            (Some(i), false) => self.people[i] = e,
            (None, false) => self.people.push(e),
            (None, true) => {}
        }
    }
}

/// Que faire d'une offre d'envoi de fichier (DCC SEND) reçue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum DccSendAction {
    /// Demander (par défaut).
    #[default]
    Ask,
    /// Accepter tout seul, sauf les fichiers dangereux (exécutables, scripts…), toujours demandés.
    AutoGet,
    /// Ignorer sans rien demander.
    Ignore,
}

/// Que faire d'une demande de discussion (DCC CHAT) reçue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum DccChatAction {
    #[default]
    Ask,
    AutoAccept,
    Ignore,
}

/// Que faire quand le fichier reçu existe déjà.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum DccExists {
    /// Demander (reprendre, écraser ou renommer).
    #[default]
    Ask,
    /// Reprendre là où le fichier s'arrête (DCC RESUME).
    Resume,
    Overwrite,
    /// Garder les deux : `nom (1).ext`.
    Rename,
}

/// DCC : envois et réceptions de fichiers, discussions directes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DccOptions {
    pub on_send: DccSendAction,
    pub on_chat: DccChatAction,
    /// Dossier des fichiers reçus ; `None` : `Téléchargements/AlpIRC` (selon le système), sinon
    /// `<données>/downloads`.
    pub folder: Option<PathBuf>,
    /// Nos envois en DCC passif (c'est l'autre qui écoute) : marche derrière une box.
    pub passive: bool,
    /// Nos demandes de discussion en DCC passif (moins de clients savent y répondre).
    pub passive_chat: bool,
    /// Ports d'écoute (0 et 0 : choisi par le système).
    pub port_min: u16,
    pub port_max: u16,
    /// Taille des blocs envoyés (octets).
    pub packet_size: u32,
    pub on_exists: DccExists,
    /// Délai d'attente d'une réponse, d'une connexion, ou de données pendant un transfert (secondes).
    pub timeout_secs: u64,
    /// Adresse IP annoncée dans nos offres (adresse publique derrière une box) ; `None` : celle vue par le
    /// serveur si elle est publique, sinon l'adresse locale de la connexion.
    pub own_ip: Option<String>,
}

impl Default for DccOptions {
    fn default() -> DccOptions {
        DccOptions {
            on_send: DccSendAction::Ask,
            on_chat: DccChatAction::Ask,
            folder: None,
            passive: true,
            passive_chat: false,
            port_min: 0,
            port_max: 0,
            packet_size: 16 * 1024,
            on_exists: DccExists::Ask,
            timeout_secs: 120,
            own_ip: None,
        }
    }
}

/// Erreur d'écriture des réglages.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("écriture impossible : {0}")]
    Io(#[from] std::io::Error),
    #[error("mise en forme TOML impossible : {0}")]
    Toml(#[from] toml::ser::Error),
}

impl Settings {
    /// Lit le fichier ; réglages par défaut s'il manque, copie `.bak` et réglages par défaut s'il est illisible.
    pub fn load(path: &Path) -> Settings {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Settings::default(),
            Err(e) => {
                log::warn!("réglages illisibles ({}) : {e}", path.display());
                let _ = std::fs::copy(path, bak_path(path));
                return Settings::default();
            }
        };
        match Settings::from_toml(&text) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("réglages invalides ({}) : {e}", path.display());
                let _ = std::fs::write(bak_path(path), &text);
                Settings::default()
            }
        }
    }

    pub fn from_toml(text: &str) -> Result<Settings, toml::de::Error> {
        let mut s: Settings = toml::from_str(text)?;
        if s.version == 0 {
            s.version = SETTINGS_VERSION;
        }
        Ok(s)
    }

    /// Mises à jour des réglages d'une version précédente, faites une seule fois (le numéro de version enregistré
    /// dit lesquelles sont faites) ; `mobile` : sur le téléphone ; `new_install` : pas encore de fichier de
    /// réglages. Rend vrai si quelque chose a changé (à enregistrer).
    ///
    /// - 2 : « Whois dans la fenêtre active » cochée sur le téléphone (une seule fenêtre visible à la fois : la
    ///   réponse dans Status passerait inaperçue). Sur l'ordinateur, la case reste décochée, comme dans mIRC.
    /// - 3 : le dossier des canaux s'ouvre à la connexion sur l'ordinateur, comme dans mIRC ; pas sur le téléphone
    ///   (une boîte à chaque connexion y gênerait).
    pub fn upgrade(&mut self, mobile: bool, new_install: bool) -> bool {
        let old = self.version;
        if mobile && (new_install || old < 2) {
            self.irc.whois_in_active = true;
        }
        if mobile && (new_install || old < 3) {
            self.connect.folder_on_connect = false;
        }
        self.version = self.version.max(SETTINGS_VERSION);
        self.version != old || (mobile && new_install)
    }

    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }

    /// Écrit le fichier d'un coup (temporaire puis renommage), droits 0600 sous Unix.
    pub fn save(&self, path: &Path) -> Result<(), SettingsError> {
        let text = self.to_toml()?;
        if let Some(dir) = path.parent()
            && !dir.as_os_str().is_empty()
        {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("toml.tmp");
        {
            use std::io::Write;
            let mut opts = std::fs::OpenOptions::new();
            opts.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                opts.mode(0o600);
            }
            let mut f = opts.open(&tmp)?;
            f.write_all(text.as_bytes())?;
            f.sync_all()?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
        }
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Couleur d'une sorte de ligne (numéro de la palette).
    pub fn color_index(&self, kind: LineKind) -> u8 {
        self.display.colors.color_index(kind)
    }

    /// Réseau par son nom (sans tenir compte de la casse).
    pub fn network(&self, name: &str) -> Option<&Network> {
        self.networks.iter().find(|n| n.name.eq_ignore_ascii_case(name))
    }

    pub fn network_mut(&mut self, name: &str) -> Option<&mut Network> {
        self.networks.iter_mut().find(|n| n.name.eq_ignore_ascii_case(name))
    }

    /// Réseau dont un serveur porte cet hôte.
    pub fn network_for_host(&self, host: &str) -> Option<&Network> {
        self.networks.iter().find(|n| n.servers.iter().any(|s| s.host.eq_ignore_ascii_case(host)))
    }
}

fn bak_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".bak");
    PathBuf::from(s)
}
