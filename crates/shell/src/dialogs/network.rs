//! Éditeur d'un réseau : nom et serveurs (adresse, port, TLS, vérification du certificat, mot de passe),
//! identité propre au réseau, encodage, identification (SASL, compte X d'Undernet, NickServ) et Perform.
//!
//! La boîte travaille sur une copie du réseau ; à la validation, elle la passe à la fermeture fournie par la
//! boîte qui l'a ouverte (Options › Serveurs, choix du serveur), qui la range où il faut.

use alpirc_core::i18n::Lang;
use alpirc_core::settings::{Network, SaslSettings, Server, UndernetX, Verify};
use alpirc_ui::{ControlSpec, DialogMsg, DialogSpec, DialogView};

use super::*;

/// Validation : reçoit le réseau modifié ; `Err(message)` garde la boîte ouverte.
pub type OnNetwork = Box<dyn FnMut(Network, &mut Cx) -> Result<(), String>>;

const OK: u32 = 1;
const CANCEL: u32 = 2;
const TAB_SERVERS: u32 = 10;
const TAB_IDENT: u32 = 11;
const TAB_LOGIN: u32 = 12;
const TAB_PERFORM: u32 = 13;

const NAME: u32 = 101;
const LIST: u32 = 103;
const ADD: u32 = 104;
const REMOVE: u32 = 105;
const UP: u32 = 106;
const DOWN: u32 = 107;
const HOST: u32 = 110;
const PORT: u32 = 112;
const TLS: u32 = 113;
const VERIFY: u32 = 115;
const FP_L: u32 = 116;
const FP: u32 = 117;
const PASS: u32 = 119;

const NICK: u32 = 202;
const ALT: u32 = 204;
const REAL: u32 = 206;
const USER: u32 = 208;
const ENC: u32 = 210;
const AUTO: u32 = 211;
const BOUNCER: u32 = 212;

pub(crate) const MECH: u32 = 302;
const ACC: u32 = 304;
const SPW: u32 = 306;
pub(crate) const CERT: u32 = 308;
pub(crate) const CERT_BROWSE: u32 = 309;
const XU: u32 = 312;
const XP: u32 = 314;
const XHIDE: u32 = 315;
const XREQ: u32 = 316;
const NS: u32 = 321;

const PERF: u32 = 401;

const W: i32 = 520;
const H: i32 = 444;

/// Mécanismes SASL proposés (le premier : pas de SASL).
const MECHS: [&str; 4] = ["", "PLAIN", "SCRAM-SHA-256", "EXTERNAL"];
/// Encodages proposés.
const ENCODINGS: [&str; 3] = ["auto", "utf-8", "latin1"];

/// L'éditeur de réseau.
pub struct NetworkEditor {
    net: Network,
    /// Nouveau réseau (titre de la boîte).
    is_new: bool,
    /// Serveur choisi dans la liste.
    sel: Option<usize>,
    on_ok: OnNetwork,
}

/// Normalise une empreinte SHA-256 : hexadécimal minuscule sans séparateur.
pub fn normalize_fp(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_hexdigit()).collect::<String>().to_ascii_lowercase()
}

fn verify_index(v: Verify) -> usize {
    match v {
        Verify::Full => 0,
        Verify::AcceptInvalid => 1,
        Verify::Pinned => 2,
    }
}

fn verify_of(i: usize) -> Verify {
    match i {
        1 => Verify::AcceptInvalid,
        2 => Verify::Pinned,
        _ => Verify::Full,
    }
}

/// Libellé court de la vérification du certificat (colonne de la liste).
fn verify_label(lang: Lang, s: &Server) -> String {
    if !s.tls {
        return String::new();
    }
    t(lang, ["net.verify.full", "net.verify.accept", "net.verify.pinned"][verify_index(s.verify)])
}

impl NetworkEditor {
    pub fn new(net: Network, is_new: bool, on_ok: OnNetwork) -> NetworkEditor {
        let sel = if net.servers.is_empty() { None } else { Some(0) };
        NetworkEditor { net, is_new, sel, on_ok }
    }

    fn server_row(lang: Lang, s: &Server) -> String {
        let host = if s.host.is_empty() { t(lang, "net.new_server") } else { s.host.clone() };
        row(&[host, s.port.to_string(), if s.tls { t(lang, "net.yes") } else { String::new() }, verify_label(lang, s)])
    }

