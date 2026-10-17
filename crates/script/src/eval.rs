//! Évaluation du texte : `$identifiants(…)` et leurs `.propriétés`, `%variables`, `$1-`, crochets d'évaluation
//! `[ ]`, concaténation `$+`.
//!
//! Comme dans mIRC, un identifiant ou une variable n'est reconnu qu'en début de mot (après une espace, ou dans
//! les paramètres d'un identifiant après `(` ou `,`) : `abc$me` reste tel quel, il faut écrire `abc $+ $me`.
//! Les crochets sont évalués d'abord, puis la ligne obtenue est évaluée à son tour (d'où la double évaluation de
//! `[ %x ]`) ; un `$+` collé au bord d'un crochet (`%var. [ $+ [ $nick ] ]`) colle le résultat au mot voisin avant
//! l'évaluation de la ligne, ce qui permet de fabriquer des noms de variables.

use crate::ast::matching_paren;
use crate::exec::{Abort, R};
use crate::{Ctx, Engine, Host};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Ligne de commande : les mots sont séparés par des espaces.
    Top,
    /// Paramètre d'identifiant : `(` ouvre aussi un mot, `,` et `)` terminent un nom de variable.
    Arg,
}

/// Découpe les paramètres d'un identifiant aux virgules hors parenthèses.
pub fn split_args(s: &str) -> Vec<&str> {
    if s.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                out.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Fin (exclue) d'un identifiant commençant en `st` (après le `$`) : nom, `(…)`, `.prop`. Sert à `$!`.
fn ident_extent(text: &str, st: usize) -> usize {
    let b = text.as_bytes();
    let n = b.len();
    let mut j = st;
    if j < n && b[j] == b'$' {
        j += 1;
    }
    if j < n && (b[j] == b'+' || b[j] == b'?' || b[j] == b'*') {
        j += 1;
    } else {
        while j < n {
            let c = text[j..].chars().next().unwrap();
            if !is_name_char(c) {
                break;
            }
            j += c.len_utf8();
        }
    }
    // `$!1-`, `$!2-3`
    if j > st && b[st].is_ascii_digit() && j < n && b[j] == b'-' {
        j += 1;
        while j < n && b[j].is_ascii_digit() {
            j += 1;
        }
    }
    if j < n
        && b[j] == b'('
        && let Some(e) = matching_paren(&text[j..])
    {
        j += e + 1;
    }
    if j + 1 < n && b[j] == b'.' && b[j + 1].is_ascii_alphabetic() {
        j += 1;
        while j < n && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
            j += 1;
        }
    }
    j
}

/// Fin d'un nom de variable commençant en `st` (sur le `%`).
fn var_end(text: &str, st: usize, mode: Mode) -> usize {
    for (k, c) in text[st + 1..].char_indices() {
        let stop = c == ' ' || (mode == Mode::Arg && (c == ',' || c == ')'));
        if stop {
            return st + 1 + k;
        }
    }
    text.len()
}

enum Node {
    Word(String),
    Group(Vec<Node>),
}

/// Vrai si le texte contient un crochet d'évaluation (`[` isolé).
pub fn has_brackets(text: &str) -> bool {
    text.contains('[') && text.split(' ').any(|w| w == "[")
}

impl Engine {
    /// Évalue une ligne (crochets, puis identifiants et variables).
    pub(crate) fn eval_text(&mut self, h: &mut dyn Host, ctx: &Ctx, text: &str) -> R<String> {
        if has_brackets(text) {
            let t = self.resolve_brackets(h, ctx, text)?;
            self.eval_plain(h, ctx, &t, Mode::Top)
        } else {
            self.eval_plain(h, ctx, text, Mode::Top)
        }
    }

    /// Évalue un paramètre d'identifiant.
    pub(crate) fn eval_arg(&mut self, h: &mut dyn Host, ctx: &Ctx, text: &str) -> R<String> {
        if has_brackets(text) {
            let t = self.resolve_brackets(h, ctx, text)?;
            self.eval_plain(h, ctx, &t, Mode::Arg)
        } else {
            self.eval_plain(h, ctx, text, Mode::Arg)
        }
    }

