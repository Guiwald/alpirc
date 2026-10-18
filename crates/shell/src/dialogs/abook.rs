//! Carnet d'adresses (Alt+B) : onglets Infos (ce qu'on sait des gens : nom réel, e-mail, site, notes — ce que montre
//! aussi User Central), Notify (amis dont on suit l'arrivée), Ignore (masques ignorés, par sorte de message) et
//! Control (auto-op, auto-voice, protection). Ajouter et modifier passent par la petite
//! boîte de saisie ; OK écrit le carnet dans les réglages et resynchronise la liste Notify des connexions
//! ouvertes sans rien afficher (`Client::notify_changed` : `MONITOR` ou `ISON`).

use std::cell::RefCell;
use std::rc::Rc;

use alpirc_core::i18n::Lang;
use alpirc_core::settings::{AddressBook as Book, ControlEntry, ControlKind as CtlKind, IgnoreEntry, NotifyEntry, PersonEntry};
use alpirc_ui::{ControlKind, ControlSpec, DialogMsg, DialogSpec, DialogView};

use super::form::{Field, Form, Val};
use super::*;

const W: i32 = 520;
const H: i32 = 390;

const OK: u32 = 1;
const CANCEL: u32 = 2;
const TAB_NOTIFY: u32 = 10;
const TAB_IGNORE: u32 = 11;
const TAB_CONTROL: u32 = 12;
const TAB_INFO: u32 = 13;

/// Les quatre listes : (onglet, liste, ajouter, modifier, retirer, note).
const LISTS: [(u32, u32, u32, u32, u32, u32); 4] = [
    (TAB_INFO, 140, 141, 142, 143, 144),
    (TAB_NOTIFY, 110, 111, 112, 113, 114),
    (TAB_IGNORE, 120, 121, 122, 123, 124),
    (TAB_CONTROL, 130, 131, 132, 133, 134),
];

#[derive(Default)]
struct Shared {
    book: Book,
    version: u64,
}

/// Le carnet d'adresses.
pub struct AddressBook {
    st: Rc<RefCell<Shared>>,
    seen: u64,
}

/// Sortes de messages ignorés, dans l'ordre des cases.
fn ignore_types(e: &IgnoreEntry) -> [bool; 7] {
    [e.private, e.channel, e.notice, e.ctcp, e.invite, e.codes, e.dcc]
}

const IGNORE_KEYS: [&str; 7] = ["ab.ign.private", "ab.ign.channel", "ab.ign.notice", "ab.ign.ctcp", "ab.ign.invite", "ab.ign.codes", "ab.ign.dcc"];

fn kind_key(k: CtlKind) -> &'static str {
    match k {
        CtlKind::Autoop => "ab.ctl.autoop",
        CtlKind::Autovoice => "ab.ctl.autovoice",
        CtlKind::Protect => "ab.ctl.protect",
    }
}

/// Réseaux connus, pour les listes déroulantes.
fn network_names(client: &Client) -> Vec<String> {
    client.settings().networks.iter().map(|n| n.name.clone()).collect()
}

impl AddressBook {
    pub fn new(client: &Client) -> AddressBook {
        let st = Shared { book: client.settings().address_book.clone(), version: 0 };
        AddressBook { st: Rc::new(RefCell::new(st)), seen: 0 }
    }

    fn rows(&self, lang: Lang, list: u32) -> Vec<String> {
        let st = self.st.borrow();
        let all = t(lang, "ab.all");
        let net = |n: &Option<String>| n.clone().unwrap_or_else(|| all.clone());
        match list {
            140 => st
                .book
                .people
                .iter()
                .map(|e| row(&[e.nick.clone(), e.name.clone(), e.notes.lines().next().unwrap_or("").to_string()]))
                .collect(),
            110 => st.book.notify.iter().map(|e| row(&[e.nick.clone(), e.note.clone(), net(&e.network)])).collect(),
            120 => st
                .book
                .ignore
                .iter()
                .map(|e| {
                    let types: Vec<String> = ignore_types(e).iter().zip(IGNORE_KEYS).filter(|(on, _)| **on).map(|(_, k)| plain(&t(lang, k))).collect();
                    row(&[e.mask.clone(), types.join(", "), net(&e.network)])
                })
                .collect(),
            _ => st
                .book
                .control
                .iter()
                .map(|e| {
                    let chans = if e.channels.is_empty() { all.clone() } else { e.channels.join(" ") };
                    row(&[t(lang, kind_key(e.kind)), e.mask.clone(), chans, net(&e.network)])
                })
                .collect(),
        }
    }