    fn fill_servers(&self, view: &mut DialogView, lang: Lang) {
        let rows: Vec<String> = self.net.servers.iter().map(|s| Self::server_row(lang, s)).collect();
        view.set_items(LIST, &rows);
        select_row(view, LIST, self.sel);
    }

    /// Recopie le serveur choisi dans les champs.
    fn load_server(&self, view: &mut DialogView) {
        let s = self.sel.and_then(|i| self.net.servers.get(i)).cloned();
        let on = s.is_some();
        for id in [HOST, PORT, TLS, VERIFY, FP, PASS, REMOVE] {
            view.set_enabled(id, on);
        }
        let s = s.unwrap_or_default();
        view.set_text(HOST, &s.host);
        view.set_text(PORT, &if on { s.port.to_string() } else { String::new() });
        view.set_checked(TLS, s.tls);
        view.set_selection(VERIFY, &[verify_index(s.verify)]);
        view.set_text(FP, s.fingerprint.as_deref().unwrap_or(""));
        view.set_text(PASS, s.password.as_deref().unwrap_or(""));
        view.set_enabled(UP, self.sel.is_some_and(|i| i > 0));
        view.set_enabled(DOWN, self.sel.is_some_and(|i| i + 1 < self.net.servers.len()));
        self.update_enables(view);
    }

    fn update_enables(&self, view: &mut DialogView) {
        let on = self.sel.is_some();
        let tls = on && view.checked(TLS);
        view.set_enabled(VERIFY, tls);
        let pinned = tls && sel(view, VERIFY) == Some(2);
        view.set_enabled(FP, pinned);
        view.set_enabled(FP_L, pinned);
    }

    /// Recopie les champs dans le serveur choisi (et sa ligne).
    fn store_server(&mut self, view: &mut DialogView, lang: Lang) {
        let Some(i) = self.sel else { return };
        let Some(s) = self.net.servers.get_mut(i) else { return };
        s.host = text_of(view, HOST).trim().to_string();
        s.port = num(view, PORT, s.port);
        s.tls = view.checked(TLS);
        s.verify = verify_of(sel(view, VERIFY).unwrap_or(0));
        s.fingerprint = opt_text(view, FP).map(|f| normalize_fp(&f)).filter(|f| !f.is_empty());
        s.password = opt_text(view, PASS);
        let r = Self::server_row(lang, s);
        let mut items = view.items(LIST);
        if let Some(x) = items.get_mut(i) {
            *x = r;
            view.set_items(LIST, &items);
            select_row(view, LIST, Some(i));
        }
    }

    /// Le réseau tel que la boîte le montre.
    fn collect(&mut self, view: &mut DialogView, lang: Lang) -> Network {
        self.store_server(view, lang);
        let mut n = self.net.clone();
        n.name = text_of(view, NAME).trim().to_string();
        n.servers.retain(|s| !s.host.is_empty());
        n.nick = opt_text(view, NICK);
        n.alt_nick = opt_text(view, ALT);
        n.realname = opt_text(view, REAL);
        n.username = opt_text(view, USER);
        n.encoding = ENCODINGS[sel(view, ENC).unwrap_or(0).min(2)].to_string();
        n.auto_connect = view.checked(AUTO);
        n.bouncer = view.checked(BOUNCER);
        let mech = sel(view, MECH).unwrap_or(0).min(3);
        n.sasl = (mech > 0).then(|| SaslSettings {
            mechanism: MECHS[mech].to_string(),
            account: text_of(view, ACC).trim().to_string(),
            password: text_of(view, SPW),
            client_cert: opt_text(view, CERT).map(PathBuf::from),
        });
        let (xu, xp) = (text_of(view, XU).trim().to_string(), text_of(view, XP));
        n.undernet_x =
            (!xu.is_empty() || !xp.is_empty()).then(|| UndernetX { username: xu, password: xp, hide_host: view.checked(XHIDE), require_x: view.checked(XREQ) });
        n.nickserv_password = opt_text(view, NS);
        n.perform = text_of(view, PERF).lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect();
        n
    }
}

