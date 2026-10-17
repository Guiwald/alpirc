//! `$calc` et l'écriture des nombres à la façon de mIRC (jusqu'à 6 décimales, sans zéros inutiles).

/// Nombre entier ou décimal écrit en entier (`12`, `-3.5`, `+4`, `.5`) ; `None` sinon.
pub fn parse_num(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let b = t.as_bytes();
    let mut i = 0;
    if b[0] == b'-' || b[0] == b'+' {
        i = 1;
    }
    let mut digits = 0;
    let mut dot = false;
    while i < b.len() {
        match b[i] {
            b'0'..=b'9' => digits += 1,
            b'.' if !dot => dot = true,
            _ => return None,
        }
        i += 1;
    }
    if digits == 0 {
        return None;
    }
    t.parse::<f64>().ok()
}

/// Valeur numérique d'un texte qui commence par un nombre (`12abc` → 12), 0 sinon (`inc`, tri numérique).
pub fn parse_num_prefix(s: &str) -> f64 {
    let t = s.trim();
    let b = t.as_bytes();
    let mut i = 0;
    if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
        i += 1;
    }
    let mut dot = false;
    while i < b.len() && (b[i].is_ascii_digit() || (b[i] == b'.' && !dot)) {
        if b[i] == b'.' {
            dot = true;
        }
        i += 1;
    }
    t[..i].parse::<f64>().unwrap_or(0.0)
}

/// Écrit un nombre : entier sans décimales, sinon au plus 6 décimales sans zéros finaux.
pub fn fmt_num(v: f64) -> String {
    if !v.is_finite() {
        return "0".into();
    }
    if v.fract() == 0.0 {
        let s = format!("{v:.0}");
        return if s == "-0" { "0".into() } else { s };
    }
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() { "0".into() } else { s.to_string() }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum T {
    Num(f64),
    Op(char),
    Open,
    Close,
}

fn lex(s: &str) -> Vec<T> {
    let mut out = Vec::new();
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || c == '.' {
            let st = i;
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == '.') {
                i += 1;
            }
            // Exposant éventuel (1e3).
            if i < b.len() && (b[i] == 'e' || b[i] == 'E') && i + 1 < b.len() && (b[i + 1].is_ascii_digit()) {
                i += 1;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let txt: String = b[st..i].iter().collect();
            out.push(T::Num(txt.parse().unwrap_or(0.0)));
        } else if "+-*/%^".contains(c) {
            out.push(T::Op(c));
            i += 1;
        } else if c == '(' {
            out.push(T::Open);
            i += 1;
        } else if c == ')' {
            out.push(T::Close);
            i += 1;
        } else {
            // Caractère inconnu : ignoré, comme mIRC qui ne s'arrête pas sur du texte.
            i += 1;
        }
    }
    out
}

struct P {
    t: Vec<T>,
    i: usize,
}

impl P {
    fn peek(&self) -> Option<T> {
        self.t.get(self.i).copied()
    }

    fn expr(&mut self) -> f64 {
        let mut v = self.term();
        while let Some(T::Op(c @ ('+' | '-'))) = self.peek() {
            self.i += 1;
            let r = self.term();
            v = if c == '+' { v + r } else { v - r };
        }
        v
    }

    fn term(&mut self) -> f64 {
        let mut v = self.power();
        while let Some(T::Op(c @ ('*' | '/' | '%'))) = self.peek() {
            self.i += 1;
            let r = self.power();
            v = match c {
                '*' => v * r,
                '/' => {
                    if r == 0.0 {
                        0.0
                    } else {
                        v / r
                    }
                }
                _ => {
                    if r == 0.0 {
                        0.0
                    } else {
                        v % r
                    }
                }
            };
        }
        v
    }

    fn power(&mut self) -> f64 {
        let b = self.unary();
        if let Some(T::Op('^')) = self.peek() {
            self.i += 1;
            let e = self.power();
            let r = b.powf(e);
            return if r.is_finite() { r } else { 0.0 };
        }
        b
    }

    fn unary(&mut self) -> f64 {
        match self.peek() {
            Some(T::Op('-')) => {
                self.i += 1;
                -self.unary()
            }
            Some(T::Op('+')) => {
                self.i += 1;
                self.unary()
            }
            _ => self.atom(),
        }
    }

    fn atom(&mut self) -> f64 {
        match self.peek() {
            Some(T::Num(n)) => {
                self.i += 1;
                n
            }
            Some(T::Open) => {
                self.i += 1;
                let v = self.expr();
                if let Some(T::Close) = self.peek() {
                    self.i += 1;
                }
                v
            }
            Some(_) => {
                self.i += 1;
                0.0
            }
            None => 0.0,
        }
    }
}

/// Évalue une expression arithmétique (`+ - * / % ^`, parenthèses, décimales).
/// La division par zéro donne 0, comme dans mIRC.
pub fn calc(s: &str) -> f64 {
    let mut p = P { t: lex(s), i: 0 };
    let mut v = p.expr();
    // Reste non lu (parenthèse fermante en trop…) : on continue comme si de rien n'était.
    while p.i < p.t.len() {
        p.i += 1;
        if p.i < p.t.len() {
            v = p.expr();
        }
    }
    if v.is_finite() { v } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arith() {
        assert_eq!(fmt_num(calc("1+2*3")), "7");
        assert_eq!(fmt_num(calc("(1+2)*3")), "9");
        assert_eq!(fmt_num(calc("10/3")), "3.333333");
        assert_eq!(fmt_num(calc("7 % 3")), "1");
        assert_eq!(fmt_num(calc("2^10")), "1024");
        assert_eq!(fmt_num(calc("-2^2")), "4");
        assert_eq!(fmt_num(calc("1/0")), "0");
        assert_eq!(fmt_num(calc("0.1+0.2")), "0.3");
        assert_eq!(fmt_num(calc("2.5*2")), "5");
        assert_eq!(fmt_num(calc("")), "0");
        assert_eq!(fmt_num(calc("5 - -3")), "8");
    }

    #[test]
    fn nums() {
        assert_eq!(parse_num("12"), Some(12.0));
        assert_eq!(parse_num("-1.5"), Some(-1.5));
        assert_eq!(parse_num("1e3"), None);
        assert_eq!(parse_num("abc"), None);
        assert_eq!(parse_num_prefix("12abc"), 12.0);
    }
}