    fn fill(&self, view: &mut DialogView, lang: Lang, list: u32, select: Option<usize>) {
        let rows = self.rows(lang, list);
        let n = rows.len();
        view.set_items(list, &rows);
        select_row(view, list, select.filter(|&i| i < n));
        self.update_buttons(view, list);
    }

    fn update_buttons(&self, view: &mut DialogView, list: u32) {
        let on = sel(view, list).is_some();
        view.set_enabled(list + 2, on);
        view.set_enabled(list + 3, on);
    }

    /// Ouvre la petite boîte pour ajouter (`i` = None) ou modifier une entrée de la liste `list`.
    fn edit(&self, list: u32, i: Option<usize>, cx: &mut Cx) {
        let lang = cx.lang;
        let nets = network_names(cx.client);
        let st = self.st.clone();
        let add = i.is_none();
        let form = match list {
            140 => {
                let e = i.and_then(|i| self.st.borrow().book.people.get(i).cloned()).unwrap_or_default();
                let fields = vec![
                    Field::text(&t(lang, "ab.nick"), &e.nick),
                    Field::text(&t(lang, "uc.realname"), &e.name),
                    Field::text(&t(lang, "uc.email"), &e.email),
                    Field::text(&t(lang, "uc.website"), &e.website),
                    Field::notes(&t(lang, "uc.notes"), &e.notes),
                ];
                Form::new(
                    &t(lang, if add { "ab.info.add" } else { "ab.info.edit" }),
                    "abook-info",
                    fields,
                    Box::new(move |v: &[Val], _cx| {
                        let nick = v[0].text();
                        if nick.is_empty() || nick.contains(' ') {
                            return Err(t(lang, "ab.err_nick"));
                        }
                        let e = PersonEntry { nick, name: v[1].text(), email: v[2].text(), website: v[3].text(), notes: v[4].text() };
                        let mut s = st.borrow_mut();
                        if s.book.people.iter().enumerate().any(|(k, x)| Some(k) != i && x.nick.eq_ignore_ascii_case(&e.nick)) {
                            return Err(t(lang, "ab.err_exists"));
                        }
                        match i {
                            Some(i) if i < s.book.people.len() => s.book.people[i] = e,
                            _ => s.book.people.push(e),
                        }
                        s.version += 1;
                        Ok(())
                    }),
                )
            }
            110 => {
                let e = i.and_then(|i| self.st.borrow().book.notify.get(i).cloned()).unwrap_or_default();
                let fields = vec![
                    Field::text(&t(lang, "ab.nick"), &e.nick),
                    Field::text(&t(lang, "ab.note"), &e.note),
                    Field::combo(&t(lang, "ab.network"), nets, e.network.as_deref().unwrap_or("")),
                ];
                Form::new(
                    &t(lang, if add { "ab.notify.add" } else { "ab.notify.edit" }),
                    "abook-notify",
                    fields,
                    Box::new(move |v: &[Val], _cx| {
                        let nick = v[0].text();
                        if nick.is_empty() || nick.contains(' ') {
                            return Err(t(lang, "ab.err_nick"));
                        }
                        let e = NotifyEntry { nick, note: v[1].text(), network: v[2].opt() };
                        let mut s = st.borrow_mut();
                        if s.book.notify.iter().enumerate().any(|(k, x)| Some(k) != i && x.nick.eq_ignore_ascii_case(&e.nick)) {
                            return Err(t(lang, "ab.err_exists"));
                        }
                        match i {
                            Some(i) if i < s.book.notify.len() => s.book.notify[i] = e,
                            _ => s.book.notify.push(e),
                        }
                        s.version += 1;
                        Ok(())
                    }),
                )
            }
            120 => {
                let e = i.and_then(|i| self.st.borrow().book.ignore.get(i).cloned()).unwrap_or(IgnoreEntry {
                    private: true,
                    channel: true,
                    notice: true,
                    ctcp: true,
                    invite: true,
                    dcc: true,
                    ..Default::default()
                });
                let mut fields = vec![Field::text(&t(lang, "ab.mask"), &e.mask)];
                for (on, k) in ignore_types(&e).iter().zip(IGNORE_KEYS) {
                    fields.push(Field::check(&t(lang, k), *on));
                }
                fields.push(Field::combo(&t(lang, "ab.network"), nets, e.network.as_deref().unwrap_or("")));
                let until = e.until_ms;
                Form::new(
                    &t(lang, if add { "ab.ignore.add" } else { "ab.ignore.edit" }),
                    "abook-ignore",
                    fields,
                    Box::new(move |v: &[Val], _cx| {
                        let mask = v[0].text();
                        if mask.is_empty() || mask.contains(' ') {
                            return Err(t(lang, "ab.err_mask"));
                        }
                        let e = IgnoreEntry {
                            mask,
                            private: v[1].on(),
                            channel: v[2].on(),
                            notice: v[3].on(),
                            ctcp: v[4].on(),
                            invite: v[5].on(),
                            codes: v[6].on(),
                            dcc: v[7].on(),
                            network: v[8].opt(),
                            until_ms: until,
                        };
                        let mut s = st.borrow_mut();
                        match i {
                            Some(i) if i < s.book.ignore.len() => s.book.ignore[i] = e,
                            _ => s.book.ignore.push(e),
                        }
                        s.version += 1;
                        Ok(())
                    }),
                )
            }
            _ => {
                let e = i.and_then(|i| self.st.borrow().book.control.get(i).cloned()).unwrap_or_default();
                let kinds = vec![t(lang, "ab.ctl.autoop"), t(lang, "ab.ctl.autovoice"), t(lang, "ab.ctl.protect")];
                let k = match e.kind {
                    CtlKind::Autoop => 0,
                    CtlKind::Autovoice => 1,
                    CtlKind::Protect => 2,
                };
                let fields = vec![
                    Field::choice(&t(lang, "ab.ctl.kind"), kinds, k),
                    Field::text(&t(lang, "ab.mask"), &e.mask),
                    Field::text(&t(lang, "ab.channels"), &e.channels.join(" ")),
                    Field::combo(&t(lang, "ab.network"), nets, e.network.as_deref().unwrap_or("")),
                ];
                Form::new(
                    &t(lang, if add { "ab.control.add" } else { "ab.control.edit" }),
                    "abook-control",
                    fields,
                    Box::new(move |v: &[Val], _cx| {
                        let mask = v[1].text();
                        if mask.is_empty() || mask.contains(' ') {
                            return Err(t(lang, "ab.err_mask"));
                        }
                        let kind = match v[0].index() {
                            1 => CtlKind::Autovoice,
                            2 => CtlKind::Protect,
                            _ => CtlKind::Autoop,
                        };
                        let channels = v[2].text().split([',', ' ']).filter(|c| !c.is_empty()).map(str::to_string).collect();
                        let e = ControlEntry { kind, mask, channels, network: v[3].opt() };
                        let mut s = st.borrow_mut();
                        match i {
                            Some(i) if i < s.book.control.len() => s.book.control[i] = e,
                            _ => s.book.control.push(e),
                        }
                        s.version += 1;
                        Ok(())
                    }),
                )
            }
        };
        cx.actions.push(Action::Open(Box::new(form.width(380))));
    }