impl DialogCtl for NetworkEditor {
    fn kind(&self) -> Option<String> {
        Some("network".into())
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let tr = |k: &str| t(lang, k);
        let title = if self.is_new { tr("net.title_new") } else { tf(lang, "net.title", &[("name", &self.net.name)]) };
        let page = (MARGIN, MARGIN, W - 2 * MARGIN, H - 2 * MARGIN - BTN_H - 8);
        let mut c: Vec<ControlSpec> = vec![
            ControlSpec::new(TAB_SERVERS, alpirc_ui::ControlKind::Tab, page, &tr("net.tab.servers")),
            ControlSpec::new(TAB_IDENT, alpirc_ui::ControlKind::Tab, page, &tr("net.tab.identity")),
            ControlSpec::new(TAB_LOGIN, alpirc_ui::ControlKind::Tab, page, &tr("net.tab.login")),
            ControlSpec::new(TAB_PERFORM, alpirc_ui::ControlKind::Tab, page, &tr("net.tab.perform")),
        ];
        let (x0, x1) = (20, W - 20);
        // --- Serveurs ---
        let s = |c: ControlSpec| c.in_tab(TAB_SERVERS);
        c.push(s(field_label(100, x0, 40, 110, &tr("net.name"))));
        c.push(s(edit(NAME, (x0 + 114, 40, 240, EDIT_H), "")));
        c.push(s(text(102, (x0, 72, 200, LABEL_H), &tr("net.servers"))));
        let cols = [tr("net.col.host"), tr("net.col.port"), tr("net.col.tls"), tr("net.col.cert")];
        c.push(s(list_view(LIST, (x0, 90, 376, 118), &cols)));
        let bx = x1 - 96;
        for (k, (id, key)) in [(ADD, "dlg.add"), (REMOVE, "dlg.remove"), (UP, "dlg.up"), (DOWN, "dlg.down")].into_iter().enumerate() {
            c.push(s(button(id, (bx, 90 + k as i32 * 27, 96, BTN_H), &tr(key))));
        }
        c.push(s(group(108, (x0, 216, x1 - x0, 166), &tr("net.server_group"))));
        let (lx, fx) = (x0 + 12, x0 + 106);
        c.push(s(field_label(109, lx, 238, 90, &tr("net.host"))));
        c.push(s(edit(HOST, (fx, 238, 210, EDIT_H), "")));
        c.push(s(field_label(111, fx + 222, 238, 40, &tr("net.port"))));
        c.push(s(edit(PORT, (fx + 262, 238, 60, EDIT_H), "").style("limit 5")));
        c.push(s(check(TLS, (fx, 266, 300, CHECK_H), &tr("net.tls"))));
        c.push(s(field_label(114, lx, 290, 90, &tr("net.verify"))));
        let verify_items = [tr("net.verify.full_long"), tr("net.verify.accept_long"), tr("net.verify.pinned_long")];
        c.push(s(drop_list(VERIFY, (fx, 290, 262, EDIT_H + 3 * 16 + 4), &verify_items)));
        c.push(s(field_label(FP_L, lx, 318, 90, &tr("net.fingerprint"))));
        c.push(s(edit(FP, (fx, 318, x1 - 12 - fx, EDIT_H), "")));
        c.push(s(field_label(118, lx, 346, 90, &tr("net.password"))));
        c.push(s(password(PASS, (fx, 346, 210, EDIT_H), "")));
        // --- Identité ---
        let i = |c: ControlSpec| c.in_tab(TAB_IDENT);
        c.push(i(text(200, (x0, 40, x1 - x0, 32), &tr("net.ident_note"))));
        let fx = x0 + 136;
        for (k, (lid, id, key)) in
            [(201, NICK, "opt.nick"), (203, ALT, "opt.alt"), (205, REAL, "opt.realname"), (207, USER, "opt.username")].into_iter().enumerate()
        {
            let y = 80 + k as i32 * ROW;
            c.push(i(field_label(lid, x0, y, 130, &tr(key))));
            c.push(i(edit(id, (fx, y, 220, EDIT_H), "")));
        }
        c.push(i(field_label(209, x0, 200, 130, &tr("net.encoding"))));
        let enc = [tr("net.enc.auto"), tr("net.enc.utf8"), tr("net.enc.latin1")];
        c.push(i(drop_list(ENC, (fx, 200, 280, EDIT_H + 3 * 16 + 4), &enc)));
        c.push(i(check(AUTO, (x0, 240, x1 - x0, CHECK_H), &tr("net.auto_connect"))));
        c.push(i(check(BOUNCER, (x0, 264, x1 - x0, CHECK_H), &tr("net.bouncer"))));
        // --- Identification ---
        let l = |c: ControlSpec| c.in_tab(TAB_LOGIN);
        c.push(l(group(300, (x0, 38, x1 - x0, 140), &tr("net.sasl"))));
        let (lx, fx) = (x0 + 12, x0 + 146);
        let mechs = [tr("net.sasl.none"), "PLAIN".to_string(), "SCRAM-SHA-256".to_string(), "EXTERNAL".to_string()];
        c.push(l(field_label(301, lx, 58, 130, &tr("net.sasl.mech"))));
        c.push(l(drop_list(MECH, (fx, 58, 170, EDIT_H + 4 * 16 + 4), &mechs)));
        c.push(l(field_label(303, lx, 86, 130, &tr("net.sasl.account"))));
        c.push(l(edit(ACC, (fx, 86, 200, EDIT_H), "")));
        c.push(l(field_label(305, lx, 114, 130, &tr("net.password"))));
        c.push(l(password(SPW, (fx, 114, 200, EDIT_H), "")));
        c.push(l(field_label(307, lx, 142, 130, &tr("net.sasl.cert"))));
        c.push(l(edit(CERT, (fx, 142, x1 - 12 - fx - 102, EDIT_H), "")));
        c.push(l(button(CERT_BROWSE, (x1 - 12 - 96, 141, 96, BTN_H), &tr("se.browse"))));
        c.push(l(group(310, (x0, 186, x1 - x0, 104), &tr("net.x"))));
        c.push(l(field_label(311, lx, 206, 130, &tr("net.x.user"))));
        c.push(l(edit(XU, (fx, 206, 110, EDIT_H), "")));
        c.push(l(field_label(313, fx + 120, 206, 90, &tr("net.x.password"))));
        c.push(l(password(XP, (fx + 214, 206, x1 - 12 - fx - 214, EDIT_H), "")));
        c.push(l(check(XHIDE, (fx, 236, 330, CHECK_H), &tr("net.x.hide"))));
        c.push(l(check(XREQ, (fx, 258, 330, CHECK_H), &tr("net.x.require"))));
        c.push(l(field_label(320, x0, 304, 164, &tr("net.nickserv"))));
        c.push(l(password(NS, (x0 + 170, 304, 200, EDIT_H), "")));
        c.push(l(text(322, (x0, 338, x1 - x0, 40), &tr("net.login_note"))));
        // --- Perform ---
        let p = |c: ControlSpec| c.in_tab(TAB_PERFORM);
        c.push(p(text(400, (x0, 40, x1 - x0, LABEL_H), &tr("net.perform"))));
        c.push(p(text_area(PERF, (x0, 60, x1 - x0, H - 2 * MARGIN - BTN_H - 8 - 60), "")));
        c.extend(ok_cancel(lang, W, H, OK, CANCEL, None));
        spec(&title, W, H, c)
    }

