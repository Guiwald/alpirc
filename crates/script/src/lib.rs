//! Moteur de scripts mSL d'AlpIRC (ADR 0007).
//!
//! Le moteur ne connaît ni le réseau ni l'interface : tout ce qui touche à l'état du client (envoyer un message,
//! écrire dans une fenêtre, lire la liste des pseudos d'un canal…) passe par le trait [`Host`], que le client
//! (`alpirc-core`) implémente. Le moteur gère lui-même le langage : lecture des fichiers, alias, popups, remotes
//! (`on …`, `ctcp …`, `raw …`), groupes, variables, identifiants « purs » (`$calc`, `$len`, `$gettok`…), contrôle
//! de flux, timers, tables de hachage, fichiers du bac à sable, niveaux d'utilisateurs et dialogues personnalisés.
//!
//! Bac à sable : pas de `/run`, pas de `$dll`/`$com`, fichiers limités au dossier racine donné à [`Engine::new`].
//!
//! Organisation :
//! - `source` lit les fichiers (remotes, alias, popups, `.ini` de mIRC) ; `ast` découpe les corps en instructions ;
//! - `eval` évalue le texte (identifiants, variables, `[ ]`, `$+`) ; `cond` les conditions ; `exec` exécute
//!   (alias, aiguillage des commandes, garde-fous) ; `idents` porte les identifiants du moteur, aidé de `strings`,
//!   `calc`, `timefmt`, `regexid` et `wild` (jokers) ;
//! - `vars`, `hash`, `timers`, `files` (bac à sable), `users`, `events`, `popups`, `dialogs` et `scripts`
//!   (chargement, groupes) portent chacun leur part de l'état et leurs commandes.
//!
//! Le client crée un [`Engine`], charge les scripts ([`Engine::load_file`], [`Engine::restore_file`]…), lui passe
//! les lignes tapées ([`Engine::run_line`]), les événements ([`Engine::fire`]), les menus ([`Engine::popup`]), les
//! actions dans les dialogues ([`Engine::dialog_event`]) et l'appelle régulièrement ([`Engine::tick`]).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

mod ast;
mod calc;
mod cond;
mod dialogs;
mod eval;
mod events;
mod exec;
mod files;
mod hash;
mod idents;
mod msg;
mod popups;
mod regexid;
mod scripts;
mod source;
mod strings;
mod timefmt;
mod timers;
mod users;
mod vars;
mod wild;

pub use msg::{Msg, ParseProblem};
pub use popups::PopupToken;

/// Erreur d'un script (lecture ou exécution), décrite par ses données. `Display` l'écrit en anglais comme mIRC
/// (`* /msg: insufficient parameters`) ; le client la rédige dans sa langue s'il le veut (voir [`Msg`]).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScriptError {
    #[error("{0}")]
    Message(Msg),
    #[error("{file}:{line}: {problem}")]
    Parse { file: String, line: usize, problem: ParseProblem },
    #[error("access denied outside the scripts folder: {0}")]
    Sandbox(String),
}

impl ScriptError {
    /// Le message à montrer à l'utilisateur, décrit par ses données (l'hôte le rédige avec [`Host::message_text`]).
    pub fn msg(&self) -> Msg {
        match self {
            ScriptError::Message(m) => m.clone(),
            ScriptError::Parse { file, line, problem } => {
                Msg::Parse { file: file.clone(), line: *line, problem: problem.clone() }
            }
            ScriptError::Sandbox(p) => Msg::Sandbox { path: p.clone() },
        }
    }
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

    /// Écrit une erreur de script là où l'utilisateur la verra (fenêtre courante ou Status). `text` est déjà
    /// rédigé par [`Host::message_text`].
    fn script_error(&mut self, ctx: &Ctx, text: &str);

    /// Rédige un message du moteur (erreur, information comme `* Timer 1 activated`), astérisque du début compris.
    /// Par défaut en anglais, comme mIRC ; le client le rédige dans la langue de son interface.
    fn message_text(&self, msg: &Msg) -> String {
        msg.to_string()
    }

    /// Heure actuelle en millisecondes depuis 1970 (sert à `$ctime`, `$ticks`, aux timers ; les tests la fixent).
    fn now_ms(&self) -> i64;

    /// Variante silencieuse d'une commande (préfixe `.` : `/.msg`, `/.notice`…) : le client n'affiche pas l'écho
    /// habituel. Par défaut, identique à [`Host::command`].
    fn command_quiet(&mut self, ctx: &Ctx, name: &str, args: &str) -> Result<(), String> {
        self.command(ctx, name, args)
    }

    /// Décalage de l'heure locale sur UTC, en secondes, à l'instant donné (secondes depuis 1970) : sert à
    /// `$asctime`, `$time`, `$date`, `$ctime(texte)`, `$timezone`. Par défaut celui du système (0 s'il est
    /// inconnu) ; les tests le fixent.
    fn utc_offset(&self, unix_secs: i64) -> i32 {
        time::OffsetDateTime::from_unix_timestamp(unix_secs)
            .ok()
            .and_then(|t| time::UtcOffset::local_offset_at(t).ok())
            .map(|o| o.whole_seconds())
            .unwrap_or(0)
    }