    /// Remplace chaque `[ … ]` par son évaluation (les plus intérieurs d'abord) ; le reste n'est pas évalué.
    pub(crate) fn resolve_brackets(&mut self, h: &mut dyn Host, ctx: &Ctx, text: &str) -> R<String> {
        let words: Vec<&str> = text.split(' ').collect();
        // Paires de crochets qui se correspondent ; les autres restent du texte.
        let mut pair = vec![usize::MAX; words.len()];
        let mut stack = Vec::new();
        for (i, w) in words.iter().enumerate() {
            if *w == "[" {
                stack.push(i);
            } else if *w == "]"
                && let Some(o) = stack.pop()
            {
                pair[o] = i;
                pair[i] = o;
            }
        }
        fn build(words: &[&str], pair: &[usize], i: &mut usize, end: usize) -> Vec<Node> {
            let mut out = Vec::new();
            while *i < end {
                let w = words[*i];
                if w == "[" && pair[*i] != usize::MAX {
                    let close = pair[*i];
                    *i += 1;
                    let inner = build(words, pair, i, close);
                    *i = close + 1;
                    out.push(Node::Group(inner));
                } else {
                    out.push(Node::Word(w.to_string()));
                    *i += 1;
                }
            }
            out
        }
        let mut i = 0;
        let nodes = build(&words, &pair, &mut i, words.len());
        let flat = self.flatten(h, ctx, nodes)?;
        Ok(flat.join(" "))
    }

    fn flatten(&mut self, h: &mut dyn Host, ctx: &Ctx, nodes: Vec<Node>) -> R<Vec<String>> {
        let mut out: Vec<String> = Vec::new();
        let mut glue_next = false;
        for n in nodes {
            match n {
                Node::Word(w) => {
                    if glue_next && let Some(last) = out.last_mut() {
                        last.push_str(&w);
                    } else {
                        out.push(w);
                    }
                    glue_next = false;
                }
                Node::Group(children) => {
                    let mut ws = self.flatten(h, ctx, children)?;
                    while ws.first().is_some_and(|w| w.is_empty()) {
                        ws.remove(0);
                    }
                    while ws.last().is_some_and(|w| w.is_empty()) {
                        ws.pop();
                    }
                    let join_left = ws.first().is_some_and(|w| w == "$+");
                    if join_left {
                        ws.remove(0);
                    }
                    let join_right = ws.last().is_some_and(|w| w == "$+");
                    if join_right {
                        ws.pop();
                    }
                    let content = ws.join(" ");
                    let val = self.eval_plain(h, ctx, &content, Mode::Top)?;
                    if (join_left || glue_next) && let Some(last) = out.last_mut() {
                        last.push_str(&val);
                    } else {
                        out.push(val);
                    }
                    glue_next = join_right;
                }
            }
        }
        Ok(out)
    }

