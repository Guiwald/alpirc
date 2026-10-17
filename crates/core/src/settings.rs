//! Réglages d'AlpIRC (fichier TOML) : identité, réseaux et serveurs, favoris (Channels Folder), options rangées
//! comme dans la boîte Options de mIRC, carnet d'adresses (Notify, Ignore, Control).
//!
//! [`Settings::load`] ne échoue jamais : fichier absent → réglages par défaut ; fichier illisible → copie de
//! secours `.bak` et réglages par défaut. [`Settings::save`] écrit d'un coup (fichier temporaire puis
//! renommage), avec les droits 0600 sous Unix (le fichier contient des mots de passe).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::client::LineKind;

/// Version du format du fichier.
pub const SETTINGS_VERSION: u32 = 1;

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
pub fn default_networks() -> Vec<Network> {
    vec![
        Network::new("Undernet", vec![Server::new("irc.undernet.org", 6667, false)]),
        Network::new("Libera.Chat", vec![Server::new("irc.libera.chat", 6697, true)]),
        Network::new("OFTC", vec![Server::new("irc.oftc.net", 6697, true)]),
        Network::new("IRCnet", vec![Server::new("open.ircnet.net", 6667, false)]),
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
    pub default_message: String,
    /// Montrer une fois le message d'absence de l'autre dans un privé.
    pub show_in_queries: bool,
}

impl Default for AwayOptions {
    fn default() -> AwayOptions {
        AwayOptions { default_message: "Away".into(), show_in_queries: true }
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

/// Thèmes fournis : `classic` (mIRC) et `dark`.
pub fn themes() -> Vec<Theme> {
    let dark_colors = EventColors {
        action: 13,
        ctcp: 4,
        highlight: 7,
        info: 11,
        info2: 12,
        invite: 9,
        join: 9,
        kick: 9,
        mode: 9,
        nick: 9,
        normal: 15,
        notice: 7,
        notify: 4,
        other: 13,
        own: 15,
        part: 9,
        quit: 11,
        topic: 9,
        wallops: 7,
        whois: 15,
        error: 4,
        background: 1,
        editbox_bg: 1,
        editbox_text: 15,
        listbox_bg: 1,
        listbox_text: 15,
    };
    vec![
        Theme { name: "classic", palette: MIRC_PALETTE, colors: EventColors::default() },
        Theme {
            name: "dark",
            palette: [
                0xFFFFFF, 0x1C1C1C, 0x5C8DFF, 0x4FC14F, 0xFF5555, 0xD06060, 0xC678DD, 0xFFA040, 0xFFFF55, 0x7CFC7C, 0x2EC4C4,
                0x7FFFFF, 0x6C9CFF, 0xFF79C6, 0x9A9A9A, 0xD8D8D8,
            ],
            colors: dark_colors,
        },
    ]
}

/// Un thème fourni, par son nom.
pub fn theme(name: &str) -> Option<Theme> {
    themes().into_iter().find(|t| t.name.eq_ignore_ascii_case(name))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DisplayOptions {
    pub theme: String,
    pub chat_font: String,
    pub chat_font_size: u32,
    /// `top`, `bottom`, `left`, `right` ou `off`.
    pub switchbar: String,
    pub treebar: bool,
    pub toolbar: bool,
    pub nicklist: bool,
    pub colors: EventColors,
    pub palette: [u32; 16],
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
        }
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AddressBook {
    pub notify: Vec<NotifyEntry>,
    pub ignore: Vec<IgnoreEntry>,
    pub control: Vec<ControlEntry>,
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
