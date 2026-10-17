//! Dates et durées à la façon de mIRC : `$asctime`, `$duration`, `$ctime(texte)`.
//!
//! Calendrier grégorien proleptique calculé à la main (jours depuis 1970), le décalage horaire local étant donné
//! par l'hôte ([`crate::Host::utc_offset`]).

pub const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
pub const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Date et heure décomposées.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Civil {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub min: u32,
    pub sec: u32,
    /// 0 = dimanche.
    pub wday: u32,
}

/// Jours depuis le 1/1/1970 → (année, mois, jour). Algorithme de Howard Hinnant.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// (année, mois, jour) → jours depuis le 1/1/1970.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = m as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Décompose un instant (secondes depuis 1970) décalé de `offset` secondes.
pub fn civil(ts: i64, offset: i32) -> Civil {
    let t = ts + offset as i64;
    let days = t.div_euclid(86_400);
    let secs = t.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    Civil {
        year,
        month,
        day,
        hour: (secs / 3600) as u32,
        min: (secs % 3600 / 60) as u32,
        sec: (secs % 60) as u32,
        wday: (days + 4).rem_euclid(7) as u32,
    }
}

fn ordinal(d: u32) -> &'static str {
    match (d % 10, d % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    }
}

/// Format par défaut de `$asctime` : `ddd mmm dd HH:nn:ss yyyy`.
pub const DEFAULT_FORMAT: &str = "ddd mmm dd HH:nn:ss yyyy";

/// `$asctime(N, format)`. Jetons : `yyyy yy mmmm mmm mm m dddd ddd dd d oo HH H hh h nn n ss s TT T tt t zzz zz z`,
/// texte entre apostrophes recopié tel quel.
pub fn asctime(ts: i64, offset: i32, fmt: &str) -> String {
    let c = civil(ts, offset);
    let f: Vec<char> = fmt.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let run = |i: usize, ch: char| -> usize {
        let mut k = 0;
        while i + k < f.len() && f[i + k] == ch {
            k += 1;
        }
        k
    };
    let h12 = if c.hour.is_multiple_of(12) { 12 } else { c.hour % 12 };
    while i < f.len() {
        let ch = f[i];
        match ch {
            '\'' => {
                i += 1;
                while i < f.len() && f[i] != '\'' {
                    out.push(f[i]);
                    i += 1;
                }
                i += 1;
            }
            'y' => {
                let k = run(i, 'y');
                if k >= 4 {
                    out.push_str(&format!("{:04}", c.year));
                    i += 4;
                } else if k >= 2 {
                    out.push_str(&format!("{:02}", c.year.rem_euclid(100)));
                    i += 2;
                } else {
                    out.push('y');
                    i += 1;
                }
            }
            'm' => {
                let k = run(i, 'm').min(4);
                match k {
                    4 => out.push_str(MONTHS[c.month as usize - 1]),
                    3 => out.push_str(&MONTHS[c.month as usize - 1][..3]),
                    2 => out.push_str(&format!("{:02}", c.month)),
                    _ => out.push_str(&c.month.to_string()),
                }
                i += k;
            }
            'd' => {
                let k = run(i, 'd').min(4);
                match k {
                    4 => out.push_str(DAYS[c.wday as usize]),
                    3 => out.push_str(&DAYS[c.wday as usize][..3]),
                    2 => out.push_str(&format!("{:02}", c.day)),
                    _ => out.push_str(&c.day.to_string()),
                }
                i += k;
            }
            'o' if run(i, 'o') >= 2 => {
                out.push_str(ordinal(c.day));
                i += 2;
            }
            'H' | 'h' | 'n' | 's' => {
                let k = run(i, ch).min(2);
                let v = match ch {
                    'H' => c.hour,
                    'h' => h12,
                    'n' => c.min,
                    _ => c.sec,
                };
                if k == 2 {
                    out.push_str(&format!("{v:02}"));
                } else {
                    out.push_str(&v.to_string());
                }
                i += k;
            }
            't' | 'T' => {
                let k = run(i, ch).min(2);
                let s = if c.hour < 12 { "am" } else { "pm" };
                let s = if k == 2 { s.to_string() } else { s[..1].to_string() };
                out.push_str(&if ch == 'T' { s.to_uppercase() } else { s });
                i += k;
            }
            'z' => {
                let k = run(i, 'z').min(3);
                let sign = if offset < 0 { '-' } else { '+' };
                let a = offset.unsigned_abs();
                let (hh, mm) = (a / 3600, a % 3600 / 60);
                match k {
                    3 => out.push_str(&format!("{sign}{hh:02}:{mm:02}")),
                    2 => out.push_str(&format!("{sign}{hh:02}{mm:02}")),
                    _ => {
                        if mm == 0 {
                            out.push_str(&format!("{sign}{hh}"))
                        } else {
                            out.push_str(&format!("{sign}{hh}:{mm:02}"))
                        }
                    }
                }
                i += k;
            }
            _ => {
                out.push(ch);
                i += 1;
            }
        }
    }
    out
}

