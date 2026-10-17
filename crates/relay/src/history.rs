//! Historique des conversations, pour le rattrapage (`draft/chathistory`).
//!
//! Rangement : `<données>/history/<utilisateur>/<netid>/<cible>/<AAAA-MM-JJ>.jsonl`, un fichier par jour (UTC)
//! et par cible (canal, ou l'autre personne d'un privé ; nom replié selon CASEMAPPING puis échappé en `%XX`),
//! une ligne JSON par message, ajoutée au bout du fichier ([`Entry`]).
//!
//! Les heures d'une cible ne reculent jamais (un message daté avant le précédent prend l'heure du précédent) :
//! l'ordre des lignes est l'ordre du temps, d'un fichier au suivant, ce qui rend les recherches simples. Le
//! `msgid` d'un message contient son heure ([`crate::ids`]) : on retrouve son fichier sans tout parcourir.
//!
//! Les bornes des requêtes sont exclues, comme le veut la spécification ; une borne `timestamp=` exclut les
//! messages de cette milliseconde, une borne `msgid=` exclut ce message seulement.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use alpirc_proto::{Message, Source, Tags};
use serde::{Deserialize, Serialize};

use crate::ids::{MsgIdGen, day_name, day_of, format_time, msgid_time, parse_day_name};

/// Un message gardé.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// `msgid`.
    pub id: String,
    /// Heure (ms depuis 1970).
    pub t: i64,
    /// Nom de la cible tel qu'il était (canal, ou l'autre personne d'un privé).
    pub buf: String,
    /// `pseudo!ident@hôte` de l'auteur.
    #[serde(default)]
    pub src: String,
    /// `PRIVMSG`, `NOTICE` ou `TAGMSG`.
    pub cmd: String,
    #[serde(default)]
    pub params: Vec<String>,
    /// Étiquettes des clients (`+draft/reply`…).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tags: BTreeMap<String, String>,
}

impl Entry {
    /// La ligne à rejouer, avec `time` et `msgid`.
    pub fn to_message(&self) -> Message {
        let mut tags = Tags(self.tags.iter().map(|(k, v)| (k.clone(), (!v.is_empty()).then(|| v.clone()))).collect());
        tags.set("time", Some(&format_time(self.t)));
        tags.set("msgid", Some(&self.id));
        Message {
            tags,
            source: (!self.src.is_empty()).then(|| Source::new(self.src.clone())),
            command: self.cmd.clone(),
            params: self.params.clone(),
        }
    }
}

/// Repère d'une requête.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ref {
    Time(i64),
    Msgid(String),
}

impl Ref {
    /// `timestamp=…` ou `msgid=…`.
    pub fn parse(s: &str) -> Option<Ref> {
        if let Some(t) = s.strip_prefix("timestamp=") {
            return crate::ids::parse_time(t).map(Ref::Time);
        }
        s.strip_prefix("msgid=").filter(|m| !m.is_empty()).map(|m| Ref::Msgid(m.to_string()))
    }
}

/// Une requête `CHATHISTORY` sur une cible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Query {
    /// Les plus récents (après le repère s'il y en a un).
    Latest(Option<Ref>),
    Before(Ref),
    After(Ref),
    /// Autour : la moitié avant (exclu), le reste à partir du repère (inclus).
    Around(Ref),
    /// Entre deux repères (exclus), en partant du premier.
    Between(Ref, Ref),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryError {
    /// `msgid` inconnu (ou d'une autre origine).
    UnknownMsgid,
}

/// Dernière activité d'une cible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetInfo {
    pub name: String,
    pub last: i64,
}

/// Réglages de l'historique.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryOptions {
    pub enabled: bool,
    /// Jours gardés (0 : sans limite).
    pub retention_days: u32,
    /// Lignes gardées par cible (0 : sans limite).
    pub max_lines_per_target: usize,
}

