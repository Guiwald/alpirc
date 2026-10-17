//! Moteur de scripts mSL d'AlpIRC (ADR 0007).
//!
//! Le moteur ne connaît ni le réseau ni l'interface : tout ce qui touche à l'état du client (envoyer un message,
//! écrire dans une fenêtre, lire la liste des pseudos d'un canal…) passe par le trait [`Host`], que le client
//! (`alpirc-core`) implémente. Le moteur gère lui-même le langage : lecture des fichiers, alias, popups, remotes
//! (`on …`, `ctcp …`, `raw …`), groupes, variables, identifiants « purs » (`$calc`, `$len`, `$gettok`…), contrôle
//! de flux, timers, tables de hachage, fichiers du bac à sable, niveaux d'utilisateurs et dialogues personnalisés.
//!
//! Bac à sable : pas de `/run`, pas de `$dll`/`$com`, fichiers limités au dossier racine donné à [`Engine::new`].

use std::path::{Path, PathBuf};

/// Erreur d'un script (lecture ou exécution). Le texte est affiché à l'utilisateur, en anglais comme mIRC
/// (`* /msg: insufficient parameters`), le client le traduit si besoin.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScriptError {
    #[error("{0}")]
    Message(String),
    #[error("{file}:{line}: {message}")]
    Parse { file: String, line: usize, message: String },
    #[error("accès refusé hors du dossier des scripts : {0}")]
    Sandbox(String),
}

/// Ce que contient un fichier de script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptKind {
    /// Fichier d'alias seuls (format de l'onglet Aliases : `nom commandes` par ligne, ou `alias nom { … }`).
    Aliases,
    /// Fichier de popups seuls (format de l'onglet Popups : sections `[status]`, `[channel]`… ou un seul menu).
    Popups,
    /// Fichier « remote » (`.mrc`) : `on`, `ctcp`, `raw`, `alias`, `menu`, `dialog`, groupes `#nom on|off`.
    Remote,
}

/// Un fichier de script chargé.
#[derive(Debug, Clone)]
pub struct LoadedScript {
    pub name: String,
    pub path: Option<PathBuf>,
    pub kind: ScriptKind,
}

/// Les sortes de menus de popups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PopupKind {
    Status,
    Channel,
    Query,
    Nicklist,
    Menubar,
    /// Menu d'une fenêtre `@` (`menu @nom { … }`).
    Custom,
}

/// Une entrée de menu, titres déjà évalués.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItem {
    /// Libellé (`-` pour un séparateur). Un `&` marque la lettre de raccourci, comme dans mIRC.
    pub label: String,
    pub enabled: bool,
    pub checked: bool,
    pub children: Vec<MenuItem>,
    /// À passer à [`Engine::popup_run`] ; `None` pour un séparateur ou un sous-menu.
    pub action: Option<u32>,
}

/// Sortes d'événements (`on <niveau>:<SORTE>:…`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventKind {
    Start,
    Exit,
    Load,
    Unload,
    Connect,
    Disconnect,
    Logon,
    Text,
    Action,
    Notice,
    Join,
    Part,
    Quit,
    Kick,
    Nick,
    Topic,
    Invite,
    /// Mode de canal (`on MODE`), une fois par ligne MODE.
    Mode,
    /// Chaque mode, un par un (`on RAWMODE`).
    RawMode,
    Op,
    Deop,
    Help,
    Dehelp,
    Voice,
    Devoice,
    Ban,
    Unban,
    UserMode,
    /// Requête CTCP (`ctcp <niveau>:<commande>:<cible>:…`).
    Ctcp,
    CtcpReply,
    /// Ligne brute du serveur (`raw <numérique ou commande>:<texte>:…`).
    Raw,
    Input,
    Open,
    Close,
    Active,
    Notify,
    Unotify,
    Snotice,
    Wallops,
    Error,
    Ping,
    Pong,
    /// DCC CHAT : texte reçu (`on CHAT`).
    Chat,
    FileSent,
    FileRcvd,
    SendFail,
    GetFail,
    Signal,
    Dialog,
    KeyDown,
    KeyUp,
}

