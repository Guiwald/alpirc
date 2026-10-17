//! Corps de commandes : découpage en instructions (`{ }`, `|`, lignes), `if`/`elseif`/`else`, `while`, étiquettes.
//!
//! Comme dans mIRC, les accolades et la barre `|` ne comptent que si elles sont isolées par des espaces
//! (`if (x) { echo a | echo b }`) ; une accolade collée au texte (`{hello}`) reste du texte. Les lignes de commande
//! elles-mêmes restent du texte brut : identifiants et variables sont évalués à l'exécution.

/// Une suite d'instructions.
pub type Block = Vec<Stmt>;

/// Une instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    /// Ligne de commande brute (`msg $chan salut`).
    Cmd { line: usize, text: String },
    /// `if (…) … elseif (…) … else …` : `cond` vaut `None` pour le `else`.
    If { line: usize, branches: Vec<(Option<String>, Block)> },
    While { line: usize, cond: String, body: Block },
    /// `:etiquette` (cible de `goto`), en minuscules.
    Label { line: usize, name: String },
    /// Bloc `{ … }` isolé.
    Block { line: usize, body: Block },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Text(String, usize),
    Open(usize),
    Close,
    Sep,
}

fn is_ws(b: u8) -> bool {
    b == b' ' || b == b'\t'
}

/// Découpe un corps en jetons : texte, `{`, `}`, séparateurs (fin de ligne ou `|`).
fn tokenize(text: &str, first_line: usize) -> Vec<Tok> {
    let mut toks = Vec::new();
    for (n, raw) in text.split('\n').enumerate() {
        let ln = first_line + n;
        let line = raw.trim_end_matches('\r');
        let b = line.as_bytes();
        let mut start = 0usize;
        let mut j = 0usize;
        let flush = |toks: &mut Vec<Tok>, s: usize, e: usize| {
            let t = line[s..e].trim();
            if !t.is_empty() {
                toks.push(Tok::Text(t.to_string(), ln));
            }
        };
        // Saute les espaces au début d'une instruction ; un `;` à cet endroit commente la fin de la ligne.
        let skip_comment = |j: usize| -> bool {
            let mut k = j;
            while k < b.len() && is_ws(b[k]) {
                k += 1;
            }
            k < b.len() && b[k] == b';'
        };
        if skip_comment(0) {
            toks.push(Tok::Sep);
            continue;
        }
        while j < b.len() {
            let c = b[j];
            let prev_ws = j == 0 || is_ws(b[j - 1]);
            let next_ws = j + 1 >= b.len() || is_ws(b[j + 1]);
            if c == b'|' && prev_ws && next_ws {
                flush(&mut toks, start, j);
                toks.push(Tok::Sep);
                start = j + 1;
                if skip_comment(start) {
                    start = b.len();
                    j = b.len();
                    continue;
                }
            } else if c == b'{' && (prev_ws || b[j - 1] == b')') && next_ws {
                flush(&mut toks, start, j);
                toks.push(Tok::Open(ln));
                start = j + 1;
            } else if c == b'}' && prev_ws && next_ws {
                flush(&mut toks, start, j);
                toks.push(Tok::Close);
                start = j + 1;
            }
            j += 1;
        }
        flush(&mut toks, start, b.len());
        toks.push(Tok::Sep);
    }
    toks
}

/// Premier mot (en minuscules) d'un texte.
fn first_word_lower(t: &str) -> String {
    t.split([' ', '(']).next().unwrap_or("").to_ascii_lowercase()
}