    fn init(&mut self, view: &mut DialogView, cx: &mut Cx) {
        let lang = cx.lang;
        view.set_columns(LIST, &[(t(lang, "net.col.host"), 170), (t(lang, "net.col.port"), 52), (t(lang, "net.col.tls"), 44), (t(lang, "net.col.cert"), 104)]);
        let n = self.net.clone();
        view.set_text(NAME, &n.name);
        self.fill_servers(view, lang);
        self.load_server(view);
        view.set_text(NICK, n.nick.as_deref().unwrap_or(""));
        view.set_text(ALT, n.alt_nick.as_deref().unwrap_or(""));
        view.set_text(REAL, n.realname.as_deref().unwrap_or(""));
        view.set_text(USER, n.username.as_deref().unwrap_or(""));
        view.set_selection(ENC, &[ENCODINGS.iter().position(|e| e.eq_ignore_ascii_case(&n.encoding)).unwrap_or(0)]);
        view.set_checked(AUTO, n.auto_connect);
        view.set_checked(BOUNCER, n.bouncer);
        let mech = n.sasl.as_ref().map(|s| MECHS.iter().position(|m| m.eq_ignore_ascii_case(&s.mechanism)).unwrap_or(1)).unwrap_or(0);
        view.set_selection(MECH, &[mech]);
        if let Some(s) = &n.sasl {
            view.set_text(ACC, &s.account);
            view.set_text(SPW, &s.password);
            view.set_text(CERT, &s.client_cert.as_ref().map(|p| p.display().to_string()).unwrap_or_default());
        }
        let x = n.undernet_x.clone().unwrap_or_default();
        view.set_text(XU, &x.username);
        view.set_text(XP, &x.password);
        view.set_checked(XHIDE, x.hide_host);
        view.set_checked(XREQ, x.require_x);
        view.set_text(NS, n.nickserv_password.as_deref().unwrap_or(""));
        view.set_text(PERF, &n.perform.join("\n"));
        self.update_login(view);
        view.set_focus(NAME);
    }

    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, cx: &mut Cx) -> Outcome {
        let lang = cx.lang;
        match msg {
            DialogMsg::Ok(_) => {
                let n = self.collect(view, lang);
                if n.name.is_empty() {
                    return self.refuse(cx, "net.err_name");
                }
                if n.servers.is_empty() {
                    return self.refuse(cx, "net.err_servers");
                }
                match (self.on_ok)(n, cx) {
                    Ok(()) => Outcome::Close,
                    Err(e) => {
                        cx.actions.push(Action::Open(Box::new(MessageBox::info(&view.title, &e, lang))));
                        Outcome::Keep
                    }
                }
            }
            DialogMsg::Cancel => Outcome::Close,
            DialogMsg::SelectionChanged(LIST) => {
                self.store_server(view, lang);
                self.sel = sel(view, LIST);
                self.load_server(view);
                Outcome::Keep
            }
            DialogMsg::Changed(HOST | PORT | FP | PASS) | DialogMsg::SelectionChanged(VERIFY) => {
                self.store_server(view, lang);
                self.update_enables(view);
                Outcome::Keep
            }
            DialogMsg::Clicked(TLS) => {
                // Port habituel du chiffrement, comme le font les réseaux.
                let port = text_of(view, PORT);
                if view.checked(TLS) && port.trim() == "6667" {
                    view.set_text(PORT, "6697");
                } else if !view.checked(TLS) && port.trim() == "6697" {
                    view.set_text(PORT, "6667");
                }
                self.store_server(view, lang);
                self.update_enables(view);
                Outcome::Keep
            }
            DialogMsg::Clicked(ADD) => {
                self.store_server(view, lang);
                self.net.servers.push(Server::new("", 6697, true));
                self.sel = Some(self.net.servers.len() - 1);
                self.fill_servers(view, lang);
                self.load_server(view);
                view.set_focus(HOST);
                Outcome::Keep
            }
            DialogMsg::Clicked(REMOVE) => {
                if let Some(i) = self.sel.filter(|&i| i < self.net.servers.len()) {
                    self.net.servers.remove(i);
                    self.sel = if self.net.servers.is_empty() { None } else { Some(i.min(self.net.servers.len() - 1)) };
                    self.fill_servers(view, lang);
                    self.load_server(view);
                }
                Outcome::Keep
            }
            DialogMsg::Clicked(id @ (UP | DOWN)) => {
                self.store_server(view, lang);
                if let Some(i) = self.sel {
                    let j = if id == UP { i.checked_sub(1) } else { Some(i + 1).filter(|&j| j < self.net.servers.len()) };
                    if let Some(j) = j {
                        self.net.servers.swap(i, j);
                        self.sel = Some(j);
                        self.fill_servers(view, lang);
                        self.load_server(view);
                    }
                }
                Outcome::Keep
            }
            DialogMsg::SelectionChanged(MECH) => {
                self.update_login(view);
                Outcome::Keep
            }
            DialogMsg::Clicked(CERT_BROWSE) => {
                // Le fichier PEM (certificat et clé) : la réponse revient par `file_picked`.
                cx.actions.push(Action::PickFile { kind: "network".into(), save: false, suggested: None });
                Outcome::Keep
            }
            _ => Outcome::Keep,
        }
    }

    fn file_picked(&mut self, path: Option<PathBuf>, view: &mut DialogView, _cx: &mut Cx) {
        if let Some(p) = path {
            view.set_text(CERT, &p.display().to_string());
        }
    }
}

