//! Petits outils de date et d'heure, sans dépendre de l'horloge du système : conversions de dates civiles,
//! ISO 8601 (étiquette `time` d'IRCv3, `MARKREAD`), formats d'heure à la mIRC (`[HH:nn]`), durées (`5mins 3secs`),
//! dates dans la langue de l'interface ([`date_text`] : noms des jours et des mois dans les sept langues).

use crate::i18n::Lang;

/// Jours depuis le 1/1/1970 → (année, mois, jour) (algorithme de Howard Hinnant).
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
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
    let yoe = (y - era * 400) as u64;
    let m = m as u64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe as i64 - 719_468
}

/// Une date et une heure civiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Civil {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub millis: u32,
    /// 0 = dimanche.
    pub weekday: u32,
}

/// Date civile d'un instant (ms depuis 1970), décalé de `offset_secs` (heure locale).
pub fn civil(ms: i64, offset_secs: i32) -> Civil {
    let t = ms + offset_secs as i64 * 1000;
    let days = t.div_euclid(86_400_000);
    let rem = t.rem_euclid(86_400_000);
    let (year, month, day) = civil_from_days(days);
    Civil {
        year,
        month,
        day,
        hour: (rem / 3_600_000) as u32,
        minute: (rem / 60_000 % 60) as u32,
        second: (rem / 1000 % 60) as u32,
        millis: (rem % 1000) as u32,
        weekday: (days + 4).rem_euclid(7) as u32,
    }
}

