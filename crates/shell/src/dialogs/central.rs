//! Channel Central (`/channel`, menu du canal « Modes du canal ») : le sujet, les modes du canal (t, n, i, m,
//! p, s, clé, limite) et les listes de bans, d'exceptions et d'invitations, qui se remplissent au fil des
//! réponses du serveur. OK ou Appliquer n'envoie que ce qui a changé (`/topic`, `/mode #canal +k clé -b
//! masque`…). Sans le rang d'opérateur, tout est montré mais rien n'est modifiable (le sujet reste modifiable
//! si le canal n'est pas `+t`).

use std::cell::RefCell;
use std::rc::Rc;

use alpirc_core::i18n::Lang;
use alpirc_core::session::{ChannelState, ListEntry};
use alpirc_ui::{ControlKind, ControlSpec, DialogMsg, DialogSpec, DialogView};

use super::form::{Field, Form, Val};
use super::*;

const W: i32 = 560;
const H: i32 = 480;

const OK: u32 = 1;
const CANCEL: u32 = 2;
const APPLY: u32 = 3;
const NOT_OP: u32 = 4;
const TOPIC: u32 = 11;
const SETBY: u32 = 12;
/// Cases des modes simples : (identifiant, lettre).
const FLAGS: [(u32, char); 6] = [(21, 't'), (22, 'n'), (23, 'i'), (24, 'm'), (25, 'p'), (26, 's')];
const KEY_ON: u32 = 27;
const KEY: u32 = 28;
const LIMIT_ON: u32 = 29;
const LIMIT: u32 = 30;
/// Onglets des listes : (onglet, liste, ajouter, modifier, retirer).
const LISTS: [(u32, u32, u32, u32, u32); 3] = [(40, 41, 42, 43, 44), (50, 51, 52, 53, 54), (60, 61, 62, 63, 64)];

/// Modes montrés.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Modes {
    flags: [bool; 6],
    key: Option<String>,
    limit: Option<u32>,
}

/// Un changement de liste en attente : (numéro de la liste, ajout, masque).
type Pending = (usize, bool, String);

#[derive(Default)]
struct Shared {
    pending: Vec<Pending>,
    version: u64,
}

/// Channel Central.
pub struct ChannelCentral {
    win: WinId,
    chan: String,
    /// Lettres des listes : bans, exceptions, invitations (`None` si le serveur ne les a pas).
    letters: [Option<char>; 3],
    modes_per_line: usize,
    topic0: String,
    modes0: Modes,
    /// Listes telles que le serveur les a données.
    lists: [Vec<ListEntry>; 3],
    st: Rc<RefCell<Shared>>,
    seen: u64,
    dirty_topic: bool,
    dirty_modes: bool,
    op: bool,
    /// Décalage horaire de l'appareil (secondes), pour les dates des listes.
    utc_offset: i32,
}

/// Modes d'un canal tels que le moteur les connaît.
fn modes_of(cs: Option<&ChannelState>) -> Modes {
    let Some(cs) = cs else { return Modes::default() };
    let mut flags = [false; 6];
    for (k, (_, m)) in FLAGS.iter().enumerate() {
        flags[k] = cs.has_mode(*m);
    }
    Modes { flags, key: cs.has_mode('k').then(|| cs.key().unwrap_or("").to_string()), limit: cs.has_mode('l').then(|| cs.limit().unwrap_or(0)) }
}

/// Date d'une entrée de liste, à l'heure de l'appareil (`offset` en secondes).
fn date(secs: Option<i64>, offset: i32) -> String {
    match secs {
        Some(s) => alpirc_core::timeutil::format_time(s * 1000, offset, "dd/mm/yyyy HH:nn"),
        None => String::new(),
    }
}