    /// `/load` d'un script qui s'exécute dès son chargement (`on LOAD`, `on START`) : vrai pour l'exécuter tout
    /// de suite ; faux pour le retirer sans rien exécuter (le client demande d'abord à l'utilisateur, ADR 0007,
    /// puis le recharge lui-même). Par défaut : vrai.
    fn confirm_load(&mut self, name: &str, path: &Path) -> bool {
        let _ = (name, path);
        true
    }

    /// `$?` et `$input(…)` : la réponse de l'utilisateur. `None` arrête l'exécution sans bruit : l'hôte pose la
    /// question, puis relance ce qui a été demandé en rendant la réponse au passage suivant (le moteur ne sait pas
    /// attendre). Par défaut : l'identifiant `?` de l'hôte (vide s'il ne le connaît pas).
    fn input(&mut self, ctx: &Ctx, req: &InputRequest) -> Option<String> {
        Some(self.identifier(ctx, "?", std::slice::from_ref(&req.prompt), None).unwrap_or_default())
    }
}

/// Question posée à l'utilisateur par `$?` (`$?="Pseudo :"`, `$?*` mot de passe, `$?!` oui ou non) ou par
/// `$input(question, drapeaux, fenêtre, titre, texte)`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InputRequest {
    pub prompt: String,
    /// Drapeaux de `$input` : `e` champ de saisie, `p` mot de passe, `o` bouton OK, `y` oui/non, `n` oui/non/annuler,
    /// `v` réponses `$ok`, `$yes`, `$no`, `$cancel` ; `i`, `q`, `w`, `h` l'icône. `$?` vaut `e`, `$?*` `ep`, `$?!` `y`.
    pub flags: String,
    pub title: String,
    /// Texte de départ du champ.
    pub default: String,
}

/// Commandes propres au moteur (en plus de celles de l'hôte), pour la complétion par Tab.
pub const ENGINE_COMMANDS: &[&str] = &[
    "alias", "auser", "break", "continue", "copy", "dec", "did", "dialog", "disable", "enable", "flushini", "goto",
    "guser", "hadd", "halt", "haltdef", "hdec", "hdel", "hfree", "hinc", "hload", "hmake", "hsave", "inc", "load",
    "mkdir", "noop", "reload", "remini", "remove", "rename", "return", "rlevel", "rmdir", "ruser", "scid", "scon",
    "set", "signal", "timer", "timers", "tokenize", "unload", "unset", "unsetall", "var", "write", "writeini",
];

/// Le moteur : scripts chargés, variables, tables, timers, dialogues.
///
/// Toutes les méthodes qui exécutent du code prennent l'hôte ([`Host`]) et le contexte ([`Ctx`]) ; les erreurs
/// d'exécution des appels sans valeur de retour ([`Engine::fire`], [`Engine::tick`], [`Engine::dialog_event`]…)
/// sont signalées par [`Host::script_error`], celles de [`Engine::run_line`], [`Engine::command`] et
/// [`Engine::popup_run`] sont rendues à l'appelant.
pub struct Engine {
    root: PathBuf,
    pub(crate) scripts: Vec<scripts::Script>,
    pub(crate) loaded: Vec<LoadedScript>,
    /// Alias créés par `/alias nom commandes`.
    pub(crate) dyn_aliases: Vec<Arc<source::Alias>>,
    /// Nom d'alias → indices (dans `scripts`) des scripts qui le définissent, dans l'ordre de chargement.
    pub(crate) alias_index: HashMap<String, Vec<usize>>,
    pub(crate) vars: vars::Vars,
    pub(crate) tables: hash::Tables,
    pub(crate) timers: timers::Timers,
    pub(crate) users: users::Users,
    pub(crate) dialogs: Vec<Dialog>,
    pub(crate) dmeta: Vec<dialogs::Meta>,
    pub(crate) dialogs_version: u64,
    pub(crate) popup_actions: Vec<popups::Action>,
    pub(crate) pending: Vec<events::Pending>,
    pub(crate) rt: exec::Runtime,
}

impl Engine {
    /// `root` : dossier des scripts ; toute lecture ou écriture de fichier y est confinée.
    pub fn new(root: PathBuf) -> Engine {
        Engine {
            root,
            scripts: Vec::new(),
            loaded: Vec::new(),
            dyn_aliases: Vec::new(),
            alias_index: HashMap::new(),
            vars: vars::Vars::default(),
            tables: hash::Tables::default(),
            timers: timers::Timers::default(),
            users: users::Users::default(),
            dialogs: Vec::new(),
            dmeta: Vec::new(),
            dialogs_version: 0,
            popup_actions: Vec::new(),
            pending: Vec::new(),
            rt: exec::Runtime::default(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Limites de sûreté : nombre total de tours de boucle (`while`, `goto`) par exécution, et profondeur d'appels
    /// d'alias. Au-delà, le script s'arrête avec `* Error: script too deep / infinite loop`.
    /// Par défaut : 1 000 000 et 100.
    pub fn set_limits(&mut self, max_iterations: u64, max_depth: usize) {
        self.rt.max_iterations = max_iterations;
        self.rt.max_depth = max_depth.max(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le client doit pouvoir confier le moteur à un autre fil.
    #[test]
    fn engine_is_send() {
        fn send<T: Send>() {}
        send::<Engine>();
        send::<ScriptError>();
    }
}