/// Noms des jours (dimanche d'abord) et des mois, courts et longs, d'une langue.
struct Names {
    days: [&'static str; 7],
    days_long: [&'static str; 7],
    months: [&'static str; 12],
    months_long: [&'static str; 12],
}

const EN: Names = Names {
    days: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
    days_long: ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"],
    months: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
    months_long: [
        "January", "February", "March", "April", "May", "June",
        "July", "August", "September", "October", "November", "December",
    ],
};

const FR: Names = Names {
    days: ["dim.", "lun.", "mar.", "mer.", "jeu.", "ven.", "sam."],
    days_long: ["dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi"],
    months: ["janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc."],
    months_long: [
        "janvier", "février", "mars", "avril", "mai", "juin",
        "juillet", "août", "septembre", "octobre", "novembre", "décembre",
    ],
};

const ES: Names = Names {
    days: ["dom", "lun", "mar", "mié", "jue", "vie", "sáb"],
    days_long: ["domingo", "lunes", "martes", "miércoles", "jueves", "viernes", "sábado"],
    months: ["ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sept", "oct", "nov", "dic"],
    months_long: [
        "enero", "febrero", "marzo", "abril", "mayo", "junio",
        "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre",
    ],
};

const DE: Names = Names {
    days: ["So.", "Mo.", "Di.", "Mi.", "Do.", "Fr.", "Sa."],
    days_long: ["Sonntag", "Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag"],
    months: ["Jan.", "Feb.", "März", "Apr.", "Mai", "Juni", "Juli", "Aug.", "Sept.", "Okt.", "Nov.", "Dez."],
    months_long: [
        "Januar", "Februar", "März", "April", "Mai", "Juni",
        "Juli", "August", "September", "Oktober", "November", "Dezember",
    ],
};

const IT: Names = Names {
    days: ["dom", "lun", "mar", "mer", "gio", "ven", "sab"],
    days_long: ["domenica", "lunedì", "martedì", "mercoledì", "giovedì", "venerdì", "sabato"],
    months: ["gen", "feb", "mar", "apr", "mag", "giu", "lug", "ago", "set", "ott", "nov", "dic"],
    months_long: [
        "gennaio", "febbraio", "marzo", "aprile", "maggio", "giugno",
        "luglio", "agosto", "settembre", "ottobre", "novembre", "dicembre",
    ],
};

/// Valencien.
const VA: Names = Names {
    days: ["dg.", "dl.", "dt.", "dc.", "dj.", "dv.", "ds."],
    days_long: ["diumenge", "dilluns", "dimarts", "dimecres", "dijous", "divendres", "dissabte"],
    months: ["gen.", "febr.", "març", "abr.", "maig", "juny", "jul.", "ag.", "set.", "oct.", "nov.", "des."],
    months_long: [
        "gener", "febrer", "març", "abril", "maig", "juny",
        "juliol", "agost", "setembre", "octubre", "novembre", "desembre",
    ],
};

/// Irlandais.
const GA: Names = Names {
    days: ["Domh", "Luan", "Máirt", "Céad", "Déar", "Aoine", "Sath"],
    days_long: ["Dé Domhnaigh", "Dé Luain", "Dé Máirt", "Dé Céadaoin", "Déardaoin", "Dé hAoine", "Dé Sathairn"],
    months: ["Ean", "Feabh", "Márta", "Aib", "Beal", "Meith", "Iúil", "Lún", "MFómh", "DFómh", "Samh", "Noll"],
    months_long: [
        "Eanáir", "Feabhra", "Márta", "Aibreán", "Bealtaine", "Meitheamh", "Iúil", "Lúnasa", "Meán Fómhair", "Deireadh Fómhair",
        "Samhain", "Nollaig",
    ],
};

fn names(lang: Lang) -> &'static Names {
    match lang {
        Lang::En => &EN,
        Lang::Fr => &FR,
        Lang::Es => &ES,
        Lang::De => &DE,
        Lang::It => &IT,
        Lang::Va => &VA,
        Lang::Ga => &GA,
    }
}

/// Forme d'une date complète dans une langue (jetons de [`format_time`]).
fn date_pattern(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "ddd mmm dd HH:nn:ss yyyy",
        Lang::De => "ddd d. mmm yyyy HH:nn:ss",
        _ => "ddd d mmm yyyy HH:nn:ss",
    }
}

/// Date et heure d'un instant dans la langue de l'interface, pour les textes affichés (« sujet posé par X le
/// sam. 17 oct. 2026 12:00:00 »). En anglais, comme `$asctime` de mIRC (`Sat Oct 17 12:00:00 2026`).
pub fn date_text(ms: i64, offset_secs: i32, lang: Lang) -> String {
    format_time_in(ms, offset_secs, date_pattern(lang), lang)
}

/// Lit une date ISO 8601 en UTC (`2026-10-17T10:00:00.000Z`, décalage `+02:00` accepté) → ms depuis 1970.
pub fn parse_iso8601(s: &str) -> Option<i64> {
    let s = s.trim();
    let b = s.as_bytes();
    if b.len() < 19 || b[4] != b'-' || b[7] != b'-' || !(b[10] == b'T' || b[10] == b' ') || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let num = |a: usize, z: usize| -> Option<i64> { s.get(a..z)?.parse::<i64>().ok() };
    let (y, mo, d, h, mi, se) = (num(0, 4)?, num(5, 7)?, num(8, 10)?, num(11, 13)?, num(14, 16)?, num(17, 19)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || se > 60 {
        return None;
    }
    let mut i = 19;
    let mut millis = 0i64;
    if b.get(i) == Some(&b'.') {
        i += 1;
        let st = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        let frac = &s[st..i];
        let mut v: String = frac.chars().take(3).collect();
        while v.len() < 3 {
            v.push('0');
        }
        millis = v.parse().unwrap_or(0);
    }
    let mut offset = 0i64;
    match b.get(i) {
        None | Some(b'Z') | Some(b'z') => {}
        Some(&c) if c == b'+' || c == b'-' => {
            let oh = num(i + 1, i + 3)?;
            let om = if b.get(i + 3) == Some(&b':') { num(i + 4, i + 6)? } else { num(i + 3, i + 5).unwrap_or(0) };
            offset = (oh * 3600 + om * 60) * if c == b'-' { -1 } else { 1 };
        }
        _ => return None,
    }
    let days = days_from_civil(y, mo as u32, d as u32);
    Some(((days * 86_400 + h * 3600 + mi * 60 + se - offset) * 1000) + millis)
}

/// Écrit un instant en ISO 8601 UTC avec millisecondes (`2026-10-17T10:00:00.000Z`).
pub fn format_iso8601(ms: i64) -> String {
    let c = civil(ms, 0);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        c.year, c.month, c.day, c.hour, c.minute, c.second, c.millis
    )
}

/// Date à la manière de `$asctime` de mIRC (`Sat Oct 17 12:00:00 2026`).
pub fn asctime(ms: i64, offset_secs: i32) -> String {
    format_time(ms, offset_secs, "ddd mmm dd HH:nn:ss yyyy")
}

/// Format d'heure de mIRC : `yyyy yy mmmm mmm mm m dddd ddd dd d HH H hh h nn n ss s TT tt T t`, le reste tel
/// quel. Le texte entre apostrophes n'est pas interprété. Noms des jours et des mois en anglais.
pub fn format_time(ms: i64, offset_secs: i32, fmt: &str) -> String {
    format_time_in(ms, offset_secs, fmt, Lang::En)
}

/// Comme [`format_time`], avec les noms des jours et des mois de `lang`.
pub fn format_time_in(ms: i64, offset_secs: i32, fmt: &str, lang: Lang) -> String {
    let nm = names(lang);
    let c = civil(ms, offset_secs);
    let chars: Vec<char> = fmt.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let run = |i: usize, ch: char| -> usize { chars[i..].iter().take_while(|&&x| x == ch).count() };
    while i < chars.len() {
        let ch = chars[i];
        match ch {
            '\'' => {
                i += 1;
                while i < chars.len() && chars[i] != '\'' {
                    out.push(chars[i]);
                    i += 1;
                }
                i += 1;
            }
            'y' => {
                let n = run(i, 'y');
                if n >= 3 {
                    out.push_str(&format!("{:04}", c.year));
                } else {
                    out.push_str(&format!("{:02}", c.year.rem_euclid(100)));
                }
                i += n;
            }
            'm' => {
                let n = run(i, 'm');
                match n {
                    1 => out.push_str(&c.month.to_string()),
                    2 => out.push_str(&format!("{:02}", c.month)),
                    3 => out.push_str(nm.months[(c.month - 1) as usize]),
                    _ => out.push_str(nm.months_long[(c.month - 1) as usize]),
                }
                i += n;
            }
            'd' => {
                let n = run(i, 'd');
                match n {
                    1 => out.push_str(&c.day.to_string()),
                    2 => out.push_str(&format!("{:02}", c.day)),
                    3 => out.push_str(nm.days[c.weekday as usize]),
                    _ => out.push_str(nm.days_long[c.weekday as usize]),
                }
                i += n;
            }
            'H' => {
                let n = run(i, 'H');
                if n >= 2 {
                    out.push_str(&format!("{:02}", c.hour));
                } else {
                    out.push_str(&c.hour.to_string());
                }
                i += n;
            }
            'h' => {
                let n = run(i, 'h');
                let h12 = if c.hour.is_multiple_of(12) { 12 } else { c.hour % 12 };
                if n >= 2 {
                    out.push_str(&format!("{h12:02}"));
                } else {
                    out.push_str(&h12.to_string());
                }
                i += n;
            }
            'n' => {
                let n = run(i, 'n');
                if n >= 2 {
                    out.push_str(&format!("{:02}", c.minute));
                } else {
                    out.push_str(&c.minute.to_string());
                }
                i += n;
            }
            's' => {
                let n = run(i, 's');
                if n >= 2 {
                    out.push_str(&format!("{:02}", c.second));
                } else {
                    out.push_str(&c.second.to_string());
                }
                i += n;
            }
            'T' | 't' => {
                let n = run(i, ch);
                let pm = c.hour >= 12;
                let s = match (n >= 2, pm) {
                    (true, true) => "pm",
                    (true, false) => "am",
                    (false, true) => "p",
                    (false, false) => "a",
                };
                if ch == 'T' {
                    out.push_str(&s.to_uppercase());
                } else {
                    out.push_str(s);
                }
                i += n;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// Durée à la manière de `$duration` de mIRC : `1wk 2days 3hrs 5mins 3secs`.
pub fn duration(total_secs: i64) -> String {
    let mut t = total_secs.max(0);
    let parts = [(604_800, "wk", "wks"), (86_400, "day", "days"), (3600, "hr", "hrs"), (60, "min", "mins"), (1, "sec", "secs")];
    let mut out = Vec::new();
    for (unit, one, many) in parts {
        let n = t / unit;
        t %= unit;
        if n > 0 {
            out.push(format!("{n}{}", if n == 1 { one } else { many }));
        }
    }
    if out.is_empty() {
        "0secs".into()
    } else {
        out.join(" ")
    }
}

/// Décalage de l'heure locale sur UTC (secondes) à un instant, selon le système ; 0 s'il est inconnu.
pub fn local_offset(ms: i64) -> i32 {
    time::OffsetDateTime::from_unix_timestamp(ms.div_euclid(1000))
        .ok()
        .and_then(|t| time::UtcOffset::local_offset_at(t).ok())
        .map(|o| o.whole_seconds())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_roundtrip() {
        let ms = parse_iso8601("2026-10-17T10:00:00.123Z").unwrap();
        assert_eq!(format_iso8601(ms), "2026-10-17T10:00:00.123Z");
        assert_eq!(parse_iso8601("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_iso8601("1970-01-01T02:00:00+02:00"), Some(0));
        assert_eq!(parse_iso8601("garbage"), None);
    }

    #[test]
    fn mirc_formats() {
        let ms = parse_iso8601("2026-10-17T09:05:03Z").unwrap();
        assert_eq!(format_time(ms, 0, "[HH:nn]"), "[09:05]");
        assert_eq!(format_time(ms, 7200, "[HH:nn:ss]"), "[11:05:03]");
        assert_eq!(asctime(ms, 0), "Sat Oct 17 09:05:03 2026");
        assert_eq!(format_time(ms, 0, "h:nn TT 'at' yy"), "9:05 AM at 26");
        assert_eq!(duration(303), "5mins 3secs");
        assert_eq!(duration(0), "0secs");
        assert_eq!(duration(90061), "1day 1hr 1min 1sec");
    }

    #[test]
    fn dates_in_seven_languages() {
        // Samedi 17 octobre 2026, 09:05:03 UTC.
        let ms = parse_iso8601("2026-10-17T09:05:03Z").unwrap();
        assert_eq!(date_text(ms, 0, Lang::En), "Sat Oct 17 09:05:03 2026");
        assert_eq!(date_text(ms, 0, Lang::En), asctime(ms, 0));
        assert_eq!(date_text(ms, 0, Lang::Fr), "sam. 17 oct. 2026 09:05:03");
        assert_eq!(date_text(ms, 0, Lang::Es), "sáb 17 oct 2026 09:05:03");
        assert_eq!(date_text(ms, 0, Lang::De), "Sa. 17. Okt. 2026 09:05:03");
        assert_eq!(date_text(ms, 0, Lang::It), "sab 17 ott 2026 09:05:03");
        assert_eq!(date_text(ms, 0, Lang::Va), "ds. 17 oct. 2026 09:05:03");
        assert_eq!(date_text(ms, 0, Lang::Ga), "Sath 17 DFómh 2026 09:05:03");
        // Décalage horaire et noms longs.
        assert_eq!(format_time_in(ms, 7200, "dddd d mmmm", Lang::Fr), "samedi 17 octobre");
        assert_eq!(format_time_in(ms, 0, "dddd d mmmm", Lang::Ga), "Dé Sathairn 17 Deireadh Fómhair");
        let feb = parse_iso8601("2026-02-02T00:00:00Z").unwrap();
        assert_eq!(format_time_in(feb, 0, "dddd d mmmm", Lang::Es), "lunes 2 febrero");
        assert_eq!(format_time_in(feb, 0, "ddd d mmm", Lang::Va), "dl. 2 febr.");
        assert_eq!(format_time_in(feb, 0, "dddd d. mmmm", Lang::De), "Montag 2. Februar");
        assert_eq!(format_time_in(feb, 0, "dddd d mmmm", Lang::It), "lunedì 2 febbraio");
        // Chaque langue a ses douze mois et sept jours, tous non vides.
        for l in Lang::ALL {
            let n = names(l);
            let mut all = n.days.iter().chain(n.days_long.iter()).chain(n.months.iter()).chain(n.months_long.iter());
            assert!(all.all(|s| !s.is_empty()));
        }
    }
}
