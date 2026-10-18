//! Messages du moteur pour l'utilisateur : erreurs (`* /hmake: insufficient parameters`) et informations
//! (`* Timer 1 activated`), décrits par leurs données. [`fmt::Display`] les écrit en anglais, comme mIRC : le moteur
//! reste utilisable seul. L'hôte peut les rédiger dans la langue de l'interface ([`crate::Host::message_text`]) ;
//! noms de commandes, d'identifiants, de fichiers et de tables restent tels quels.

use std::fmt;

/// Problème de lecture d'un fichier de script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseProblem {
    /// `}` manquant.
    MissingClose,
    /// `{` manquant.
    MissingOpen,
}

impl fmt::Display for ParseProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ParseProblem::MissingClose => "missing '}'",
            ParseProblem::MissingOpen => "missing '{'",
        })
    }
}

/// Un message du moteur. `cmd` : nom de la commande sans `/` ; `ident` : nom de l'identifiant sans `$`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Msg {
    // --- erreurs ---
    InsufficientParams { cmd: String },
    InvalidParams { cmd: String },
    /// Fichier impossible à ouvrir (lecture, ou écriture pour `/write`).
    CannotOpen { cmd: String, file: String },
    CannotWrite { cmd: String, file: String },
    CannotRemove { cmd: String, file: String },
    CannotRename { cmd: String, file: String },
    CannotCopy { cmd: String, file: String },
    /// Dossier impossible à créer.
    CannotCreate { cmd: String, file: String },
    /// Le fichier de destination existe déjà.
    FileExists { cmd: String, file: String },
    NoSuchDialog { cmd: String, name: String },
    NoSuchTable { cmd: String, name: String },
    NoSuchScript { cmd: String, name: String },
    TableExists { cmd: String, name: String },
    /// Nom de dialogue déjà pris.
    NameInUse { cmd: String, name: String },
    /// Adresse d'un pseudo inconnue (pas dans l'IAL).
    NoAddress { cmd: String, nick: String },
    /// Étiquette de `/goto` introuvable.
    LabelNotFound { label: String },
    /// Commande refusée par le bac à sable (`/run`, `/dll`, sockets…).
    CommandSandboxed { cmd: String },
    /// Identifiant refusé par le bac à sable (`$dll`, `$com`…).
    IdentSandboxed { ident: String },
    /// Paramètres d'un identifiant invalides ; `detail` : précision de la bibliothèque (expression régulière), en
    /// anglais.
    IdentParams { ident: String, detail: Option<String> },
    /// `$encode`/`$decode` : seule l'option `m` est prise en charge.
    IdentOnlyMime { ident: String },
    /// Garde-fous : trop de profondeur ou boucle infinie.
    TooDeep,
    /// Entrée de popup inconnue.
    NoSuchMenuItem,
    /// Accès à un fichier hors du dossier des scripts (chemin demandé).
    Sandbox { path: String },
    /// Erreur de lecture d'un script chargé par une commande.
    Parse { file: String, line: usize, problem: ParseProblem },
    /// Erreur rendue par l'hôte ([`crate::Host::command`]), déjà rédigée par lui.
    Host(String),
    /// Le message, et où il est survenu (ligne et fichier du script).
    At { msg: Box<Msg>, line: usize, script: String },
    // --- informations ---
    MadeTable { name: String, size: usize },
    AddedItem { item: String, table: String },
    TimerNotActive { name: String },
    TimerHalted { name: String },
    TimerActivated { name: String },
    AllTimersHalted,
    NoActiveTimers,
    ActiveTimers,
    /// Un timer de la liste : `delay` déjà écrit (`30s`, `500ms`) ; `left` : répétitions restantes (`None` :
    /// sans fin).
    TimerLine { name: String, delay: String, command: String, left: Option<u64> },
    VarSet { name: String, value: String },
    VarUnsetAll,
    VarUnset { name: String },
    VarInc { name: String, value: String },
    VarDec { name: String, value: String },
}

impl Msg {
    /// Le message sans son lieu (`At`).
    pub fn inner(&self) -> &Msg {
        match self {
            Msg::At { msg, .. } => msg.inner(),
            m => m,
        }
    }