    /// Évalue un texte sans crochets.
    pub(crate) fn eval_plain(&mut self, h: &mut dyn Host, ctx: &Ctx, text: &str, mode: Mode) -> R<String> {
        if !text.contains(['$', '%', '#']) {
            return Ok(text.to_string());
        }
        let b = text.as_bytes();
        let n = b.len();
        let mut out = String::with_capacity(text.len());
        let mut i = 0;
        let mut word_start = true;
        let mut joining = false;
        while i < n {
            let c = b[i];
            if c == b' ' {
                if joining {
                    i += 1;
                    continue;
                }
                out.push(' ');
                i += 1;
                word_start = true;
                continue;
            }
            if word_start && c == b'$' {
                // `$+` seul : colle les mots voisins.
                if i + 1 < n && b[i + 1] == b'+' && (i + 2 == n || b[i + 2] == b' ') {
                    while out.ends_with(' ') {
                        out.pop();
                    }
                    joining = true;
                    i += 2;
                    continue;
                }
                let (end, val) = self.eval_ident_at(h, ctx, text, i, mode)?;
                match val {
                    Some(v) => out.push_str(&v),
                    None => out.push_str(&text[i..end]),
                }
                i = end;
                word_start = false;
                joining = false;
                continue;
            }
            if word_start && c == b'#' {
                // `#` seul : le canal courant ; `#$1` : `#` devant la valeur (sauf si elle en a déjà un).
                let next = b.get(i + 1).copied();
                let alone = match next {
                    None | Some(b' ') => true,
                    Some(b',') | Some(b')') => mode == Mode::Arg,
                    _ => false,
                };
                if alone {
                    match self.event_ident(h, ctx, "chan") {
                        Some(c) => out.push_str(&c),
                        None => out.push('#'),
                    }
                    i += 1;
                    word_start = false;
                    joining = false;
                    continue;
                }
                if next == Some(b'$') {
                    let (end, val) = self.eval_ident_at(h, ctx, text, i + 1, mode)?;
                    match val {
                        Some(v) => {
                            if !v.starts_with('#') {
                                out.push('#');
                            }
                            out.push_str(&v);
                        }
                        None => out.push_str(&text[i..end]),
                    }
                    i = end;
                    word_start = false;
                    joining = false;
                    continue;
                }
            }
            if word_start && c == b'%' && i + 1 < n {
                let nc = text[i + 1..].chars().next().unwrap();
                let ok = nc != ' ' && !(mode == Mode::Arg && (nc == ',' || nc == ')'));
                if ok {
                    let end = var_end(text, i, mode);
                    let v = self.var_get(&text[i..end]);
                    out.push_str(&v);
                    i = end;
                    word_start = false;
                    joining = false;
                    continue;
                }
            }
            let ch = text[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
            word_start = mode == Mode::Arg && ch == '(';
            joining = false;
        }
        Ok(out)
    }

    /// Valeur de `$N`, `$N-`, `$N-M`, `$0`.
    pub(crate) fn param_value(&self, a: usize, range: Option<Option<usize>>) -> String {
        let args = &self.frame().args;
        match range {
            None if a == 0 => args.len().to_string(),
            None => args.get(a - 1).cloned().unwrap_or_default(),
            Some(end) => {
                let st = a.max(1) - 1;
                let en = end.unwrap_or(args.len()).min(args.len());
                if st >= en { String::new() } else { args[st..en].join(" ") }
            }
        }
    }

    /// Lit et évalue l'identifiant qui commence en `i` (sur le `$`). Rend la fin et la valeur (`None` : ce n'était
    /// pas un identifiant, le texte est recopié).
    fn eval_ident_at(&mut self, h: &mut dyn Host, ctx: &Ctx, text: &str, i: usize, mode: Mode) -> R<(usize, Option<String>)> {
        let _ = mode;
        let b = text.as_bytes();
        let n = b.len();
        let mut j = i + 1;
        if j >= n {
            return Ok((j, None));
        }
        let mut halt_null = false;
        let mut builtin_only = false;
        if b[j] == b'$' {
            halt_null = true;
            j += 1;
        }
        if j < n && b[j] == b'~' {
            builtin_only = true;
            j += 1;
        }
        if j >= n {
            return Ok((j, None));
        }
        let done = |v: String, end: usize| -> R<(usize, Option<String>)> {
            if halt_null && v.is_empty() { Err(Abort::Halt) } else { Ok((end, Some(v))) }
        };
        match b[j] {
            b'!' => {
                let st = j + 1;
                let e = ident_extent(text, st);
                if e == st {
                    let v = h.identifier(ctx, "!", &[], None).unwrap_or_default();
                    return Ok((st, Some(v)));
                }
                return Ok((e, Some(format!("${}", &text[st..e]))));
            }
            b'0'..=b'9' => {
                let st = j;
                while j < n && b[j].is_ascii_digit() {
                    j += 1;
                }
                let a: usize = text[st..j].parse().unwrap_or(0);
                let mut range = None;
                if j < n && b[j] == b'-' {
                    j += 1;
                    let st2 = j;
                    while j < n && b[j].is_ascii_digit() {
                        j += 1;
                    }
                    range = Some(if j > st2 { text[st2..j].parse().ok() } else { None });
                }
                let v = self.param_value(a, range);
                return done(v, j);
            }
            b'*' => {
                let v = match &self.rt.star {
                    Some(s) => s.clone(),
                    None => self.param_value(1, Some(None)),
                };
                return done(v, j + 1);
            }
            b'?' => {
                j += 1;
                if j < n && (b[j] == b'*' || b[j] == b'!') {
                    j += 1;
                }
                let mut prompt = String::new();
                if j + 1 < n && b[j] == b'=' && b[j + 1] == b'"' {
                    let st = j + 2;
                    let e = text[st..].find('"').map(|k| st + k).unwrap_or(n);
                    prompt = self.eval_arg(h, ctx, &text[st..e])?;
                    j = (e + 1).min(n);
                }
                let c = self.eff_ctx(ctx).into_owned();
                let v = h.identifier(&c, "?", &[prompt], None).unwrap_or_default();
                return done(v, j);
            }
            _ => {}
        }
        let st = j;
        if b[j] == b'+' && j + 1 < n && b[j + 1] == b'(' {
            j += 1;
        } else {
            while j < n {
                let c = text[j..].chars().next().unwrap();
                if !is_name_char(c) {
                    break;
                }
                j += c.len_utf8();
            }
        }
        let name = &text[st..j];
        if name.is_empty() && !(j < n && b[j] == b'(') {
            return Ok((i + 1, None));
        }
        let mut raw_args: Option<Vec<&str>> = None;
        if j < n
            && b[j] == b'('
            && let Some(close) = matching_paren(&text[j..])
        {
            raw_args = Some(split_args(&text[j + 1..j + close]));
            j += close + 1;
        }
        let mut prop = None;
        if j + 1 < n && b[j] == b'.' && b[j + 1].is_ascii_alphabetic() {
            let ps = j + 1;
            j += 1;
            while j < n && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
                j += 1;
            }
            prop = Some(&text[ps..j]);
        }
        let v = self.call_identifier(h, ctx, name, raw_args, prop, builtin_only)?;
        done(v, j)
    }