    fn remove(&self, list: u32, i: usize) {
        let mut s = self.st.borrow_mut();
        let removed = match list {
            140 if i < s.book.people.len() => {
                s.book.people.remove(i);
                true
            }
            110 if i < s.book.notify.len() => {
                s.book.notify.remove(i);
                true
            }
            120 if i < s.book.ignore.len() => {
                s.book.ignore.remove(i);
                true
            }
            130 if i < s.book.control.len() => {
                s.book.control.remove(i);
                true
            }
            _ => false,
        };
        if removed {
            s.version += 1;
        }
    }

    /// OK : écrit le carnet et resynchronise la liste Notify des connexions ouvertes.
    fn apply(&self, cx: &mut Cx) {
        let book = self.st.borrow().book.clone();
        let old = cx.client.settings().address_book.clone();
        if book == old {
            return;
        }
        let notify_changed = book.notify != old.notify;
        cx.client.settings_mut().address_book = book;
        cx.actions.push(Action::SettingsChanged);
        if notify_changed {
            cx.client.notify_changed(cx.now);
        }
    }
}

impl DialogCtl for AddressBook {
    fn kind(&self) -> Option<String> {
        Some("abook".into())
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let tr = |k: &str| t(lang, k);
        let page = (MARGIN, MARGIN, W - 2 * MARGIN, H - 2 * MARGIN - BTN_H - 8);
        let mut c: Vec<ControlSpec> = vec![
            ControlSpec::new(TAB_INFO, ControlKind::Tab, page, &tr("ab.tab.info")),
            ControlSpec::new(TAB_NOTIFY, ControlKind::Tab, page, &tr("ab.tab.notify")),
            ControlSpec::new(TAB_IGNORE, ControlKind::Tab, page, &tr("ab.tab.ignore")),
            ControlSpec::new(TAB_CONTROL, ControlKind::Tab, page, &tr("ab.tab.control")),
        ];
        let x0 = 20;
        let bx = W - MARGIN - 12 - 96;
        let lw = bx - 8 - x0;
        let cols: [Vec<String>; 4] = [
            vec![tr("ab.col.nick"), tr("ab.col.name"), tr("ab.col.notes")],
            vec![tr("ab.col.nick"), tr("ab.col.note"), tr("ab.col.network")],
            vec![tr("ab.col.mask"), tr("ab.col.ignored"), tr("ab.col.network")],
            vec![tr("ab.col.kind"), tr("ab.col.mask"), tr("ab.col.channels"), tr("ab.col.network")],
        ];
        let notes = ["ab.note.info", "ab.note.notify", "ab.note.ignore", "ab.note.control"];
        for (k, (tab, list, add, edit_id, rem, note)) in LISTS.into_iter().enumerate() {
            c.push(list_view(list, (x0, 40, lw, 250), &cols[k]).in_tab(tab));
            c.push(button(add, (bx, 40, 96, BTN_H), &tr("dlg.add_dots")).in_tab(tab));
            c.push(button(edit_id, (bx, 67, 96, BTN_H), &tr("dlg.edit_dots")).in_tab(tab));
            c.push(button(rem, (bx, 94, 96, BTN_H), &tr("dlg.remove")).in_tab(tab));
            c.push(text(note, (x0, 298, W - 2 * x0, 36), &tr(notes[k])).in_tab(tab));
        }
        c.extend(ok_cancel(lang, W, H, OK, CANCEL, None));
        spec(&tr("ab.title"), W, H, c)
    }