    /// Le texte anglais, sans l'astérisque du début.
    pub fn body(&self) -> String {
        match self {
            Msg::InsufficientParams { cmd } => format!("/{cmd}: insufficient parameters"),
            Msg::InvalidParams { cmd } => format!("/{cmd}: invalid parameters"),
            Msg::CannotOpen { cmd, file } => format!("/{cmd}: unable to open '{file}'"),
            Msg::CannotWrite { cmd, file } => format!("/{cmd}: unable to write '{file}'"),
            Msg::CannotRemove { cmd, file } => format!("/{cmd}: unable to remove '{file}'"),
            Msg::CannotRename { cmd, file } => format!("/{cmd}: unable to rename '{file}'"),
            Msg::CannotCopy { cmd, file } => format!("/{cmd}: unable to copy '{file}'"),
            Msg::CannotCreate { cmd, file } => format!("/{cmd}: unable to create '{file}'"),
            Msg::FileExists { cmd, file } => format!("/{cmd}: '{file}' exists"),
            Msg::NoSuchDialog { cmd, name } => format!("/{cmd}: no such dialog '{name}'"),
            Msg::NoSuchTable { cmd, name } => format!("/{cmd}: no such table '{name}'"),
            Msg::NoSuchScript { cmd, name } => format!("/{cmd}: no such script '{name}'"),
            Msg::TableExists { cmd, name } => format!("/{cmd}: table '{name}' exists"),
            Msg::NameInUse { cmd, name } => format!("/{cmd}: '{name}' name in use"),
            Msg::NoAddress { cmd, nick } => format!("/{cmd}: unable to find address of {nick}"),
            Msg::LabelNotFound { label } => format!("/goto: '{label}' not found"),
            Msg::CommandSandboxed { cmd } => format!("/{cmd}: not allowed by the script sandbox"),
            Msg::IdentSandboxed { ident } => format!("${ident}: not allowed by the script sandbox"),
            Msg::IdentParams { ident, detail: None } => format!("Invalid parameters: ${ident}"),
            Msg::IdentParams { ident, detail: Some(d) } => format!("Invalid parameters: ${ident} ({d})"),
            Msg::IdentOnlyMime { ident } => format!("Invalid parameters: ${ident} (only 'm' is supported)"),
            Msg::TooDeep => "Error: script too deep / infinite loop".into(),
            Msg::NoSuchMenuItem => "popup: no such menu item".into(),
            Msg::Sandbox { path } => format!("access denied outside the scripts folder: {path}"),
            Msg::Parse { file, line, problem } => format!("{file}:{line}: {problem}"),
            Msg::Host(s) => s.strip_prefix("* ").unwrap_or(s).to_string(),
            Msg::At { msg, line, script } => format!("{} (line {line}, {script})", msg.body()),
            Msg::MadeTable { name, size } => format!("Made hash table '{name}' ({size})"),
            Msg::AddedItem { item, table } => format!("Added item '{item}' to hash table '{table}'"),
            Msg::TimerNotActive { name } => format!("Timer {name} not active"),
            Msg::TimerHalted { name } => format!("Timer {name} halted"),
            Msg::TimerActivated { name } => format!("Timer {name} activated"),
            Msg::AllTimersHalted => "All timers halted".into(),
            Msg::NoActiveTimers => "No active timers".into(),
            Msg::ActiveTimers => "Active timers:".into(),
            Msg::TimerLine { name, delay, command, left: None } => format!("Timer {name} {delay} delay {command}"),
            Msg::TimerLine { name, delay, command, left: Some(n) } => {
                format!("Timer {name} {delay} delay {command} ({n} left)")
            }
            Msg::VarSet { name, value } => format!("Set {name} to {value}"),
            Msg::VarUnsetAll => "Unset all variables".into(),
            Msg::VarUnset { name } => format!("Unset {name}"),
            Msg::VarInc { name, value } => format!("Inc {name} to {value}"),
            Msg::VarDec { name, value } => format!("Dec {name} to {value}"),
        }
    }
}

/// Le texte affiché, au format de mIRC : `* ` puis le message.
impl fmt::Display for Msg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "* {}", self.body())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english() {
        let fail = Msg::InsufficientParams { cmd: "fail".into() };
        let m = Msg::At { msg: Box::new(fail), line: 3, script: "test.mrc".into() };
        assert_eq!(m.to_string(), "* /fail: insufficient parameters (line 3, test.mrc)");
        assert_eq!(m.inner(), &Msg::InsufficientParams { cmd: "fail".into() });
        assert_eq!(Msg::Host("* /msg: insufficient parameters".into()).to_string(), "* /msg: insufficient parameters");
        assert_eq!(Msg::Host("/msg: x".into()).to_string(), "* /msg: x");
        let t = Msg::TimerLine { name: "1".into(), delay: "30s".into(), command: "echo hi".into(), left: Some(2) };
        assert_eq!(t.to_string(), "* Timer 1 30s delay echo hi (2 left)");
        let p = Msg::Parse { file: "a.mrc".into(), line: 4, problem: ParseProblem::MissingClose };
        assert_eq!(p.to_string(), "* a.mrc:4: missing '}'");
    }
}