/// Contexte d'un événement. Le moteur en tire `$nick`, `$address`, `$chan`, `$target`, `$1-`, `$knick`,
/// `$newnick`, `$opnick`, `$bnick`, `$numeric`, `$rawmsg`, `$signal`, `$dname`, `$devent`, `$did`…
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventInfo {
    pub kind: Option<EventKind>,
    /// Connexion concernée (`$cid`).
    pub conn: Option<u32>,
    /// Auteur.
    pub nick: String,
    /// `ident@hôte` de l'auteur.
    pub address: String,
    /// Canal concerné, s'il y en a un.
    pub chan: Option<String>,
    /// Cible du message (canal, ou notre pseudo pour un privé).
    pub target: Option<String>,
    /// Texte (`$1-`), sans codes retirés.
    pub text: String,
    /// Pour CTCP : la commande (`VERSION`) ; pour RAW : la commande ou le numérique ; pour SIGNAL : le nom ;
    /// pour MODE et RAWMODE : la chaîne de modes ; pour DIALOG : l'événement (`sclick`, `init`…).
    pub what: String,
    /// Valeurs propres à l'événement : `knick` (KICK), `newnick` (NICK), `opnick`/`vnick`/`hnick` (OP…),
    /// `bnick`/`banmask` (BAN), `dname`/`did` (DIALOG), `keyval`/`keychar` (KEYDOWN), `filename` (FILE*),
    /// `numeric`, `rawmsg`, `msgtags`.
    pub extra: Vec<(String, String)>,
}

impl EventInfo {
    pub fn get(&self, key: &str) -> Option<&str> {
        self.extra.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }
}

/// Contexte d'exécution donné par le client.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ctx {
    /// Connexion courante (`$cid`, cible implicite de `/msg`, `/join`…).
    pub conn: Option<u32>,
    /// Nom de la fenêtre courante (`$active` dans une saisie, fenêtre d'un popup…).
    pub window: Option<String>,
    /// Pour un popup de la liste des pseudos : les pseudos sélectionnés (`$snicks`, `$1`).
    pub selected: Vec<String>,
    /// Événement en cours, s'il y en a un.
    pub event: Option<EventInfo>,
}

/// Résultat du déclenchement d'un événement.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FireResult {
    /// `/haltdef` ou `/halt` : le client n'affiche pas son texte habituel.
    pub haltdef: bool,
}

/// Événement venu d'un dialogue (clic, saisie…), à passer à [`Engine::dialog_event`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogEvent {
    /// Ouverture (`init`, id 0).
    Init,
    /// Clic simple (`sclick`) sur un contrôle.
    Click(u32),
    /// Double clic (`dclick`).
    DoubleClick(u32),
    /// Texte modifié (`edit`) : le client a déjà mis le nouveau texte dans le contrôle.
    Edit(u32),
    /// Sélection changée dans une liste/combo : le client a déjà mis à jour la sélection.
    Select(u32),
    /// Fermeture (`close`, id 0) : bouton de fermeture, Échap, ou bouton `cancel`.
    Close,
    /// Bouton `ok` : fermeture après `sclick`.
    Ok(u32),
}

/// Sortes de contrôles de dialogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKind {
    Text,
    Edit,
    Button,
    Check,
    Radio,
    Box,
    List,
    Combo,
    Icon,
    Link,
    Tab,
    Scroll,
}