impl Default for HistoryOptions {
    fn default() -> Self {
        HistoryOptions { enabled: true, retention_days: 0, max_lines_per_target: 0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Pos {
    day: i64,
    idx: usize,
}

/// Borne d'un parcours : une heure, ou la place exacte d'un message.
#[derive(Debug, Clone, Copy)]
struct Bound {
    t: i64,
    pos: Option<Pos>,
    inclusive: bool,
}

impl Bound {
    fn day(&self) -> i64 {
        self.pos.map(|p| p.day).unwrap_or_else(|| day_of(self.t))
    }
    /// Place d'un message par rapport à la borne.
    fn cmp_entry(&self, pos: Pos, t: i64) -> Ordering {
        match self.pos {
            Some(p) => pos.cmp(&p),
            None => t.cmp(&self.t),
        }
    }
    fn below(&self, pos: Pos, t: i64) -> bool {
        let o = self.cmp_entry(pos, t);
        o == Ordering::Less || (self.inclusive && o == Ordering::Equal)
    }
    fn above(&self, pos: Pos, t: i64) -> bool {
        let o = self.cmp_entry(pos, t);
        o == Ordering::Greater || (self.inclusive && o == Ordering::Equal)
    }
    fn order(&self, other: &Bound) -> Ordering {
        match (self.pos, other.pos) {
            (Some(a), Some(b)) => a.cmp(&b),
            _ => self.t.cmp(&other.t),
        }
    }
}

type Scope = (String, String);

/// L'historique de tous les utilisateurs.
pub struct HistoryStore {
    /// `<données>/history`.
    root: PathBuf,
    opts: HistoryOptions,
    ids: MsgIdGen,
    /// Par (utilisateur, réseau) : cible repliée → dernière activité (chargé à la demande).
    index: HashMap<Scope, BTreeMap<String, TargetInfo>>,
}

impl HistoryStore {
    pub fn new(root: PathBuf, opts: HistoryOptions) -> HistoryStore {
        HistoryStore { root, opts, ids: MsgIdGen::new(), index: HashMap::new() }
    }

    pub fn options(&self) -> &HistoryOptions {
        &self.opts
    }

    /// Un `msgid` pour un message qui n'est pas gardé.
    pub fn new_id(&mut self, t: i64) -> String {
        self.ids.next(t)
    }

    fn scope_dir(&self, user: &str, net: &str) -> PathBuf {
        self.root.join(escape(user)).join(escape(net))
    }

    fn target_dir(&self, user: &str, net: &str, target: &str) -> PathBuf {
        self.scope_dir(user, net).join(escape(target))
    }

    fn index_mut(&mut self, user: &str, net: &str) -> &mut BTreeMap<String, TargetInfo> {
        let key = (user.to_string(), net.to_string());
        if !self.index.contains_key(&key) {
            let loaded = if self.opts.enabled { load_index(&self.scope_dir(user, net)) } else { BTreeMap::new() };
            self.index.insert(key.clone(), loaded);
        }
        self.index.get_mut(&key).expect("présent")
    }

    /// Range un message (cible déjà repliée dans `target`, nom affiché dans `buf`). L'heure proposée est
    /// avancée au besoin pour ne pas précéder le dernier message de la cible. Rend (`msgid`, heure retenue).
    #[allow(clippy::too_many_arguments)]
    pub fn append(
        &mut self,
        user: &str,
        net: &str,
        target: &str,
        buf: &str,
        proposed: i64,
        src: &str,
        cmd: &str,
        params: &[String],
        tags: BTreeMap<String, String>,
    ) -> (String, i64) {
        if !self.opts.enabled {
            return (self.ids.next(proposed), proposed);
        }
        let last = self.index_mut(user, net).get(target).map(|t| t.last).unwrap_or(i64::MIN);
        let t = proposed.max(last);
        let id = self.ids.next(t);
        let entry = Entry {
            id: id.clone(),
            t,
            buf: buf.to_string(),
            src: src.to_string(),
            cmd: cmd.to_string(),
            params: params.to_vec(),
            tags,
        };
        let dir = self.target_dir(user, net, target);
        if let Err(e) = append_line(&dir, day_of(t), &entry) {
            log::error!("historique {} : {e}", dir.display());
        }
        self.index_mut(user, net).insert(target.to_string(), TargetInfo { name: buf.to_string(), last: t });
        (id, t)
    }

    /// Nom affiché d'une cible déjà vue.
    pub fn target_name(&mut self, user: &str, net: &str, target: &str) -> Option<String> {
        if !self.opts.enabled {
            return None;
        }
        self.index_mut(user, net).get(target).map(|t| t.name.clone())
    }

    /// Messages d'une cible, dans l'ordre du temps (`limit` au plus).
    pub fn query(
        &mut self,
        user: &str,
        net: &str,
        target: &str,
        q: &Query,
        limit: usize,
    ) -> Result<Vec<Entry>, HistoryError> {
        if !self.opts.enabled || limit == 0 {
            return Ok(Vec::new());
        }
        let mut files = Files::open(self.target_dir(user, net, target));
        let excl = |files: &mut Files, r: &Ref| files.bound(r, false);
        Ok(match q {
            Query::Latest(None) => files.scan_back(None, None, limit),
            Query::Latest(Some(r)) => {
                let b = excl(&mut files, r)?;
                files.scan_back(None, Some(b), limit)
            }
            Query::Before(r) => {
                let b = excl(&mut files, r)?;
                files.scan_back(Some(b), None, limit)
            }
            Query::After(r) => {
                let b = excl(&mut files, r)?;
                files.scan_fwd(Some(b), None, limit)
            }
            Query::Around(r) => {
                let before_bound = excl(&mut files, r)?;
                let from_bound = files.bound(r, true)?;
                let mut v = files.scan_back(Some(before_bound), None, limit / 2);
                let rest = limit - v.len();
                v.extend(files.scan_fwd(Some(from_bound), None, rest));
                v
            }
            Query::Between(a, b) => {
                let ba = excl(&mut files, a)?;
                let bb = excl(&mut files, b)?;
                if ba.order(&bb) != Ordering::Greater {
                    files.scan_fwd(Some(ba), Some(bb), limit)
                } else {
                    files.scan_back(Some(ba), Some(bb), limit)
                }
            }
        })
    }

    /// `CHATHISTORY TARGETS` : cibles dont le dernier message est entre `a` et `b` (exclus), au plus `limit`
    /// en partant de `a`, rendues dans l'ordre du temps.
    pub fn targets(&mut self, user: &str, net: &str, a: i64, b: i64, limit: usize) -> Vec<TargetInfo> {
        if !self.opts.enabled {
            return Vec::new();
        }
        let (lo, hi) = (a.min(b), a.max(b));
        let mut v: Vec<TargetInfo> =
            self.index_mut(user, net).values().filter(|t| t.last > lo && t.last < hi).cloned().collect();
        v.sort_by(|x, y| x.last.cmp(&y.last).then_with(|| x.name.cmp(&y.name)));
        if a <= b {
            v.truncate(limit);
        } else if v.len() > limit {
            v.drain(..v.len() - limit);
        }
        v
    }

    /// Ménage : fichiers plus vieux que la durée gardée, lignes au-delà du plafond par cible. Rend le nombre
    /// de fichiers retirés ou raccourcis.
    pub fn cleanup(&mut self, now: i64) -> usize {
        if !self.opts.enabled || (self.opts.retention_days == 0 && self.opts.max_lines_per_target == 0) {
            return 0;
        }
        let mut touched = 0;
        let cutoff = (self.opts.retention_days > 0).then(|| day_of(now) - self.opts.retention_days as i64);
        for user_dir in subdirs(&self.root) {
            for net_dir in subdirs(&user_dir) {
                for target_dir in subdirs(&net_dir) {
                    touched += cleanup_target(&target_dir, cutoff, self.opts.max_lines_per_target);
                    if std::fs::read_dir(&target_dir).map(|mut d| d.next().is_none()).unwrap_or(false) {
                        let _ = std::fs::remove_dir(&target_dir);
                    }
                }
            }
        }
        // L'index sera relu.
        self.index.clear();
        touched
    }
}

/// Les fichiers d'une cible, lus à la demande pendant une requête.
struct Files {
    dir: PathBuf,
    days: Vec<i64>,
    cache: HashMap<i64, Vec<Entry>>,
}

impl Files {
    fn open(dir: PathBuf) -> Files {
        let days = list_days(&dir);
        Files { dir, days, cache: HashMap::new() }
    }

    fn load(&mut self, day: i64) -> &Vec<Entry> {
        let dir = &self.dir;
        self.cache.entry(day).or_insert_with(|| read_day(dir, day))
    }

    fn bound(&mut self, r: &Ref, inclusive: bool) -> Result<Bound, HistoryError> {
        match r {
            Ref::Time(t) => Ok(Bound { t: *t, pos: None, inclusive }),
            Ref::Msgid(id) => {
                let t = msgid_time(id).ok_or(HistoryError::UnknownMsgid)?;
                let day = day_of(t);
                let idx = self.load(day).iter().position(|e| e.id == *id).ok_or(HistoryError::UnknownMsgid)?;
                Ok(Bound { t, pos: Some(Pos { day, idx }), inclusive })
            }
        }
    }

    /// En remontant le temps depuis `upper` (exclu sauf borne inclusive) jusqu'à `lower` : les `limit` plus
    /// proches de `upper`, rendus dans l'ordre du temps.
    fn scan_back(&mut self, upper: Option<Bound>, lower: Option<Bound>, limit: usize) -> Vec<Entry> {
        let mut out = Vec::new();
        if limit == 0 {
            return out;
        }
        let days = self.days.clone();
        'days: for &day in days.iter().rev() {
            if upper.is_some_and(|u| day > u.day()) {
                continue;
            }
            if lower.is_some_and(|l| day < l.day()) {
                break;
            }
            let entries = self.load(day);
            for (idx, e) in entries.iter().enumerate().rev() {
                let pos = Pos { day, idx };
                if upper.is_some_and(|u| !u.below(pos, e.t)) {
                    continue;
                }
                if lower.is_some_and(|l| !l.above(pos, e.t)) {
                    break 'days;
                }
                out.push(e.clone());
                if out.len() >= limit {
                    break 'days;
                }
            }
        }
        out.reverse();
        out
    }

    /// En avançant depuis `lower` jusqu'à `upper` : les `limit` plus proches de `lower`.
    fn scan_fwd(&mut self, lower: Option<Bound>, upper: Option<Bound>, limit: usize) -> Vec<Entry> {
        let mut out = Vec::new();
        if limit == 0 {
            return out;
        }
        let days = self.days.clone();
        'days: for &day in days.iter() {
            if lower.is_some_and(|l| day < l.day()) {
                continue;
            }
            if upper.is_some_and(|u| day > u.day()) {
                break;
            }
            let entries = self.load(day);
            for (idx, e) in entries.iter().enumerate() {
                let pos = Pos { day, idx };
                if lower.is_some_and(|l| !l.above(pos, e.t)) {
                    continue;
                }
                if upper.is_some_and(|u| !u.below(pos, e.t)) {
                    break 'days;
                }
                out.push(e.clone());
                if out.len() >= limit {
                    break 'days;
                }
            }
        }
        out
    }
}

/// Nom de fichier sûr : lettres, chiffres, `-`, `_` gardés, le reste en `%XX` (un `.` aussi, pour éviter
/// `.` et `..`).
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    if out.is_empty() {
        out.push_str("%00");
    }
    out
}