impl NetworkEditor {
    fn refuse(&self, cx: &mut Cx, key: &str) -> Outcome {
        let title = if self.is_new { t(cx.lang, "net.title_new") } else { tf(cx.lang, "net.title", &[("name", &self.net.name)]) };
        cx.actions.push(Action::Open(Box::new(MessageBox::info(&title, &t(cx.lang, key), cx.lang))));
        Outcome::Keep
    }

    /// Champs SASL utiles selon le mécanisme.
    fn update_login(&self, view: &mut DialogView) {
        let mech = sel(view, MECH).unwrap_or(0);
        view.set_enabled(ACC, mech == 1 || mech == 2);
        view.set_enabled(SPW, mech == 1 || mech == 2);
        view.set_enabled(CERT, mech == 3);
        view.set_enabled(CERT_BROWSE, mech == 3);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testkit::{Kit, assert_translated};
    use std::cell::RefCell;
    use std::rc::Rc;

    fn editor(got: Rc<RefCell<Option<Network>>>) -> NetworkEditor {
        let mut n = Network::new("Undernet", vec![Server::new("irc.undernet.org", 6667, false)]);
        n.perform = vec!["/join #lyon".into()];
        NetworkEditor::new(
            n,
            false,
            Box::new(move |n, _| {
                *got.borrow_mut() = Some(n);
                Ok(())
            }),
        )
    }

    #[test]
    fn edit_servers_login_and_perform() {
        let mut k = Kit::new("network");
        let got = Rc::new(RefCell::new(None));
        let mut e = editor(got.clone());
        assert_translated(&e);
        let (mut v, _) = k.open(&mut e);
        assert_eq!(v.items(LIST).len(), 1);
        // Un second serveur, en TLS épinglé.
        k.send(&mut e, &mut v, DialogMsg::Clicked(ADD));
        v.set_text(HOST, "eu.undernet.org");
        k.send(&mut e, &mut v, DialogMsg::Changed(HOST));
        v.set_selection(VERIFY, &[2]);
        k.send(&mut e, &mut v, DialogMsg::SelectionChanged(VERIFY));
        assert!(v.enabled(FP));
        v.set_text(FP, "AB:CD:EF");
        k.send(&mut e, &mut v, DialogMsg::Changed(FP));
        // Le monter en tête.
        k.send(&mut e, &mut v, DialogMsg::Clicked(UP));
        // SASL, X, Perform.
        v.set_selection(MECH, &[1]);
        v.set_text(ACC, "guiwald");
        v.set_text(SPW, "secret");
        v.set_text(XU, "Guiwald");
        v.set_text(PERF, "/join #lyon\n\n  /mode $me +x  \n");
        v.set_text(NICK, "Gui");
        let (o, _) = k.send(&mut e, &mut v, DialogMsg::Ok(OK));
        assert_eq!(o, Outcome::Close);
        let n = got.borrow().clone().unwrap();
        assert_eq!(n.servers.len(), 2);
        assert_eq!(n.servers[0].host, "eu.undernet.org");
        assert_eq!((n.servers[0].port, n.servers[0].tls, n.servers[0].verify), (6697, true, Verify::Pinned));
        assert_eq!(n.servers[0].fingerprint.as_deref(), Some("abcdef"));
        assert_eq!(n.sasl.as_ref().map(|s| (s.mechanism.as_str(), s.account.as_str())), Some(("PLAIN", "guiwald")));
        assert_eq!(n.undernet_x.as_ref().map(|x| x.username.as_str()), Some("Guiwald"));
        assert_eq!(n.perform, vec!["/join #lyon", "/mode $me +x"]);
        assert_eq!(n.nick.as_deref(), Some("Gui"));
        assert_eq!(n.alt_nick, None);
    }

    #[test]
    fn refuses_empty_name() {
        let mut k = Kit::new("network2");
        let got = Rc::new(RefCell::new(None));
        let mut e = editor(got.clone());
        let (mut v, _) = k.open(&mut e);
        v.set_text(NAME, "  ");
        let (o, a) = k.send(&mut e, &mut v, DialogMsg::Ok(OK));
        assert_eq!(o, Outcome::Keep);
        assert_eq!(a.len(), 1);
        assert!(got.borrow().is_none());
    }
}