/// `$duration(N)` : `1wk 2days 3hrs 4mins 5secs` ; `mode` 2 : sans les secondes ; 3 : `H:nn:ss`.
pub fn duration(total: i64, mode: u32) -> String {
    let neg = total < 0;
    let t = total.unsigned_abs();
    if mode == 3 {
        let s = format!("{:02}:{:02}:{:02}", t / 3600, t % 3600 / 60, t % 60);
        return if neg { format!("-{s}") } else { s };
    }
    let parts = [(604_800u64, "wk"), (86_400, "day"), (3600, "hr"), (60, "min"), (1, "sec")];
    let mut rest = t;
    let mut out = Vec::new();
    for (i, (unit, name)) in parts.iter().enumerate() {
        if mode == 2 && i == 4 {
            break;
        }
        let n = rest / unit;
        rest %= unit;
        if n > 0 {
            out.push(format!("{n}{name}{}", if n > 1 { "s" } else { "" }));
        }
    }
    if out.is_empty() {
        return if mode == 2 { "0mins".into() } else { "0secs".into() };
    }
    let s = out.join(" ");
    if neg { format!("-{s}") } else { s }
}

/// Lit une durée écrite (`1hr 1min 40secs`, `2d 3h`, `1w`) → secondes.
pub fn parse_duration(text: &str) -> Option<i64> {
    let mut total = 0i64;
    let mut any = false;
    for word in text.split_whitespace() {
        let w = word.to_ascii_lowercase();
        let split = w.find(|c: char| !c.is_ascii_digit()).unwrap_or(w.len());
        let (n, unit) = w.split_at(split);
        let n: i64 = n.parse().ok()?;
        let mult = match unit.trim_end_matches('s') {
            "w" | "wk" | "week" => 604_800,
            "d" | "day" => 86_400,
            "h" | "hr" | "hour" => 3600,
            "m" | "min" | "minute" => 60,
            "" | "sec" | "second" => 1,
            _ => return None,
        };
        total += n * mult;
        any = true;
    }
    any.then_some(total)
}

fn month_from_name(s: &str) -> Option<u32> {
    let l = s.to_ascii_lowercase();
    if l.len() < 3 {
        return None;
    }
    let p = l.get(..3)?;
    MONTHS.iter().position(|m| m.to_ascii_lowercase().starts_with(p)).map(|i| i as u32 + 1)
}