impl ChannelCentral {
    /// La boîte du canal de la fenêtre `win` ; `None` si ce n'est pas un canal.
    pub fn new(client: &Client, win: WinId) -> Option<ChannelCentral> {
        let w = client.window(win)?;
        if w.kind != alpirc_core::WinKind::Channel {
            return None;
        }
        let session = w.conn.and_then(|c| client.session(c));
        let is = session.map(|s| s.isupport().clone()).unwrap_or_default();
        let mut c = ChannelCentral {
            win,
            chan: w.name.clone(),
            letters: [Some('b'), is.excepts, is.invex],
            modes_per_line: is.modes_per_line.clamp(1, 12),
            topic0: String::new(),
            modes0: Modes::default(),
            lists: Default::default(),
            st: Rc::default(),
            seen: 0,
            dirty_topic: false,
            dirty_modes: false,
            op: false,
            utc_offset: 0,
        };
        c.read_state(client);
        Some(c)
    }

    /// Relit le sujet, les modes et le rang depuis le moteur.
    fn read_state(&mut self, client: &Client) {
        let cs = client.channel_state(self.win);
        self.topic0 = cs.and_then(|c| c.topic.clone()).unwrap_or_default();
        self.modes0 = modes_of(cs);
        self.op = self.is_op(client);
    }

    fn is_op(&self, client: &Client) -> bool {
        let Some(conn) = client.window(self.win).and_then(|w| w.conn) else { return false };
        let (Some(s), Some(cs)) = (client.session(conn), client.channel_state(self.win)) else { return false };
        if !cs.joined {
            return false;
        }
        cs.members.get(&s.fold(s.nick())).is_some_and(|m| m.modes.contains(['q', 'a', 'o', 'h']))
    }

    fn server_lists(&self, client: &Client) -> [Vec<ListEntry>; 3] {
        let cs = client.channel_state(self.win);
        match cs {
            Some(cs) => [
                cs.bans.clone(),
                if self.letters[1].is_some() { cs.excepts.clone() } else { Vec::new() },
                if self.letters[2].is_some() { cs.invites.clone() } else { Vec::new() },
            ],
            None => Default::default(),
        }
    }

    fn fill_list(&self, view: &mut DialogView, lang: Lang, k: usize) {
        let (_, list, ..) = LISTS[k];
        let st = self.st.borrow();
        let removed = |m: &str| st.pending.iter().any(|(l, add, x)| *l == k && !add && x == m);
        let mut rows: Vec<String> = self.lists[k]
            .iter()
            .filter(|e| !removed(&e.mask))
            .map(|e| {
                let by = e.setter.as_deref().map(|s| s.split('!').next().unwrap_or(s).to_string()).unwrap_or_default();
                row(&[e.mask.clone(), by, date(e.time, self.utc_offset)])
            })
            .collect();
        for (_, _, m) in st.pending.iter().filter(|(l, add, _)| *l == k && *add) {
            rows.push(row(&[m.clone(), t(lang, "cc.pending"), String::new()]));
        }
        let keep = sel(view, list);
        let n = rows.len();
        view.set_items(list, &rows);
        select_row(view, list, keep.filter(|&i| i < n));
        drop(st);
        self.update_list_buttons(view, k);
    }

    fn update_list_buttons(&self, view: &mut DialogView, k: usize) {
        let (_, list, add, edit_id, rem) = LISTS[k];
        let ok = self.op && self.letters[k].is_some();
        view.set_enabled(list, self.letters[k].is_some());
        view.set_enabled(add, ok);
        let s = sel(view, list).is_some();
        view.set_enabled(edit_id, ok && s);
        view.set_enabled(rem, ok && s);
    }

    /// Masque de la ligne `i` de la liste `k` (entrée du serveur ou ajout en attente).
    fn mask_at(&self, view: &DialogView, k: usize, i: usize) -> Option<String> {
        view.items(LISTS[k].1).get(i).map(|r| r.split('\t').next().unwrap_or("").to_string())
    }

    fn show_modes(&self, view: &mut DialogView) {
        for (k, (id, _)) in FLAGS.iter().enumerate() {
            view.set_checked(*id, self.modes0.flags[k]);
        }
        view.set_checked(KEY_ON, self.modes0.key.is_some());
        view.set_text(KEY, self.modes0.key.as_deref().unwrap_or(""));
        view.set_checked(LIMIT_ON, self.modes0.limit.is_some());
        view.set_text(LIMIT, &self.modes0.limit.map(|l| l.to_string()).unwrap_or_default());
    }