    fn init(&mut self, view: &mut DialogView, cx: &mut Cx) {
        let lang = cx.lang;
        let tr = |k: &str| t(lang, k);
        view.set_columns(140, &[(tr("ab.col.nick"), 100), (tr("ab.col.name"), 120), (tr("ab.col.notes"), 148)]);
        view.set_columns(110, &[(tr("ab.col.nick"), 110), (tr("ab.col.note"), 160), (tr("ab.col.network"), 98)]);
        view.set_columns(120, &[(tr("ab.col.mask"), 140), (tr("ab.col.ignored"), 150), (tr("ab.col.network"), 78)]);
        view.set_columns(130, &[(tr("ab.col.kind"), 86), (tr("ab.col.mask"), 124), (tr("ab.col.channels"), 90), (tr("ab.col.network"), 68)]);
        for (_, list, ..) in LISTS {
            self.fill(view, lang, list, Some(0));
        }
    }

    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, cx: &mut Cx) -> Outcome {
        let lang = cx.lang;
        let find = |id: u32| LISTS.iter().find(|l| l.1 == id || l.2 == id || l.3 == id || l.4 == id).copied();
        match msg {
            DialogMsg::Ok(_) => {
                self.apply(cx);
                Outcome::Close
            }
            DialogMsg::Cancel => Outcome::Close,
            DialogMsg::SelectionChanged(id) => {
                if let Some(l) = find(id) {
                    self.update_buttons(view, l.1);
                }
                Outcome::Keep
            }
            DialogMsg::DoubleClicked(id) => {
                if let Some(l) = find(id).filter(|l| l.1 == id)
                    && let Some(i) = sel(view, l.1)
                {
                    self.edit(l.1, Some(i), cx);
                }
                Outcome::Keep
            }
            DialogMsg::Clicked(id) => {
                let Some(l) = find(id) else { return Outcome::Keep };
                let (list, i) = (l.1, sel(view, l.1));
                if id == l.2 {
                    self.edit(list, None, cx);
                } else if let (true, Some(i)) = (id == l.3, i) {
                    self.edit(list, Some(i), cx);
                } else if let (true, Some(i)) = (id == l.4, i) {
                    self.remove(list, i);
                    self.fill(view, lang, list, Some(i.saturating_sub(1)));
                    self.seen = self.st.borrow().version;
                }
                Outcome::Keep
            }
            _ => Outcome::Keep,
        }
    }

    fn refresh(&mut self, view: &mut DialogView, cx: &mut Cx) {
        let v = self.st.borrow().version;
        if v == self.seen {
            return;
        }
        self.seen = v;
        for (_, list, ..) in LISTS {
            let prev = view.items(list).len();
            let n = self.rows(cx.lang, list).len();
            let pick = if n > prev { Some(n - 1) } else { sel(view, list) };
            self.fill(view, cx.lang, list, pick);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testkit::{Kit, assert_translated};

    fn add(k: &mut Kit, ab: &mut AddressBook, v: &mut DialogView, btn: u32, fill: impl FnOnce(&mut DialogView)) {
        let (_, mut a) = k.send(ab, v, DialogMsg::Clicked(btn));
        let Some(Action::Open(mut f)) = a.pop() else { panic!() };
        let (mut fv, _) = k.open(f.as_mut());
        fill(&mut fv);
        let (o, _) = k.send(f.as_mut(), &mut fv, DialogMsg::Ok(1));
        assert_eq!(o, Outcome::Close);
        k.refresh(ab, v);
    }

    #[test]
    fn notify_ignore_control_and_resync() {
        let mut k = Kit::new("abook");
        let (_conn, _st) = k.connect();
        k.client.settings_mut().address_book.notify.push(NotifyEntry { nick: "Zaza".into(), ..Default::default() });
        let mut ab = AddressBook::new(&k.client);
        assert_translated(&ab);
        let (mut v, _) = k.open(&mut ab);
        assert_eq!(v.items(110).len(), 1);
        add(&mut k, &mut ab, &mut v, 111, |f| {
            f.set_text(11, "Elisa");
            f.set_text(13, "de Lyon");
        });
        assert_eq!(v.items(110).len(), 2);
        // Retirer Zaza.
        v.set_selection(110, &[0]);
        k.send(&mut ab, &mut v, DialogMsg::Clicked(113));
        add(&mut k, &mut ab, &mut v, 121, |f| {
            f.set_text(11, "*!*@spam.example");
            f.set_checked(23, true); // codes
        });
        add(&mut k, &mut ab, &mut v, 131, |f| {
            f.set_selection(11, &[1]);
            f.set_text(13, "Elisa!*@*");
            f.set_text(15, "#lyon, #grenoble");
        });
        let (o, actions) = k.send(&mut ab, &mut v, DialogMsg::Ok(OK));
        assert_eq!(o, Outcome::Close);
        // Plus de `/notify` (et de lignes « ajouté »/« retiré ») : resynchronisation silencieuse.
        assert!(!actions.iter().any(|a| matches!(a, Action::Command(..))));
        let b = &k.client.settings().address_book;
        assert_eq!(b.notify.len(), 1);
        assert_eq!((b.notify[0].nick.as_str(), b.notify[0].note.as_str()), ("Elisa", "de Lyon"));
        assert!(b.ignore[0].codes && b.ignore[0].private);
        assert_eq!(b.control[0].kind, CtlKind::Autovoice);
        assert_eq!(b.control[0].channels, vec!["#lyon", "#grenoble"]);
        // Les commandes partent vers le serveur (ISON ou MONITOR).
        k.run(actions);
        k.client.tick(k.now + 1);
        let sent = k.sent();
        assert!(sent.iter().any(|l| l.starts_with("ISON") && l.contains("Elisa") && !l.contains("Zaza")), "{sent:?}");
    }

    #[test]
    fn info_tab_keeps_people_and_their_notes() {
        let mut k = Kit::new("abook-info");
        let mut ab = AddressBook::new(&k.client);
        let (mut v, _) = k.open(&mut ab);
        assert_eq!(v.items(140).len(), 0);
        add(&mut k, &mut ab, &mut v, 141, |f| {
            f.set_text(11, "Elisa");
            f.set_text(13, "Élisa Dumas");
            f.set_text(19, "Rencontrée sur #lyon.\nFan de ski.");
        });
        assert_eq!(v.items(140), vec!["Elisa\tÉlisa Dumas\tRencontrée sur #lyon.".to_string()]);
        // Pas deux fois le même pseudo.
        let (_, mut a) = k.send(&mut ab, &mut v, DialogMsg::Clicked(141));
        let Some(Action::Open(mut f)) = a.pop() else { panic!() };
        let (mut fv, _) = k.open(f.as_mut());
        fv.set_text(11, "ELISA");
        let (o, _) = k.send(f.as_mut(), &mut fv, DialogMsg::Ok(1));
        assert_eq!(o, Outcome::Keep);
        k.send(&mut ab, &mut v, DialogMsg::Ok(OK));
        let p = k.client.settings().address_book.person("elisa").cloned().unwrap();
        assert_eq!((p.name.as_str(), p.notes.as_str()), ("Élisa Dumas", "Rencontrée sur #lyon.\nFan de ski."));
    }
}