/// Inverse de [`escape`].
pub fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Some(v) = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            if v != 0 {
                out.push(v);
            }
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()).collect();
    v.sort();
    v
}

/// Jours présents dans le dossier d'une cible, dans l'ordre.
fn list_days(dir: &Path) -> Vec<i64> {
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut v: Vec<i64> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".jsonl")).and_then(parse_day_name))
        .collect();
    v.sort_unstable();
    v
}

fn day_path(dir: &Path, day: i64) -> PathBuf {
    dir.join(format!("{}.jsonl", day_name(day)))
}

fn read_day(dir: &Path, day: i64) -> Vec<Entry> {
    let Ok(f) = std::fs::File::open(day_path(dir, day)) else { return Vec::new() };
    std::io::BufReader::new(f)
        .lines()
        .map_while(Result::ok)
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<Entry>(&l).ok())
        .collect()
}

fn append_line(dir: &Path, day: i64, e: &Entry) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut line = serde_json::to_string(e).map_err(std::io::Error::other)?;
    line.push('\n');
    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    // Une seule écriture par ligne : une ligne n'est jamais coupée par une autre.
    opts.open(day_path(dir, day))?.write_all(line.as_bytes())
}

/// Index d'un (utilisateur, réseau) : la dernière ligne lisible de chaque cible.
fn load_index(scope: &Path) -> BTreeMap<String, TargetInfo> {
    let mut out = BTreeMap::new();
    for target_dir in subdirs(scope) {
        let Some(name) = target_dir.file_name().and_then(|n| n.to_str()) else { continue };
        let fold = unescape(name);
        for day in list_days(&target_dir).into_iter().rev() {
            if let Some(e) = read_day(&target_dir, day).pop() {
                out.insert(fold.clone(), TargetInfo { name: e.buf, last: e.t });
                break;
            }
        }
    }
    out
}