    fn show_topic(&self, view: &mut DialogView, client: &Client, lang: Lang) {
        view.set_text(TOPIC, &self.topic0);
        let cs = client.channel_state(self.win);
        let by = match (cs.and_then(|c| c.topic_setter.clone()), cs.and_then(|c| c.topic_time)) {
            (Some(s), Some(tm)) => {
                let date = date(Some(tm), self.utc_offset);
                tf(lang, "cc.set_by", &[("nick", s.split('!').next().unwrap_or(&s)), ("date", &date)])
            }
            (Some(s), None) => tf(lang, "cc.set_by", &[("nick", s.split('!').next().unwrap_or(&s)), ("date", "")]),
            _ => String::new(),
        };
        view.set_text(SETBY, by.trim_end());
    }

    fn update_enables(&self, view: &mut DialogView) {
        let t_mode = view.checked(21);
        view.set_enabled(TOPIC, self.op || !t_mode);
        for (id, _) in FLAGS {
            view.set_enabled(id, self.op);
        }
        view.set_enabled(KEY_ON, self.op);
        view.set_enabled(LIMIT_ON, self.op);
        view.set_enabled(KEY, self.op && view.checked(KEY_ON));
        view.set_enabled(LIMIT, self.op && view.checked(LIMIT_ON));
        view.set_visible(NOT_OP, !self.op);
        for k in 0..3 {
            self.update_list_buttons(view, k);
        }
    }

    /// Modes tels que la boîte les montre.
    fn modes_now(&self, view: &DialogView) -> Modes {
        let mut flags = [false; 6];
        for (k, (id, _)) in FLAGS.iter().enumerate() {
            flags[k] = view.checked(*id);
        }
        let key = opt_text(view, KEY).filter(|_| view.checked(KEY_ON));
        let limit = text_of(view, LIMIT).trim().parse::<u32>().ok().filter(|&l| l > 0 && view.checked(LIMIT_ON));
        Modes { flags, key, limit }
    }

