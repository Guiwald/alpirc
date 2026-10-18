//! Les messages du moteur de scripts ([`Msg`], [`ScriptError`]) rédigés dans la langue de l'interface. Noms de
//! commandes, d'identifiants, de fichiers, de tables et de variables restent tels quels ; les précisions venues
//! d'une bibliothèque (expressions régulières) aussi.

use alpirc_script::{Msg, ParseProblem, ScriptError};

use crate::i18n::{Key, Lang, trf};

/// Le message tel qu'il s'affiche, au format de mIRC : `* ` puis le texte.
pub fn msg_text(lang: Lang, m: &Msg) -> String {
    format!("* {}", body(lang, m))
}

/// Le message sans l'astérisque du début.
pub fn body(lang: Lang, m: &Msg) -> String {
    let t = |k: Key, args: &[(&str, &str)]| trf(lang, k, args);
    let cf = |k: Key, cmd: &str, file: &str| trf(lang, k, &[("cmd", cmd), ("file", file)]);
    let cn = |k: Key, cmd: &str, name: &str| trf(lang, k, &[("cmd", cmd), ("name", name)]);
    match m {
        Msg::InsufficientParams { cmd } => t(Key::InsufficientParams, &[("cmd", cmd)]),
        Msg::InvalidParams { cmd } => t(Key::ScrInvalidParams, &[("cmd", cmd)]),
        Msg::CannotOpen { cmd, file } => cf(Key::ScrCannotOpen, cmd, file),
        Msg::CannotWrite { cmd, file } => cf(Key::ScrCannotWrite, cmd, file),
        Msg::CannotRemove { cmd, file } => cf(Key::ScrCannotRemove, cmd, file),
        Msg::CannotRename { cmd, file } => cf(Key::ScrCannotRename, cmd, file),
        Msg::CannotCopy { cmd, file } => cf(Key::ScrCannotCopy, cmd, file),
        Msg::CannotCreate { cmd, file } => cf(Key::ScrCannotCreate, cmd, file),
        Msg::FileExists { cmd, file } => cf(Key::ScrFileExists, cmd, file),
        Msg::NoSuchDialog { cmd, name } => cn(Key::ScrNoSuchDialog, cmd, name),
        Msg::NoSuchTable { cmd, name } => cn(Key::ScrNoSuchTable, cmd, name),
        Msg::NoSuchScript { cmd, name } => cn(Key::ScrNoSuchScript, cmd, name),
        Msg::TableExists { cmd, name } => cn(Key::ScrTableExists, cmd, name),
        Msg::NameInUse { cmd, name } => cn(Key::ScrNameInUse, cmd, name),
        Msg::NoAddress { cmd, nick } => t(Key::ScrNoAddress, &[("cmd", cmd), ("nick", nick)]),
        Msg::LabelNotFound { label } => t(Key::ScrLabelNotFound, &[("label", label)]),
        Msg::CommandSandboxed { cmd } => t(Key::ScrCommandSandboxed, &[("cmd", cmd)]),
        Msg::IdentSandboxed { ident } => t(Key::ScrIdentSandboxed, &[("ident", ident)]),
        Msg::IdentParams { ident, detail: None } => t(Key::ScrIdentParams, &[("ident", ident)]),
        Msg::IdentParams { ident, detail: Some(d) } => t(Key::ScrIdentParamsDetail, &[("ident", ident), ("detail", d)]),
        Msg::IdentOnlyMime { ident } => t(Key::ScrIdentOnlyMime, &[("ident", ident)]),
        Msg::TooDeep => t(Key::ScrTooDeep, &[]),
        Msg::NoSuchMenuItem => t(Key::ScrNoSuchMenuItem, &[]),
        Msg::Sandbox { path } => t(Key::ScrSandbox, &[("path", path)]),
        Msg::Parse { file, line, problem } => parse(lang, file, *line, problem),
        // Déjà rédigée par le client (dans sa langue).
        Msg::Host(s) => s.strip_prefix("* ").unwrap_or(s).to_string(),
        Msg::At { msg, line, script } => {
            t(Key::ScrAt, &[("msg", &body(lang, msg)), ("line", &line.to_string()), ("script", script)])
        }
        Msg::MadeTable { name, size } => t(Key::ScrMadeTable, &[("name", name), ("size", &size.to_string())]),
        Msg::AddedItem { item, table } => t(Key::ScrAddedItem, &[("item", item), ("table", table)]),
        Msg::TimerNotActive { name } => t(Key::ScrTimerNotActive, &[("name", name)]),
        Msg::TimerHalted { name } => t(Key::ScrTimerHalted, &[("name", name)]),
        Msg::TimerActivated { name } => t(Key::ScrTimerActivated, &[("name", name)]),
        Msg::AllTimersHalted => t(Key::ScrAllTimersHalted, &[]),
        Msg::NoActiveTimers => t(Key::ScrNoActiveTimers, &[]),
        Msg::ActiveTimers => t(Key::ScrActiveTimers, &[]),
        Msg::TimerLine { name, delay, command, left: None } => {
            t(Key::ScrTimerLine, &[("name", name), ("delay", delay), ("command", command)])
        }
        Msg::TimerLine { name, delay, command, left: Some(n) } => t(
            Key::ScrTimerLineLeft,
            &[("name", name), ("delay", delay), ("command", command), ("left", &n.to_string())],
        ),
        Msg::VarSet { name, value } => t(Key::ScrVarSet, &[("name", name), ("value", value)]),
        Msg::VarUnsetAll => t(Key::ScrVarUnsetAll, &[]),
        Msg::VarUnset { name } => t(Key::ScrVarUnset, &[("name", name)]),
        Msg::VarInc { name, value } => t(Key::ScrVarInc, &[("name", name), ("value", value)]),
        Msg::VarDec { name, value } => t(Key::ScrVarDec, &[("name", name), ("value", value)]),
    }
}