fn count_lines(path: &Path) -> usize {
    std::fs::File::open(path)
        .map(|f| std::io::BufReader::new(f).lines().map_while(Result::ok).filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0)
}

fn cleanup_target(dir: &Path, cutoff: Option<i64>, max_lines: usize) -> usize {
    let mut touched = 0;
    let mut days = list_days(dir);
    if let Some(c) = cutoff {
        for d in days.iter().filter(|d| **d < c) {
            if std::fs::remove_file(day_path(dir, *d)).is_ok() {
                touched += 1;
            }
        }
        days.retain(|d| *d >= c);
    }
    if max_lines == 0 {
        return touched;
    }
    let mut total = 0usize;
    for (i, &d) in days.iter().enumerate().rev() {
        let n = count_lines(&day_path(dir, d));
        if total + n <= max_lines {
            total += n;
            continue;
        }
        // Ce fichier dépasse : on garde ses dernières lignes, et on retire tous les plus anciens.
        let keep = max_lines - total;
        let path = day_path(dir, d);
        if keep == 0 {
            let _ = std::fs::remove_file(&path);
        } else {
            let lines: Vec<String> = std::fs::read_to_string(&path)
                .unwrap_or_default()
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(str::to_string)
                .collect();
            let start = lines.len().saturating_sub(keep);
            let mut text = lines[start..].join("\n");
            text.push('\n');
            let mut tmp = path.as_os_str().to_owned();
            tmp.push(".tmp");
            let tmp = PathBuf::from(tmp);
            if crate::config::write_private(&tmp, &text).and_then(|_| std::fs::rename(&tmp, &path)).is_err() {
                log::warn!("historique : {} non raccourci", path.display());
            }
        }
        touched += 1;
        for &old in &days[..i] {
            if std::fs::remove_file(day_path(dir, old)).is_ok() {
                touched += 1;
            }
        }
        break;
    }
    touched
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::parse_time;

    struct T {
        dir: PathBuf,
        h: HistoryStore,
    }

    impl T {
        fn new(name: &str, opts: HistoryOptions) -> T {
            let dir = std::env::temp_dir().join(format!("alpirc-relay-hist-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            T { h: HistoryStore::new(dir.clone(), opts), dir }
        }
        fn add(&mut self, target: &str, t: i64, text: &str) -> (String, i64) {
            self.h.append(
                "gui",
                "1",
                &target.to_lowercase(),
                target,
                t,
                "Bob!b@h",
                "PRIVMSG",
                &[target.to_string(), text.to_string()],
                BTreeMap::new(),
            )
        }
        fn q(&mut self, target: &str, q: Query, limit: usize) -> Vec<String> {
            self.h.query("gui", "1", target, &q, limit).unwrap().into_iter().map(|e| e.params[1].clone()).collect()
        }
    }

    impl Drop for T {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    const DAY: i64 = 86_400_000;
    fn t0() -> i64 {
        parse_time("2026-10-15T12:00:00.000Z").unwrap()
    }

    /// m0…m9 : deux par jour sur cinq jours, plus deux à la même milliseconde.
    fn filled(name: &str) -> (T, Vec<(String, i64)>) {
        let mut t = T::new(name, HistoryOptions::default());
        let mut ids = Vec::new();
        for i in 0..10 {
            ids.push(t.add("#Lyon", t0() + (i / 2) * DAY + (i % 2) * 1000, &format!("m{i}")));
        }
        (t, ids)
    }

    fn names(r: std::ops::Range<usize>) -> Vec<String> {
        r.map(|i| format!("m{i}")).collect()
    }

    #[test]
    fn append_and_files() {
        let (t, ids) = filled("files");
        let dir = t.dir.join("gui").join("1").join("%23lyon");
        assert_eq!(list_days(&dir).len(), 5);
        let first = read_day(&dir, day_of(t0()));
        assert_eq!(first.len(), 2);
        assert_eq!(first[0].id, ids[0].0);
        assert_eq!(first[0].buf, "#Lyon");
        assert_eq!(first[1].t, t0() + 1000);
        let m = first[0].to_message();
        assert_eq!(m.tags.get("msgid"), Some(ids[0].0.as_str()));
        assert_eq!(m.tags.get("time"), Some("2026-10-15T12:00:00.000Z"));
        assert_eq!(m.to_line().split_once(' ').unwrap().1, ":Bob!b@h PRIVMSG #Lyon m0");
    }

    #[test]
    fn time_never_goes_back() {
        let mut t = T::new("mono", HistoryOptions::default());
        let (_, a) = t.add("#c", 5000, "a");
        let (_, b) = t.add("#c", 4000, "b");
        assert_eq!((a, b), (5000, 5000));
        let (_, c) = t.add("#autre", 1000, "c");
        assert_eq!(c, 1000, "par cible");
    }

    #[test]
    fn latest() {
        let (mut t, ids) = filled("latest");
        assert_eq!(t.q("#lyon", Query::Latest(None), 3), names(7..10));
        assert_eq!(t.q("#lyon", Query::Latest(None), 100), names(0..10));
        // Après un repère (exclu).
        assert_eq!(t.q("#lyon", Query::Latest(Some(Ref::Time(ids[6].1))), 100), names(7..10));
        assert_eq!(t.q("#lyon", Query::Latest(Some(Ref::Msgid(ids[6].0.clone()))), 2), names(8..10));
        assert_eq!(t.q("#lyon", Query::Latest(Some(Ref::Msgid(ids[9].0.clone()))), 2), Vec::<String>::new());
        assert!(t.q("#inconnu", Query::Latest(None), 10).is_empty());
    }

    #[test]
    fn before_after() {
        let (mut t, ids) = filled("ba");
        assert_eq!(t.q("#lyon", Query::Before(Ref::Msgid(ids[5].0.clone())), 3), names(2..5));
        assert_eq!(t.q("#lyon", Query::Before(Ref::Time(ids[5].1)), 100), names(0..5));
        assert_eq!(t.q("#lyon", Query::Before(Ref::Time(ids[5].1 + 1)), 100), names(0..6));
        assert_eq!(t.q("#lyon", Query::After(Ref::Msgid(ids[2].0.clone())), 3), names(3..6));
        assert_eq!(t.q("#lyon", Query::After(Ref::Time(ids[2].1 - 1)), 2), names(2..4));
        assert_eq!(t.q("#lyon", Query::After(Ref::Time(t0() - 10 * DAY)), 1), names(0..1));
        assert!(t.q("#lyon", Query::After(Ref::Time(t0() + 100 * DAY)), 10).is_empty());
        assert!(t.q("#lyon", Query::Before(Ref::Time(t0() - DAY)), 10).is_empty());
    }

    #[test]
    fn same_millisecond_msgids() {
        let mut t = T::new("same", HistoryOptions::default());
        let a = t.add("#c", 7000, "a");
        let b = t.add("#c", 7000, "b");
        let c = t.add("#c", 7000, "c");
        assert_eq!(t.q("#c", Query::After(Ref::Msgid(a.0.clone())), 10), vec!["b", "c"]);
        assert_eq!(t.q("#c", Query::Before(Ref::Msgid(c.0.clone())), 10), vec!["a", "b"]);
        assert_eq!(t.q("#c", Query::Between(Ref::Msgid(a.0.clone()), Ref::Msgid(c.0.clone())), 10), vec!["b"]);
        // Une borne d'heure exclut toute la milliseconde.
        assert!(t.q("#c", Query::After(Ref::Time(7000)), 10).is_empty());
        assert_eq!(t.q("#c", Query::Around(Ref::Msgid(b.0.clone())), 3), vec!["a", "b", "c"]);
    }

    #[test]
    fn around_between() {
        let (mut t, ids) = filled("ab");
        assert_eq!(t.q("#lyon", Query::Around(Ref::Msgid(ids[5].0.clone())), 4), names(3..7));
        assert_eq!(t.q("#lyon", Query::Around(Ref::Time(ids[5].1)), 3), names(4..7));
        assert_eq!(t.q("#lyon", Query::Around(Ref::Msgid(ids[0].0.clone())), 4), names(0..4));
        // Entre, dans les deux sens.
        assert_eq!(
            t.q("#lyon", Query::Between(Ref::Msgid(ids[1].0.clone()), Ref::Msgid(ids[8].0.clone())), 100),
            names(2..8)
        );
        assert_eq!(
            t.q("#lyon", Query::Between(Ref::Msgid(ids[1].0.clone()), Ref::Msgid(ids[8].0.clone())), 2),
            names(2..4)
        );
        assert_eq!(
            t.q("#lyon", Query::Between(Ref::Msgid(ids[8].0.clone()), Ref::Msgid(ids[1].0.clone())), 2),
            names(6..8)
        );
        assert_eq!(t.q("#lyon", Query::Between(Ref::Time(ids[1].1), Ref::Time(ids[8].1)), 100), names(2..8));
        assert_eq!(t.q("#lyon", Query::Between(Ref::Time(ids[8].1), Ref::Msgid(ids[1].0.clone())), 3), names(5..8));
        assert!(t.q("#lyon", Query::Between(Ref::Time(ids[3].1), Ref::Time(ids[3].1)), 10).is_empty());
    }

    #[test]
    fn unknown_msgid() {
        let (mut t, _) = filled("unk");
        let mut g = MsgIdGen::with_seed(0, 0);
        let foreign = g.next(t0());
        assert_eq!(
            t.h.query("gui", "1", "#lyon", &Query::Before(Ref::Msgid(foreign)), 10),
            Err(HistoryError::UnknownMsgid)
        );
        assert_eq!(
            t.h.query("gui", "1", "#lyon", &Query::Before(Ref::Msgid("abc".into())), 10),
            Err(HistoryError::UnknownMsgid)
        );
    }

    #[test]
    fn targets_index() {
        let (mut t, ids) = filled("targets");
        t.add("Alice", ids[9].1 + 10, "privé");
        t.add("#Grenoble", t0() - DAY, "vieux");
        let all = t.h.targets("gui", "1", 0, i64::MAX, 10);
        assert_eq!(all.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), vec!["#Grenoble", "#Lyon", "Alice"]);
        assert_eq!(all[1].last, ids[9].1);
        let rev = t.h.targets("gui", "1", i64::MAX, 0, 2);
        assert_eq!(rev.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(), vec!["#Lyon", "Alice"]);
        let first = t.h.targets("gui", "1", 0, i64::MAX, 1);
        assert_eq!(first[0].name, "#Grenoble");
        // Relu depuis le disque.
        let mut h2 = HistoryStore::new(t.dir.clone(), HistoryOptions::default());
        let all2 = h2.targets("gui", "1", 0, i64::MAX, 10);
        assert_eq!(all, all2);
        assert_eq!(h2.target_name("gui", "1", "alice").as_deref(), Some("Alice"));
        assert!(h2.targets("gui", "2", 0, i64::MAX, 10).is_empty());
    }

    #[test]
    fn retention() {
        let (mut t, ids) = filled("ret");
        t.h.opts.retention_days = 2;
        // « Maintenant » = jour de m8/m9 : on garde ce jour et les deux d'avant.
        let n = t.h.cleanup(ids[9].1);
        assert_eq!(n, 2);
        assert_eq!(t.q("#lyon", Query::Latest(None), 100), names(4..10));
        t.h.opts.retention_days = 0;
        t.h.opts.max_lines_per_target = 3;
        t.h.cleanup(ids[9].1);
        assert_eq!(t.q("#lyon", Query::Latest(None), 100), names(7..10));
        // Un msgid d'un fichier raccourci se retrouve encore.
        assert_eq!(t.q("#lyon", Query::After(Ref::Msgid(ids[7].0.clone())), 10), names(8..10));
        // Une cible vidée disparaît de l'index.
        t.h.opts.retention_days = 1;
        t.h.opts.max_lines_per_target = 0;
        t.h.cleanup(ids[9].1 + 10 * DAY);
        assert!(t.h.targets("gui", "1", 0, i64::MAX, 10).is_empty());
        assert!(!t.dir.join("gui").join("1").join("%23lyon").exists());
    }

    #[test]
    fn disabled() {
        let mut t = T::new("off", HistoryOptions { enabled: false, ..HistoryOptions::default() });
        let (id, time) = t.add("#c", 42, "x");
        assert_eq!(msgid_time(&id), Some(42));
        assert_eq!(time, 42);
        assert!(t.q("#c", Query::Latest(None), 10).is_empty());
        assert!(!t.dir.exists());
    }

    #[test]
    fn escaping() {
        for s in ["#lyon", "bob", "#a/b..", ".", "..", "#é", "x%y", ""] {
            let e = escape(s);
            assert!(e.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'%'), "{e}");
            assert_eq!(unescape(&e), s);
        }
        assert_eq!(escape("#lyon"), "%23lyon");
    }

    #[test]
    fn corrupt_lines_are_skipped() {
        let mut t = T::new("corrupt", HistoryOptions::default());
        t.add("#c", 1000, "a");
        let dir = t.dir.join("gui").join("1").join("%23c");
        let p = day_path(&dir, day_of(1000));
        let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
        f.write_all(b"{pas du json\n\n").unwrap();
        t.add("#c", 2000, "b");
        assert_eq!(t.q("#c", Query::Latest(None), 10), vec!["a", "b"]);
    }
}
