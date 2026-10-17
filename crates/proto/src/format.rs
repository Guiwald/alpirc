//! Codes de mise en forme de mIRC : gras (Ctrl+B, `\x02`), couleur (Ctrl+K, `\x03fg,bg`), couleur hexadécimale
//! (`\x04RRGGBB`), italique (Ctrl+I, `\x1D`), souligné (Ctrl+U, `\x1F`), barré (`\x1E`), chasse fixe (`\x11`),
//! inverse (Ctrl+R, `\x16`), fin de mise en forme (Ctrl+O, `\x0F`).

pub const BOLD: char = '\x02';
pub const COLOR: char = '\x03';
pub const HEX_COLOR: char = '\x04';
pub const RESET: char = '\x0F';
pub const MONOSPACE: char = '\x11';
pub const REVERSE: char = '\x16';
pub const ITALIC: char = '\x1D';
pub const STRIKE: char = '\x1E';
pub const UNDERLINE: char = '\x1F';

/// Les 16 couleurs de base de mIRC, puis les 83 couleurs étendues (16 à 98), en `0xRRGGBB`.
pub const PALETTE: [u32; 99] = [
    0xFFFFFF, 0x000000, 0x00007F, 0x009300, 0xFF0000, 0x7F0000, 0x9C009C, 0xFC7F00, 0xFFFF00, 0x00FC00, 0x009393,
    0x00FFFF, 0x0000FC, 0xFF00FF, 0x7F7F7F, 0xD2D2D2, //
    0x470000, 0x472100, 0x474700, 0x324700, 0x004700, 0x00472C, 0x004747, 0x002747, 0x000047, 0x2E0047, 0x470047,
    0x47002A, 0x740000, 0x743A00, 0x747400, 0x517400, 0x007400, 0x007449, 0x007474, 0x004074, 0x000074, 0x4B0074,
    0x740074, 0x740045, 0xB50000, 0xB56300, 0xB5B500, 0x7DB500, 0x00B500, 0x00B571, 0x00B5B5, 0x0063B5, 0x0000B5,
    0x7500B5, 0xB500B5, 0xB5006B, 0xFF0000, 0xFF8C00, 0xFFFF00, 0xB2FF00, 0x00FF00, 0x00FFA0, 0x00FFFF, 0x008CFF,
    0x0000FF, 0xA500FF, 0xFF00FF, 0xFF0098, 0xFF5959, 0xFFB459, 0xFFFF71, 0xCFFF60, 0x6FFF6F, 0x65FFC9, 0x6DFFFF,
    0x59B4FF, 0x5959FF, 0xC459FF, 0xFF66FF, 0xFF59BC, 0xFF9C9C, 0xFFD39C, 0xFFFF9C, 0xE2FF9C, 0x9CFF9C, 0x9CFFDB,
    0x9CFFFF, 0x9CD3FF, 0x9C9CFF, 0xDC9CFF, 0xFF9CFF, 0xFF94D3, 0x000000, 0x131313, 0x282828, 0x363636, 0x4D4D4D,
    0x656565, 0x818181, 0x9F9F9F, 0xBCBCBC, 0xE2E2E2, 0xFFFFFF,
];

/// Une couleur de texte : numéro mIRC (0 à 98, dont les 16 de base redéfinissables par le thème) ou valeur exacte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Index(u8),
    Rgb(u32),
}

/// Mise en forme d'un morceau de texte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub monospace: bool,
    pub reverse: bool,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
}

/// Un morceau de texte d'une même mise en forme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub style: Style,
    pub text: String,
}

fn take_digits(chars: &[char], i: &mut usize, max: usize) -> Option<u8> {
    let start = *i;
    while *i < chars.len() && *i - start < max && chars[*i].is_ascii_digit() {
        *i += 1;
    }
    if *i == start {
        return None;
    }
    let s: String = chars[start..*i].iter().collect();
    s.parse().ok()
}

fn take_hex(chars: &[char], i: &mut usize) -> Option<u32> {
    if *i + 6 > chars.len() || !chars[*i..*i + 6].iter().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let s: String = chars[*i..*i + 6].iter().collect();
    *i += 6;
    u32::from_str_radix(&s, 16).ok()
}

fn index_color(n: u8) -> Option<Color> {
    // 99 = « couleur par défaut » ; au-delà, mIRC ramène dans 0–15.
    match n {
        0..=98 => Some(Color::Index(n)),
        99 => None,
        n => Some(Color::Index(n % 16)),
    }
}