/// `$ctime(texte)` : lit une date (heure locale) dans les formats courants
/// (`dd/mm/yyyy [HH:nn[:ss]]`, `yyyy-mm-dd[ HH:nn[:ss]]`, `[ddd] mmm dd HH:nn:ss yyyy`, `dd mmm yyyy […]`).
pub fn parse_ctime(text: &str, offset_for: impl Fn(i64) -> i32) -> Option<i64> {
    // Le `T` de l'ISO 8601 (`2001-09-09T01:46:40`) seulement entre deux chiffres.
    let tc: Vec<char> = text.chars().collect();
    let text: String = tc
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let iso = c == 'T'
                && i > 0
                && tc[i - 1].is_ascii_digit()
                && tc.get(i + 1).is_some_and(|n| n.is_ascii_digit());
            if iso { ' ' } else { c }
        })
        .collect();
    let words: Vec<&str> = text.split([' ', ',']).filter(|w| !w.is_empty()).collect();
    let (mut y, mut mo, mut d) = (None, None, None);
    let (mut hh, mut mm, mut ss) = (0u32, 0u32, 0u32);
    let mut pm = None;
    for w in &words {
        if w.contains(':') {
            let p: Vec<&str> = w.split(':').collect();
            hh = p.first()?.parse().ok()?;
            mm = p.get(1).map(|x| x.parse().unwrap_or(0)).unwrap_or(0);
            ss = p.get(2).map(|x| x.trim_end_matches('Z').parse().unwrap_or(0)).unwrap_or(0);
        } else if w.contains('/') || (w.contains('-') && w.len() >= 8) {
            let p: Vec<&str> = w.split(['/', '-']).collect();
            if p.len() != 3 {
                return None;
            }
            let a: i64 = p[0].parse().ok()?;
            let b: u32 = p[1].parse().ok()?;
            let c: i64 = p[2].parse().ok()?;
            if p[0].len() == 4 {
                y = Some(a);
                mo = Some(b);
                d = Some(c as u32);
            } else {
                d = Some(a as u32);
                mo = Some(b);
                y = Some(if c < 100 { c + 2000 } else { c });
            }
        } else if let Some(m) = month_from_name(w) {
            mo = Some(m);
        } else if w.eq_ignore_ascii_case("am") || w.eq_ignore_ascii_case("pm") {
            pm = Some(w.eq_ignore_ascii_case("pm"));
        } else if let Ok(n) = w.parse::<i64>() {
            if n > 31 || w.len() == 4 {
                y = Some(n);
            } else if d.is_none() {
                d = Some(n as u32);
            } else {
                y = Some(n + if n < 100 { 2000 } else { 0 });
            }
        } else if DAYS.iter().any(|x| x.to_ascii_lowercase().starts_with(&w.to_ascii_lowercase())) {
            // Nom du jour : ignoré.
        } else {
            return None;
        }
    }
    if let Some(p) = pm {
        if p && hh < 12 {
            hh += 12;
        } else if !p && hh == 12 {
            hh = 0;
        }
    }
    let (y, mo, d) = (y?, mo?, d?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || hh > 23 || mm > 59 || ss > 60 {
        return None;
    }
    let local = days_from_civil(y, mo, d) * 86_400 + (hh * 3600 + mm * 60 + ss) as i64;
    Some(local - offset_for(local) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        let c = civil(1_000_000_000, 0);
        assert_eq!((c.year, c.month, c.day, c.hour, c.min, c.sec), (2001, 9, 9, 1, 46, 40));
        assert_eq!(c.wday, 0);
    }

    #[test]
    fn formats() {
        assert_eq!(asctime(1_000_000_000, 0, DEFAULT_FORMAT), "Sun Sep 09 01:46:40 2001");
        assert_eq!(asctime(1_000_000_000, 0, "dd/mm/yyyy"), "09/09/2001");
        assert_eq!(asctime(1_000_000_000, 0, "dddd d mmmm yy"), "Sunday 9 September 01");
        assert_eq!(asctime(1_000_000_000, 0, "h:nn TT"), "1:46 AM");
        assert_eq!(asctime(1_000_000_000, 3600, "HH:nn zz"), "02:46 +0100");
        assert_eq!(asctime(1_000_000_000, 0, "doo"), "9th");
        assert_eq!(asctime(1_000_000_000, 0, "'at' HH"), "at 01");
    }

    #[test]
    fn durations() {
        assert_eq!(duration(3700, 1), "1hr 1min 40secs");
        assert_eq!(duration(0, 1), "0secs");
        assert_eq!(duration(90061, 1), "1day 1hr 1min 1sec");
        assert_eq!(duration(694861, 1), "1wk 1day 1hr 1min 1sec");
        assert_eq!(duration(3700, 3), "01:01:40");
        assert_eq!(duration(3700, 2), "1hr 1min");
        assert_eq!(parse_duration("1hr 1min 40secs"), Some(3700));
        assert_eq!(parse_duration("2days"), Some(172_800));
    }

    #[test]
    fn ctime_parse() {
        assert_eq!(parse_ctime("09/09/2001 01:46:40", |_| 0), Some(1_000_000_000));
        assert_eq!(parse_ctime("2001-09-09 01:46:40", |_| 0), Some(1_000_000_000));
        assert_eq!(parse_ctime("Sun Sep 09 01:46:40 2001", |_| 0), Some(1_000_000_000));
        assert_eq!(parse_ctime("9 September 2001 02:46:40", |_| 3600), Some(1_000_000_000));
        assert_eq!(parse_ctime("nonsense", |_| 0), None);
    }
}
