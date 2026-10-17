//! Lecture des changements de modes (`MODE #canal +ov-b Gui Pat *!*@x`).

use crate::isupport::ISupport;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeChange {
    pub add: bool,
    pub mode: char,
    pub arg: Option<String>,
}

/// Lit les modes d'un canal : la chaîne de modes puis ses paramètres, selon CHANMODES et PREFIX.
pub fn parse_channel(modes: &str, args: &[String], is: &ISupport) -> Vec<ModeChange> {
    let mut out = Vec::new();
    let mut add = true;
    let mut args = args.iter();
    for c in modes.chars() {
        match c {
            '+' => add = true,
            '-' => add = false,
            m => {
                let takes = match is.chanmode_type(m) {
                    Some(0) | Some(1) => true,
                    Some(2) => add,
                    Some(_) => false,
                    // Mode inconnu : on suppose sans paramètre.
                    None => false,
                };
                let arg = if takes { args.next().cloned() } else { None };
                out.push(ModeChange { add, mode: m, arg });
            }
        }
    }
    out
}

/// Lit les modes d'un utilisateur (`+iwx`), sans paramètres.
pub fn parse_user(modes: &str) -> Vec<ModeChange> {
    let mut out = Vec::new();
    let mut add = true;
    for c in modes.chars() {
        match c {
            '+' => add = true,
            '-' => add = false,
            m => out.push(ModeChange { add, mode: m, arg: None }),
        }
    }
    out
}

/// Regroupe des changements en lignes MODE d'au plus `per_line` modes à paramètre.
pub fn build(changes: &[ModeChange], per_line: usize) -> Vec<(String, Vec<String>)> {
    let per_line = per_line.max(1);
    let mut lines = Vec::new();
    for chunk in changes.chunks(per_line) {
        let mut s = String::new();
        let mut cur: Option<bool> = None;
        let mut args = Vec::new();
        for ch in chunk {
            if cur != Some(ch.add) {
                s.push(if ch.add { '+' } else { '-' });
                cur = Some(ch.add);
            }
            s.push(ch.mode);
            if let Some(a) = &ch.arg {
                args.push(a.clone());
            }
        }
        lines.push((s, args));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn channel() {
        let is = ISupport::default();
        let ch = parse_channel("+ov-b+lk", &a(&["Gui", "Pat", "*!*@x", "10", "clé"]), &is);
        assert_eq!(ch.len(), 5);
        assert_eq!(ch[0], ModeChange { add: true, mode: 'o', arg: Some("Gui".into()) });
        assert_eq!(ch[2], ModeChange { add: false, mode: 'b', arg: Some("*!*@x".into()) });
        assert_eq!(ch[3].arg.as_deref(), Some("10"));
        assert_eq!(ch[4].arg.as_deref(), Some("clé"));
        let ch = parse_channel("-l+n", &[], &is);
        assert_eq!(ch[0].arg, None);
    }

    #[test]
    fn build_lines() {
        let ch: Vec<ModeChange> = ["a", "b", "c", "d"]
            .iter()
            .map(|n| ModeChange { add: true, mode: 'o', arg: Some(n.to_string()) })
            .collect();
        let l = build(&ch, 3);
        assert_eq!(l[0], ("+ooo".to_string(), a(&["a", "b", "c"])));
        assert_eq!(l[1], ("+o".to_string(), a(&["d"])));
    }
}