/// Découpe un texte en morceaux mis en forme (les codes disparaissent).
pub fn spans(text: &str) -> Vec<Span> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<Span> = Vec::new();
    let mut style = Style::default();
    let mut cur = String::new();
    let mut i = 0;
    let flush = |out: &mut Vec<Span>, cur: &mut String, style: Style| {
        if !cur.is_empty() {
            if let Some(last) = out.last_mut() {
                if last.style == style {
                    last.text.push_str(cur);
                    cur.clear();
                    return;
                }
            }
            out.push(Span { style, text: std::mem::take(cur) });
        }
    };
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        match c {
            BOLD => {
                flush(&mut out, &mut cur, style);
                style.bold = !style.bold;
            }
            ITALIC => {
                flush(&mut out, &mut cur, style);
                style.italic = !style.italic;
            }
            UNDERLINE => {
                flush(&mut out, &mut cur, style);
                style.underline = !style.underline;
            }
            STRIKE => {
                flush(&mut out, &mut cur, style);
                style.strike = !style.strike;
            }
            MONOSPACE => {
                flush(&mut out, &mut cur, style);
                style.monospace = !style.monospace;
            }
            REVERSE => {
                flush(&mut out, &mut cur, style);
                style.reverse = !style.reverse;
            }
            RESET => {
                flush(&mut out, &mut cur, style);
                style = Style::default();
            }
            COLOR => {
                flush(&mut out, &mut cur, style);
                match take_digits(&chars, &mut i, 2) {
                    None => {
                        style.fg = None;
                        style.bg = None;
                    }
                    Some(fg) => {
                        style.fg = index_color(fg);
                        if i + 1 < chars.len() && chars[i] == ',' && chars[i + 1].is_ascii_digit() {
                            i += 1;
                            if let Some(bg) = take_digits(&chars, &mut i, 2) {
                                style.bg = index_color(bg);
                            }
                        }
                    }
                }
            }
            HEX_COLOR => {
                flush(&mut out, &mut cur, style);
                match take_hex(&chars, &mut i) {
                    None => {
                        style.fg = None;
                        style.bg = None;
                    }
                    Some(fg) => {
                        style.fg = Some(Color::Rgb(fg));
                        if i < chars.len() && chars[i] == ',' {
                            let mut j = i + 1;
                            if let Some(bg) = take_hex(&chars, &mut j) {
                                style.bg = Some(Color::Rgb(bg));
                                i = j;
                            }
                        }
                    }
                }
            }
            c => cur.push(c),
        }
    }
    flush(&mut out, &mut cur, style);
    out
}

/// Retire tous les codes de mise en forme (`$strip` de mIRC).
pub fn strip(text: &str) -> String {
    spans(text).into_iter().map(|s| s.text).collect()
}

/// Vrai si le texte contient au moins un code de mise en forme.
pub fn has_codes(text: &str) -> bool {
    text.chars().any(|c| matches!(c, BOLD | COLOR | HEX_COLOR | RESET | MONOSPACE | REVERSE | ITALIC | STRIKE | UNDERLINE))
}

/// Couleur RGB d'un numéro mIRC selon la palette donnée (les 16 de base viennent du thème).
pub fn rgb(color: Color, base16: &[u32; 16]) -> u32 {
    match color {
        Color::Rgb(v) => v,
        Color::Index(n) if (n as usize) < 16 => base16[n as usize],
        Color::Index(n) => PALETTE[(n as usize).min(98)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors() {
        let s = spans("a\x034,12b\x03c\x02d\x0Fe");
        assert_eq!(s.len(), 5);
        assert_eq!(s[1].style.fg, Some(Color::Index(4)));
        assert_eq!(s[1].style.bg, Some(Color::Index(12)));
        assert_eq!(s[2].style.fg, None);
        assert!(s[3].style.bold);
        assert_eq!(s[4].style, Style::default());
        assert_eq!(strip("a\x034,12b\x03c\x02d\x0Fe"), "abcde");
    }

    #[test]
    fn comma_not_bg() {
        let s = spans("\x034,salut");
        assert_eq!(s[0].text, ",salut");
        assert_eq!(s[0].style.fg, Some(Color::Index(4)));
        let s = spans("\x03123");
        assert_eq!(s[0].text, "3");
        assert_eq!(s[0].style.fg, Some(Color::Index(12)));
    }

    #[test]
    fn hex() {
        let s = spans("\x04FF0000,00FF00x");
        assert_eq!(s[0].style.fg, Some(Color::Rgb(0xFF0000)));
        assert_eq!(s[0].style.bg, Some(Color::Rgb(0x00FF00)));
    }

    #[test]
    fn merge_same_style() {
        let s = spans("ab\x02\x02cd");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].text, "abcd");
    }
}