    /// Les commandes intégrées (nom, arguments) à lancer pour passer de l'état montré à l'ouverture à celui de la
    /// boîte. Elles passent par [`alpirc_core::Client::builtin`] : le sujet part tel quel (`|`, `{`, `}`, `$`).
    fn commands(&self, view: &DialogView) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let chan = &self.chan;
        let topic = text_of(view, TOPIC);
        if topic != self.topic0 && view.enabled(TOPIC) {
            out.push(if topic.trim().is_empty() {
                ("raw".to_string(), format!("TOPIC {chan} :"))
            } else {
                ("topic".to_string(), format!("{chan} {topic}"))
            });
        }
        if !self.op {
            return out;
        }
        // (signe, lettre, paramètre)
        let mut changes: Vec<(bool, char, Option<String>)> = Vec::new();
        let now = self.modes_now(view);
        for (k, (_, m)) in FLAGS.iter().enumerate() {
            if now.flags[k] != self.modes0.flags[k] {
                changes.push((now.flags[k], *m, None));
            }
        }
        match (&self.modes0.key, &now.key) {
            (Some(old), None) => changes.push((false, 'k', Some(if old.is_empty() { "*".into() } else { old.clone() }))),
            (None, Some(new)) => changes.push((true, 'k', Some(new.clone()))),
            (Some(old), Some(new)) if old != new => {
                changes.push((false, 'k', Some(if old.is_empty() { "*".into() } else { old.clone() })));
                changes.push((true, 'k', Some(new.clone())));
            }
            _ => {}
        }
        match (self.modes0.limit, now.limit) {
            (Some(_), None) => changes.push((false, 'l', None)),
            (old, Some(n)) if old != Some(n) => changes.push((true, 'l', Some(n.to_string()))),
            _ => {}
        }
        for (l, add, mask) in &self.st.borrow().pending {
            if let Some(c) = self.letters[*l] {
                changes.push((*add, c, Some(mask.clone())));
            }
        }
        // Les modes sans paramètre d'abord, puis par paquets de MODES paramètres.
        let (simple, with): (Vec<_>, Vec<_>) = changes.into_iter().partition(|c| c.2.is_none());
        let mut lines: Vec<Vec<(bool, char, Option<String>)>> = Vec::new();
        let mut cur = simple;
        for c in with {
            if cur.iter().filter(|x| x.2.is_some()).count() >= self.modes_per_line {
                lines.push(std::mem::take(&mut cur));
            }
            cur.push(c);
        }
        if !cur.is_empty() {
            lines.push(cur);
        }
        for l in lines {
            let mut s = String::new();
            let mut sign = None;
            let mut params = Vec::new();
            for (add, m, p) in l {
                if sign != Some(add) {
                    s.push(if add { '+' } else { '-' });
                    sign = Some(add);
                }
                s.push(m);
                if let Some(p) = p {
                    params.push(p);
                }
            }
            let tail = if params.is_empty() { String::new() } else { format!(" {}", params.join(" ")) };
            out.push(("mode".to_string(), format!("{chan} {s}{tail}")));
        }
        out
    }

    fn apply(&mut self, view: &mut DialogView, cx: &mut Cx) {
        for (name, args) in self.commands(view) {
            // Une erreur s'affiche dans la fenêtre du canal, comme pour une commande tapée.
            let _ = cx.client.builtin(self.win, &name, &args, cx.now);
        }
        // Ce qui est envoyé devient l'état de référence ; le serveur confirmera.
        self.topic0 = text_of(view, TOPIC);
        self.modes0 = self.modes_now(view);
        self.st.borrow_mut().pending.clear();
        self.dirty_topic = false;
        self.dirty_modes = false;
    }

    fn edit_mask(&self, k: usize, old: Option<String>, cx: &mut Cx) {
        let lang = cx.lang;
        let st = self.st.clone();
        let title = t(lang, ["cc.ban_title", "cc.except_title", "cc.invite_title"][k]);
        let form = Form::new(
            &title,
            "central-mask",
            vec![Field::text(&t(lang, "cc.mask"), old.as_deref().unwrap_or("*!*@*"))],
            Box::new(move |v: &[Val], _cx| {
                let m = v[0].text();
                if m.is_empty() || m.contains(' ') {
                    return Err(t(lang, "ab.err_mask"));
                }
                let mut s = st.borrow_mut();
                if let Some(o) = &old {
                    if *o == m {
                        return Ok(());
                    }
                    // Un ajout en attente est simplement remplacé.
                    match s.pending.iter().position(|(l, add, x)| *l == k && *add && x == o) {
                        Some(p) => {
                            s.pending.remove(p);
                        }
                        None => s.pending.push((k, false, o.clone())),
                    }
                }
                s.pending.push((k, true, m));
                s.version += 1;
                Ok(())
            }),
        );
        cx.actions.push(Action::Open(Box::new(form.width(400))));
    }
}