/// Un contrôle d'un dialogue ouvert. Le client le dessine et met à jour `text`, `selected` et `checked` quand
/// l'utilisateur agit, puis appelle [`Engine::dialog_event`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Control {
    pub id: u32,
    pub kind: ControlKind,
    /// x, y, largeur, hauteur (en pixels, ou en unités de dialogue si [`Dialog::dbu`]).
    pub rect: (i32, i32, i32, i32),
    pub text: String,
    /// Lignes d'une liste ou d'une combo ; lignes d'un `edit` multiligne.
    pub items: Vec<String>,
    /// Lignes sélectionnées (numéros à partir de 1, comme `$did().sel`).
    pub selected: Vec<usize>,
    pub checked: bool,
    pub enabled: bool,
    pub visible: bool,
    /// Styles écrits dans la table (`multi`, `password`, `read`, `default`, `ok`, `cancel`, `flat`, `left`,
    /// `center`, `right`, `push`, `group`, `vsbar`, `hsbar`, `autohs`, `autovs`, `limit N`, `size`, `drop`,
    /// `sort`, `extsel`, `check`, `radio`, `return`…).
    pub styles: Vec<String>,
    /// Onglet auquel appartient le contrôle (`tab N` à la fin de la ligne), s'il y en a un.
    pub tab: Option<u32>,
}

impl Control {
    pub fn has_style(&self, s: &str) -> bool {
        self.styles.iter().any(|x| x.eq_ignore_ascii_case(s))
    }
}

/// Un dialogue ouvert (`/dialog -m nom table`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dialog {
    pub name: String,
    pub table: String,
    pub title: String,
    /// x, y (-1 = centré), largeur, hauteur.
    pub rect: (i32, i32, i32, i32),
    pub dbu: bool,
    pub modal: bool,
    pub controls: Vec<Control>,
    /// Onglet affiché, pour chaque contrôle `tab` (id de l'onglet).
    pub active_tab: Option<u32>,
}

/// Ce que le client doit fournir au moteur.
pub trait Host {
    /// Exécute une commande intégrée du client (`msg`, `join`, `echo`, `window`, `aline`…). `name` est en
    /// minuscules, sans `/` ni `!` ; `args` est la suite déjà évaluée (switches compris, ex. `-a texte`).
    /// Rend une erreur à afficher (`/msg: insufficient parameters`) si la commande est inconnue ou mal formée.
    fn command(&mut self, ctx: &Ctx, name: &str, args: &str) -> Result<(), String>;

    /// Identifiant qui dépend de l'état du client (`$me`, `$server`, `$network`, `$chan(N)`, `$nick(#,N)`,
    /// `$ial`, `$comchan`, `$active`, `$window`, `$line`, `$status`, `$away`, `$idle`, `$lag`…). `name` sans `$`,
    /// en minuscules ; `prop` est la propriété après le point (`$nick(#,1).pnick`). `None` si inconnu.
    fn identifier(&mut self, ctx: &Ctx, name: &str, args: &[String], prop: Option<&str>) -> Option<String>;

    /// Modes de préfixe d'un pseudo sur un canal (`"o"`, `"v"`, `""`), ou `None` s'il n'y est pas.
    /// Sert aux opérateurs `ison`, `isop`, `ishop`, `isvoice`, `isreg`.
    fn member_modes(&mut self, ctx: &Ctx, chan: &str, nick: &str) -> Option<String>;

    /// Vrai si le nom est un canal sur la connexion courante (opérateur `ischan`, CHANTYPES).
    fn is_channel(&mut self, ctx: &Ctx, name: &str) -> bool;

    /// Écrit une erreur de script là où l'utilisateur la verra (fenêtre courante ou Status).
    fn script_error(&mut self, ctx: &Ctx, text: &str);

    /// Heure actuelle en millisecondes depuis 1970 (sert à `$ctime`, `$ticks`, aux timers ; les tests la fixent).
    fn now_ms(&self) -> i64;
}

/// Le moteur : scripts chargés, variables, tables, timers, dialogues.
pub struct Engine {
    root: PathBuf,
}

impl Engine {
    /// `root` : dossier des scripts ; toute lecture ou écriture de fichier y est confinée.
    pub fn new(root: PathBuf) -> Engine {
        Engine { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}