/// Trouve la parenthèse fermante qui correspond à celle en position 0.
pub fn matching_paren(s: &str) -> Option<usize> {
    let mut depth = 0i32;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Parenthèses équilibrées sur tout le texte.
fn balanced(s: &str) -> bool {
    let mut d = 0i32;
    for c in s.chars() {
        match c {
            '(' => d += 1,
            ')' => {
                d -= 1;
                if d < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    d == 0
}

/// Fin de la condition qui commence par un groupe fermé en `first` : les groupes suivants reliés par `&&` ou
/// `||` en font partie (`if (a) && (b) …`).
fn extend_condition(s: &str, first: usize) -> usize {
    let mut end = first;
    loop {
        let rest = &s[end + 1..];
        let t = rest.trim_start();
        let Some(after_op) = t.strip_prefix("&&").or_else(|| t.strip_prefix("||")) else { return end };
        let g = after_op.trim_start();
        let g = g.strip_prefix('!').unwrap_or(g);
        if !g.starts_with('(') {
            return end;
        }
        let start = s.len() - g.len();
        match matching_paren(&s[start..]) {
            Some(e) => end = start + e,
            None => return end,
        }
    }
}

/// Sépare `(condition) suite` ; `block_follows` : un `{` suit directement sur la même ligne.
fn split_condition(rest: &str, block_follows: bool) -> (String, String) {
    let rest = rest.trim();
    if rest.starts_with('(') {
        let first = matching_paren(rest);
        if let Some(f) = first {
            let end = extend_condition(rest, f);
            if end == rest.len() - 1 || !block_follows || balanced(rest) {
                let cond = if end == f { rest[1..f].to_string() } else { rest[..=end].to_string() };
                return (cond, rest[end + 1..].trim().to_string());
            }
        }
        if block_follows && rest.ends_with(')') {
            // Parenthèses déséquilibrées (`if ($1 == :)) {`) : on retire seulement les extrêmes.
            return (rest[1..rest.len() - 1].to_string(), String::new());
        }
        if let Some(e) = first {
            return (rest[1..e].to_string(), rest[e + 1..].trim().to_string());
        }
        return (rest.strip_prefix('(').unwrap_or(rest).to_string(), String::new());
    }
    // Sans parenthèses : tout le texte est la condition.
    (rest.to_string(), String::new())
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    /// Prochain jeton hors séparateurs, sans le consommer ; rend sa position.
    fn peek_skip_sep(&self) -> Option<(usize, &Tok)> {
        let mut p = self.pos;
        while let Some(t) = self.toks.get(p) {
            if *t != Tok::Sep {
                return Some((p, t));
            }
            p += 1;
        }
        None
    }

    fn block_until_close(&mut self) -> Block {
        let mut out = Vec::new();
        while let Some(t) = self.toks.get(self.pos).cloned() {
            self.pos += 1;
            match t {
                Tok::Sep => {}
                Tok::Close => return out,
                Tok::Open(line) => {
                    let body = self.block_until_close();
                    out.push(Stmt::Block { line, body });
                }
                Tok::Text(text, line) => {
                    if let Some(s) = self.stmt(text, line) {
                        out.push(s);
                    }
                }
            }
        }
        out
    }

    /// Corps d'un `if`/`while`/`else` : la commande sur la même ligne, ou le bloc `{ }` qui suit.
    fn body(&mut self, after: String, line: usize) -> Block {
        if !after.is_empty() {
            return self.stmt(after, line).into_iter().collect();
        }
        if let Some((p, Tok::Open(_))) = self.peek_skip_sep() {
            self.pos = p + 1;
            return self.block_until_close();
        }
        Vec::new()
    }

    fn cond_and_body(&mut self, rest: &str, line: usize) -> (String, Block) {
        let block_follows = matches!(self.peek(), Some(Tok::Open(_)));
        let (cond, after) = split_condition(rest, block_follows);
        let body = self.body(after, line);
        (cond, body)
    }

    fn stmt(&mut self, text: String, line: usize) -> Option<Stmt> {
        let kw = first_word_lower(&text);
        match kw.as_str() {
            "if" => {
                let rest = text[2..].to_string();
                let (cond, body) = self.cond_and_body(&rest, line);
                let mut branches = vec![(Some(cond), body)];
                while let Some((p, Tok::Text(t, l))) = self.peek_skip_sep().map(|(p, t)| (p, t.clone())) {
                    let w = first_word_lower(&t);
                    if w == "elseif" {
                        self.pos = p + 1;
                        let (c, b) = self.cond_and_body(&t[6..], l);
                        branches.push((Some(c), b));
                    } else if w == "else" {
                        self.pos = p + 1;
                        let rest = t[4..].trim().to_string();
                        if first_word_lower(&rest) == "if" {
                            let (c, b) = self.cond_and_body(&rest[2..], l);
                            branches.push((Some(c), b));
                        } else {
                            let b = self.body(rest, l);
                            branches.push((None, b));
                            break;
                        }
                    } else {
                        break;
                    }
                }
                Some(Stmt::If { line, branches })
            }
            "while" => {
                let rest = text[5..].to_string();
                let (cond, body) = self.cond_and_body(&rest, line);
                Some(Stmt::While { line, cond, body })
            }
            // `else` ou `elseif` orphelin : ignoré.
            "else" | "elseif" => None,
            _ => {
                if let Some(name) = text.strip_prefix(':')
                    && !name.is_empty()
                    && !name.contains(' ')
                {
                    return Some(Stmt::Label { line, name: name.to_ascii_lowercase() });
                }
                Some(Stmt::Cmd { line, text })
            }
        }
    }
}

/// Lit un corps de commandes. `first_line` : numéro (à partir de 1) de sa première ligne dans le fichier.
pub fn parse_body(text: &str, first_line: usize) -> Block {
    let mut p = Parser { toks: tokenize(text, first_line), pos: 0 };
    let mut out = Vec::new();
    // Un `}` en trop au niveau le plus haut est ignoré.
    while p.pos < p.toks.len() {
        let mut b = p.block_until_close();
        out.append(&mut b);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(t: &str) -> Stmt {
        Stmt::Cmd { line: 1, text: t.into() }
    }

    #[test]
    fn pipes_and_blocks() {
        let b = parse_body("echo a | echo b", 1);
        assert_eq!(b, vec![cmd("echo a"), cmd("echo b")]);
        let b = parse_body("echo a|b", 1);
        assert_eq!(b, vec![cmd("echo a|b")]);
        let b = parse_body("echo {hello}", 1);
        assert_eq!(b, vec![cmd("echo {hello}")]);
    }

    #[test]
    fn if_chain() {
        let b = parse_body("if (%x == 1) { echo one } elseif (%x == 2) echo two | else { echo other }", 1);
        match &b[0] {
            Stmt::If { branches, .. } => {
                assert_eq!(branches.len(), 3);
                assert_eq!(branches[0].0.as_deref(), Some("%x == 1"));
                assert_eq!(branches[1].1, vec![cmd("echo two")]);
                assert_eq!(branches[2].0, None);
            }
            _ => panic!(),
        }
        assert_eq!(b.len(), 1);
    }

    #[test]
    fn multiline() {
        let src = "var %i = 1\nwhile (%i <= 3) {\n  echo %i\n  inc %i\n}\n:fin\n; commentaire }\necho done";
        let b = parse_body(src, 10);
        assert_eq!(b.len(), 4);
        assert!(matches!(&b[1], Stmt::While { cond, body, .. } if cond == "%i <= 3" && body.len() == 2));
        assert!(matches!(&b[2], Stmt::Label { name, line: 15 } if name == "fin"));
    }

    #[test]
    fn joined_conditions() {
        let b = parse_body("if ($1 == a) && ($2) { echo x }", 1);
        assert!(matches!(&b[0], Stmt::If { branches, .. } if branches[0].0.as_deref() == Some("($1 == a) && ($2)")));
        let b = parse_body("if (%a) || (!%b) echo y", 1);
        assert!(matches!(&b[0], Stmt::If { branches, .. }
            if branches[0].0.as_deref() == Some("(%a) || (!%b)") && branches[0].1 == vec![cmd("echo y")]));
    }

    #[test]
    fn condition_with_parens() {
        let b = parse_body("if ($1 == :)) { echo smile }", 1);
        assert!(matches!(&b[0], Stmt::If { branches, .. } if branches[0].0.as_deref() == Some("$1 == :)")));
        let b = parse_body("if ($len($1) > 2) echo (long)", 1);
        assert!(matches!(&b[0], Stmt::If { branches, .. } if branches[0].1 == vec![cmd("echo (long)")]));
    }
}