impl DialogCtl for ChannelCentral {
    fn kind(&self) -> Option<String> {
        Some(format!("central:{}", self.win))
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let tr = |k: &str| t(lang, k);
        let gw = W - 2 * MARGIN;
        let mut c: Vec<ControlSpec> = vec![
            group(10, (MARGIN, MARGIN, gw, 76), &tr("cc.topic")),
            edit(TOPIC, (20, 28, W - 40, EDIT_H), ""),
            text(SETBY, (20, 56, W - 40, LABEL_H), "").style("nowrap"),
            group(20, (MARGIN, 92, gw, 118), &tr("cc.modes")),
        ];
        let keys = ["cc.mode.t", "cc.mode.n", "cc.mode.i", "cc.mode.m", "cc.mode.p", "cc.mode.s"];
        let col2 = W / 2 + 4;
        for (k, (id, _)) in FLAGS.iter().enumerate() {
            let (x, y) = if k < 4 { (20, 112 + k as i32 * 22) } else { (col2, 112 + (k as i32 - 4) * 22) };
            c.push(check(*id, (x, y, col2 - 30, CHECK_H), &tr(keys[k])));
        }
        c.push(check(KEY_ON, (col2, 156, 100, CHECK_H), &tr("cc.mode.k")));
        c.push(edit(KEY, (col2 + 104, 154, W - 20 - col2 - 104, EDIT_H), "").style("limit 50"));
        c.push(check(LIMIT_ON, (col2, 180, 100, CHECK_H), &tr("cc.mode.l")));
        c.push(edit(LIMIT, (col2 + 104, 178, 60, EDIT_H), "").style("limit 6"));
        c.push(field_label(31, col2 + 170, 178, W - 20 - col2 - 170, &tr("cc.users")));
        let page = (MARGIN, 218, gw, H - 218 - MARGIN - BTN_H - 8);
        let tabs = ["cc.tab.bans", "cc.tab.excepts", "cc.tab.invites"];
        for (k, (tab, ..)) in LISTS.iter().enumerate() {
            c.push(ControlSpec::new(*tab, ControlKind::Tab, page, &tr(tabs[k])));
        }
        let bx = W - 20 - 96;
        let cols = [tr("cc.col.mask"), tr("cc.col.by"), tr("cc.col.date")];
        for (tab, list, add, edit_id, rem) in LISTS {
            c.push(list_view(list, (20, 250, bx - 8 - 20, page.1 + page.3 - 250 - 10), &cols).in_tab(tab));
            c.push(button(add, (bx, 250, 96, BTN_H), &tr("dlg.add_dots")).in_tab(tab));
            c.push(button(edit_id, (bx, 277, 96, BTN_H), &tr("dlg.edit_dots")).in_tab(tab));
            c.push(button(rem, (bx, 304, 96, BTN_H), &tr("dlg.remove")).in_tab(tab));
        }
        c.push(text(NOT_OP, (MARGIN + 4, H - MARGIN - BTN_H + 4, 260, LABEL_H), &tr("cc.not_op")).style("nowrap"));
        c.extend(ok_cancel(lang, W, H, OK, CANCEL, Some(APPLY)));
        spec(&tf(lang, "cc.title", &[("chan", &self.chan)]), W, H, c)
    }