    /// Appelle un identifiant : ceux du moteur, puis ceux de l'hôte, puis les alias ; `$null` sinon.
    pub(crate) fn call_identifier(
        &mut self,
        h: &mut dyn Host,
        ctx: &Ctx,
        name: &str,
        raw_args: Option<Vec<&str>>,
        prop: Option<&str>,
        builtin_only: bool,
    ) -> R<String> {
        let lname = name.to_ascii_lowercase();
        // Identifiants qui reçoivent leurs paramètres non évalués.
        if let Some(raw) = &raw_args
            && let Some(v) = self.raw_ident(h, ctx, &lname, raw, prop)?
        {
            return Ok(v);
        }
        let parens = raw_args.is_some();
        let mut args = Vec::new();
        if let Some(raw) = &raw_args {
            for a in raw {
                args.push(self.eval_arg(h, ctx, a)?);
            }
        }
        if let Some(v) = self.builtin_ident(h, ctx, &lname, &args, parens, prop)? {
            return Ok(v);
        }
        let c = self.eff_ctx(ctx).into_owned();
        if let Some(v) = h.identifier(&c, &lname, &args, prop) {
            return Ok(v);
        }
        if let Some(v) = self.late_ident(h, ctx, &lname, &args, parens, prop)? {
            return Ok(v);
        }
        if !builtin_only && let Some((alias, script)) = self.find_alias(&lname) {
            let v = self.call_alias(h, ctx, &alias, script, args, true, prop.unwrap_or(""))?;
            return Ok(v.unwrap_or_default());
        }
        Ok(String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_split() {
        assert_eq!(split_args("a,b,c"), vec!["a", "b", "c"]);
        assert_eq!(split_args("a,$f(b,c),d"), vec!["a", "$f(b,c)", "d"]);
        assert_eq!(split_args(""), Vec::<&str>::new());
        assert_eq!(split_args(","), vec!["", ""]);
    }

    #[test]
    fn extents() {
        assert_eq!(ident_extent("me x", 0), 2);
        assert_eq!(ident_extent("calc(1+2).foo bar", 0), 13);
        assert_eq!(ident_extent("1-", 0), 2);
    }
}