fn parse(lang: Lang, file: &str, line: usize, problem: &ParseProblem) -> String {
    let p = match problem {
        ParseProblem::MissingClose => trf(lang, Key::ScrMissingClose, &[]),
        ParseProblem::MissingOpen => trf(lang, Key::ScrMissingOpen, &[]),
    };
    trf(lang, Key::ScrParse, &[("file", file), ("line", &line.to_string()), ("problem", &p)])
}

/// Une erreur rendue par le moteur, à afficher seule sur une ligne (`* …`).
pub fn error_line(lang: Lang, e: &ScriptError) -> String {
    msg_text(lang, &e.msg())
}

/// Une erreur rendue par le moteur, à glisser dans une phrase (« Impossible de charger le script x : … »), sans
/// astérisque.
pub fn error_text(lang: Lang, e: &ScriptError) -> String {
    body(lang, &e.msg())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(x: &str) -> String {
        x.to_string()
    }

    fn samples() -> Vec<Msg> {
        let (c, f, n) = (s("hmake"), s("a.txt"), s("t"));
        vec![
            Msg::InsufficientParams { cmd: c.clone() },
            Msg::InvalidParams { cmd: c.clone() },
            Msg::CannotOpen { cmd: c.clone(), file: f.clone() },
            Msg::CannotWrite { cmd: c.clone(), file: f.clone() },
            Msg::CannotRemove { cmd: c.clone(), file: f.clone() },
            Msg::CannotRename { cmd: c.clone(), file: f.clone() },
            Msg::CannotCopy { cmd: c.clone(), file: f.clone() },
            Msg::CannotCreate { cmd: c.clone(), file: f.clone() },
            Msg::FileExists { cmd: c.clone(), file: f.clone() },
            Msg::NoSuchDialog { cmd: c.clone(), name: n.clone() },
            Msg::NoSuchTable { cmd: c.clone(), name: n.clone() },
            Msg::NoSuchScript { cmd: c.clone(), name: n.clone() },
            Msg::TableExists { cmd: c.clone(), name: n.clone() },
            Msg::NameInUse { cmd: c.clone(), name: n.clone() },
            Msg::NoAddress { cmd: s("guser"), nick: s("Bob") },
            Msg::LabelNotFound { label: s("x") },
            Msg::CommandSandboxed { cmd: s("run") },
            Msg::IdentSandboxed { ident: s("dll") },
            Msg::IdentParams { ident: s("gettok"), detail: None },
            Msg::IdentParams { ident: s("regex"), detail: Some(s("unclosed group")) },
            Msg::IdentOnlyMime { ident: s("encode") },
            Msg::TooDeep,
            Msg::NoSuchMenuItem,
            Msg::Sandbox { path: s("../x") },
            Msg::Parse { file: f.clone(), line: 3, problem: ParseProblem::MissingClose },
            Msg::Parse { file: f.clone(), line: 3, problem: ParseProblem::MissingOpen },
            Msg::Host(s("* /msg: x")),
            Msg::At { msg: Box::new(Msg::TooDeep), line: 3, script: s("x.mrc") },
            Msg::MadeTable { name: n.clone(), size: 100 },
            Msg::AddedItem { item: s("i"), table: n.clone() },
            Msg::TimerNotActive { name: s("1") },
            Msg::TimerHalted { name: s("1") },
            Msg::TimerActivated { name: s("1") },
            Msg::AllTimersHalted,
            Msg::NoActiveTimers,
            Msg::ActiveTimers,
            Msg::TimerLine { name: s("1"), delay: s("30s"), command: s("echo -a x"), left: None },
            Msg::TimerLine { name: s("1"), delay: s("30s"), command: s("echo -a x"), left: Some(2) },
            Msg::VarSet { name: s("%x"), value: s("5") },
            Msg::VarUnsetAll,
            Msg::VarUnset { name: s("%x") },
            Msg::VarInc { name: s("%x"), value: s("6") },
            Msg::VarDec { name: s("%x"), value: s("4") },
        ]
    }

    #[test]
    fn english_is_the_engine_text() {
        for m in samples() {
            assert_eq!(msg_text(Lang::En, &m), m.to_string(), "{m:?}");
        }
    }

    #[test]
    fn other_languages() {
        let m = Msg::At { msg: Box::new(Msg::InsufficientParams { cmd: s("fail") }), line: 3, script: s("test.mrc") };
        assert_eq!(msg_text(Lang::Fr, &m), "* /fail : paramètres insuffisants (ligne 3, test.mrc)");
        assert_eq!(msg_text(Lang::De, &m), "* /fail: zu wenige Parameter (Zeile 3, test.mrc)");
        let t = Msg::NoSuchTable { cmd: s("hadd"), name: s("nope") };
        assert_eq!(msg_text(Lang::It, &t), "* /hadd: nessuna tabella «nope»");
        assert_eq!(msg_text(Lang::Ga, &Msg::TimerActivated { name: s("1") }), "* Gníomhachtaíodh an lasc ama 1");
        let e = ScriptError::Sandbox(s("../x.mrc"));
        assert_eq!(error_text(Lang::Va, &e), "accés denegat fora de la carpeta dels scripts: ../x.mrc");
        for l in Lang::ALL {
            for m in samples() {
                let x = msg_text(l, &m);
                // Les marques sont toutes remplacées (« missing '{' » n'en est pas une).
                assert!(!x.contains("{cmd}") && !x.contains("{name}") && !x.contains("{msg}"), "{} : {x}", l.code());
                if l != Lang::En && !matches!(m, Msg::Host(_)) {
                    assert_ne!(x, m.to_string(), "{} : {m:?}", l.code());
                }
            }
        }
    }
}