    fn init(&mut self, view: &mut DialogView, cx: &mut Cx) {
        let lang = cx.lang;
        self.utc_offset = cx.utc_offset;
        for (_, list, ..) in LISTS {
            view.set_columns(list, &[(t(lang, "cc.col.mask"), 200), (t(lang, "cc.col.by"), 90), (t(lang, "cc.col.date"), 116)]);
        }
        self.show_topic(view, cx.client, lang);
        self.show_modes(view);
        self.lists = self.server_lists(cx.client);
        for k in 0..3 {
            self.fill_list(view, lang, k);
        }
        self.update_enables(view);
        // Les listes ont été demandées par `/channel` (qui ouvre cette boîte) : les redemander ferait afficher
        // la seconde réponse dans le canal.
        view.set_focus(TOPIC);
    }

    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, cx: &mut Cx) -> Outcome {
        let lang = cx.lang;
        match msg {
            DialogMsg::Ok(_) => {
                self.apply(view, cx);
                Outcome::Close
            }
            DialogMsg::Cancel => Outcome::Close,
            DialogMsg::Clicked(APPLY) => {
                self.apply(view, cx);
                for k in 0..3 {
                    self.fill_list(view, lang, k);
                }
                Outcome::Keep
            }
            DialogMsg::Changed(TOPIC) => {
                self.dirty_topic = true;
                Outcome::Keep
            }
            DialogMsg::Changed(KEY | LIMIT) => {
                self.dirty_modes = true;
                Outcome::Keep
            }
            DialogMsg::Clicked(id) if FLAGS.iter().any(|f| f.0 == id) || id == KEY_ON || id == LIMIT_ON => {
                self.dirty_modes = true;
                if id == KEY_ON && view.checked(KEY_ON) {
                    view.set_enabled(KEY, true);
                    view.set_focus(KEY);
                }
                if id == LIMIT_ON && view.checked(LIMIT_ON) {
                    view.set_enabled(LIMIT, true);
                    view.set_focus(LIMIT);
                }
                self.update_enables(view);
                Outcome::Keep
            }
            DialogMsg::SelectionChanged(id) => {
                if let Some(k) = LISTS.iter().position(|l| l.1 == id) {
                    self.update_list_buttons(view, k);
                }
                Outcome::Keep
            }
            DialogMsg::Clicked(id) | DialogMsg::DoubleClicked(id) => {
                let Some(k) = LISTS.iter().position(|l| l.1 == id || l.2 == id || l.3 == id || l.4 == id) else { return Outcome::Keep };
                if !self.op {
                    return Outcome::Keep;
                }
                let (_, list, add, _, rem) = LISTS[k];
                let cur = sel(view, list).and_then(|i| self.mask_at(view, k, i));
                if id == add {
                    self.edit_mask(k, None, cx);
                } else if id == rem {
                    if let Some(m) = cur {
                        let mut s = self.st.borrow_mut();
                        match s.pending.iter().position(|(l, a, x)| *l == k && *a && *x == m) {
                            Some(p) => {
                                s.pending.remove(p);
                            }
                            None => s.pending.push((k, false, m)),
                        }
                        drop(s);
                        self.fill_list(view, lang, k);
                    }
                } else if let Some(m) = cur {
                    self.edit_mask(k, Some(m), cx);
                }
                Outcome::Keep
            }
            _ => Outcome::Keep,
        }
    }

    fn refresh(&mut self, view: &mut DialogView, cx: &mut Cx) {
        let lang = cx.lang;
        self.utc_offset = cx.utc_offset;
        let lists = self.server_lists(cx.client);
        let v = self.st.borrow().version;
        if lists != self.lists || v != self.seen {
            self.lists = lists;
            self.seen = v;
            for k in 0..3 {
                self.fill_list(view, lang, k);
            }
        }
        let op = self.is_op(cx.client);
        let cs = cx.client.channel_state(self.win);
        let topic = cs.and_then(|c| c.topic.clone()).unwrap_or_default();
        let modes = modes_of(cs);
        if !self.dirty_topic && topic != self.topic0 {
            self.topic0 = topic;
            self.show_topic(view, cx.client, lang);
        }
        if !self.dirty_modes && modes != self.modes0 {
            self.modes0 = modes;
            self.show_modes(view);
        }
        if op != self.op {
            self.op = op;
        }
        self.update_enables(view);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testkit::{Kit, assert_translated};

    fn setup(k: &mut Kit, op: bool) -> WinId {
        let (conn, _) = k.connect();
        let me = if op { "@Guiwald" } else { "Guiwald" };
        for l in [
            ":Guiwald!~a@h JOIN #dlg-test".to_string(),
            ":irc.undernet.org 332 Guiwald #dlg-test :Bienvenue".to_string(),
            ":irc.undernet.org 333 Guiwald #dlg-test Kaelis!k@h 1792250000".to_string(),
            format!(":irc.undernet.org 353 Guiwald = #dlg-test :{me} Elisa"),
            ":irc.undernet.org 366 Guiwald #dlg-test :End".to_string(),
            ":irc.undernet.org 324 Guiwald #dlg-test +ntk sesame".to_string(),
        ] {
            k.line(conn, &l);
        }
        k.client.take_effects();
        k.client.find_window(Some(conn), "#dlg-test").unwrap()
    }

    #[test]
    fn minimal_mode_and_topic_commands() {
        let mut k = Kit::new("central");
        let win = setup(&mut k, true);
        let mut d = ChannelCentral::new(&k.client, win).unwrap();
        assert_translated(&d);
        let (mut v, _) = k.open(&mut d);
        assert_eq!(text_of(&v, TOPIC), "Bienvenue");
        assert!(v.checked(21) && v.checked(22) && v.checked(KEY_ON));
        assert_eq!(text_of(&v, KEY), "sesame");
        assert!(v.enabled(24), "op : modifiable");
        // Les bans arrivent.
        let conn = k.client.conns()[0];
        k.line(conn, ":irc.undernet.org 367 Guiwald #dlg-test *!*@spam.example Kaelis 1792250000");
        k.line(conn, ":irc.undernet.org 368 Guiwald #dlg-test :End of Channel Ban List");
        k.refresh(&mut d, &mut v);
        assert_eq!(v.items(41).len(), 1);
        // Sujet, -t +m, nouvelle clé, limite, retrait du ban, nouveau ban.
        v.set_text(TOPIC, "Nouveau sujet");
        k.send(&mut d, &mut v, DialogMsg::Changed(TOPIC));
        v.set_checked(21, false);
        v.set_checked(24, true);
        v.set_text(KEY, "ouvre");
        v.set_checked(LIMIT_ON, true);
        v.set_text(LIMIT, "42");
        v.set_selection(41, &[0]);
        k.send(&mut d, &mut v, DialogMsg::Clicked(44));
        assert_eq!(v.items(41).len(), 0);
        let (_, mut a) = k.send(&mut d, &mut v, DialogMsg::Clicked(42));
        let Some(Action::Open(mut f)) = a.pop() else { panic!() };
        let (mut fv, _) = k.open(f.as_mut());
        fv.set_text(11, "*!*@troll.example");
        k.send(f.as_mut(), &mut fv, DialogMsg::Ok(1));
        k.refresh(&mut d, &mut v);
        assert_eq!(v.items(41).len(), 1);
        let cmds = d.commands(&v);
        let c = |n: &str, a: &str| (n.to_string(), a.to_string());
        assert_eq!(cmds[0], c("topic", "#dlg-test Nouveau sujet"));
        assert_eq!(cmds[1], c("mode", "#dlg-test -t+m-k+kl sesame ouvre 42"));
        assert_eq!(cmds[2], c("mode", "#dlg-test -b+b *!*@spam.example *!*@troll.example"));
        let (o, a) = k.send(&mut d, &mut v, DialogMsg::Ok(OK));
        assert_eq!(o, Outcome::Close);
        k.run(a);
        let sent = k.sent();
        assert!(sent.contains(&"TOPIC #dlg-test :Nouveau sujet".to_string()), "{sent:?}");
        assert!(sent.iter().any(|l| l.starts_with("MODE #dlg-test -t+m-k+kl")), "{sent:?}");
    }

    #[test]
    fn topic_with_pipe_is_sent_whole() {
        let mut k = Kit::new("central3");
        let win = setup(&mut k, true);
        let mut d = ChannelCentral::new(&k.client, win).unwrap();
        let (mut v, _) = k.open(&mut d);
        v.set_text(TOPIC, "Lyon | Grenoble { $me }");
        let (_, a) = k.send(&mut d, &mut v, DialogMsg::Ok(OK));
        k.run(a);
        let sent = k.sent();
        assert!(sent.iter().any(|l| l == "TOPIC #dlg-test :Lyon | Grenoble { $me }"), "{sent:?}");
    }

    #[test]
    fn not_op_disables_and_sends_nothing() {
        let mut k = Kit::new("central2");
        let win = setup(&mut k, false);
        let mut d = ChannelCentral::new(&k.client, win).unwrap();
        let (mut v, _) = k.open(&mut d);
        assert!(!v.enabled(24) && !v.enabled(TOPIC) && v.visible(NOT_OP));
        v.set_checked(24, true);
        assert!(d.commands(&v).is_empty());
        // Pas un canal : pas de boîte.
        let st = k.client.windows()[0].id;
        assert!(ChannelCentral::new(&k.client, st).is_none());
    }

    #[test]
    fn dates_use_the_device_offset() {
        // 1er janvier 2026, 12:00 UTC, affiché à l'heure de Paris (UTC+1 en hiver).
        assert_eq!(date(Some(1_767_268_800), 3600), "01/01/2026 13:00");
        assert_eq!(date(None, 3600), "");
    }
}
