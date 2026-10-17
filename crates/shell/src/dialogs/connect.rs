//! Choix du serveur (Fichier › Choisir un serveur, ou connexion sans réseau configuré) : les réseaux à gauche,
//! leurs serveurs à droite ; se connecter dans la fenêtre Status courante ou dans une nouvelle ; ajouter ou
//! modifier un réseau avec l'éditeur de réseau, qui écrit directement dans les réglages.

use alpirc_core::i18n::Lang;
use alpirc_core::settings::{Network, Server};
use alpirc_ui::{ControlSpec, DialogMsg, DialogSpec, DialogView};

use super::network::NetworkEditor;
use super::*;

const W: i32 = 540;
const H: i32 = 330;

const NETS: u32 = 10;
const SERVERS: u32 = 12;
const ADD: u32 = 13;
const EDIT: u32 = 14;
const TARGET: u32 = 15;
const CONNECT: u32 = 1;
const CONNECT_NEW: u32 = 2;
const CLOSE: u32 = 3;

/// La boîte « Choisir un serveur ».
pub struct SelectServer {
    /// Fenêtre Status où se connecter.
    status: Option<WinId>,
    /// Réseaux affichés (pour voir les changements faits ailleurs).
    shown: Vec<Network>,
}

impl SelectServer {
    pub fn new(client: &Client) -> SelectServer {
        SelectServer { status: status_window(client), shown: Vec::new() }
    }

    fn fill(&mut self, view: &mut DialogView, cx: &Cx, keep: Option<String>) {
        self.shown = cx.client.settings().networks.clone();
        let names: Vec<String> = self.shown.iter().map(|n| n.name.clone()).collect();
        view.set_items(NETS, &names);
        let i = keep.and_then(|k| names.iter().position(|n| n.eq_ignore_ascii_case(&k))).or(if names.is_empty() { None } else { Some(0) });
        select_row(view, NETS, i);
        self.fill_servers(view, cx.lang);
    }

    fn fill_servers(&self, view: &mut DialogView, lang: Lang) {
        let rows: Vec<String> = sel(view, NETS)
            .and_then(|i| self.shown.get(i))
            .map(|n| n.servers.iter().map(|s| row(&[s.host.clone(), s.port.to_string(), if s.tls { t(lang, "net.yes") } else { String::new() }])).collect())
            .unwrap_or_default();
        let empty = rows.is_empty();
        view.set_items(SERVERS, &rows);
        select_row(view, SERVERS, if empty { None } else { Some(0) });
        view.set_enabled(CONNECT, !empty);
        view.set_enabled(CONNECT_NEW, !empty);
        view.set_enabled(EDIT, sel(view, NETS).is_some());
    }

    fn connect(&self, view: &DialogView, cx: &mut Cx, new_window: bool) -> bool {
        let Some(n) = sel(view, NETS).and_then(|i| self.shown.get(i)) else { return false };
        let server = sel(view, SERVERS);
        let status = match self.status {
            Some(w) if !new_window => w,
            _ => {
                let w = cx.client.new_status(cx.now);
                cx.client.set_active(w, cx.now);
                w
            }
        };
        cx.client.connect(status, &n.name, server, cx.now);
        true
    }

    fn edit(&self, i: Option<usize>, cx: &mut Cx) {
        let lang = cx.lang;
        let (net, origin) = match i.and_then(|i| self.shown.get(i)) {
            Some(n) => (n.clone(), Some(n.name.clone())),
            None => (Network::new("", vec![Server::new("", 6697, true)]), None),
        };
        let ed = NetworkEditor::new(
            net,
            origin.is_none(),
            Box::new(move |mut n, cx| {
                let s = cx.client.settings_mut();
                let taken =
                    s.networks.iter().any(|x| x.name.eq_ignore_ascii_case(&n.name) && origin.as_deref().is_none_or(|o| !o.eq_ignore_ascii_case(&x.name)));
                if taken {
                    return Err(tf(lang, "opt.err_net_exists", &[("name", &n.name)]));
                }
                match origin.as_deref().and_then(|o| s.networks.iter().position(|x| x.name.eq_ignore_ascii_case(o))) {
                    Some(k) => {
                        n.sts = s.networks[k].sts.clone();
                        n.bouncer_networks = s.networks[k].bouncer_networks.clone();
                        s.networks[k] = n;
                    }
                    None => s.networks.push(n),
                }
                cx.actions.push(Action::SettingsChanged);
                Ok(())
            }),
        );
        cx.actions.push(Action::Open(Box::new(ed)));
    }
}

impl DialogCtl for SelectServer {
    fn kind(&self) -> Option<String> {
        Some("connect".into())
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let tr = |k: &str| t(lang, k);
        let lw = 160;
        let rx = MARGIN + lw + 10;
        let rw = W - rx - MARGIN;
        let by = H - MARGIN - BTN_H;
        let cols = [tr("net.col.host"), tr("net.col.port"), tr("net.col.tls")];
        let c: Vec<ControlSpec> = vec![
            text(9, (MARGIN, MARGIN, lw, LABEL_H), &tr("srv.networks")),
            list_box(NETS, (MARGIN, 26, lw, by - 26 - 8 - BTN_H - 6)),
            button(ADD, (MARGIN, by - BTN_H - 8, (lw - 6) / 2, BTN_H), &tr("dlg.add_dots")),
            button(EDIT, (MARGIN + (lw - 6) / 2 + 6, by - BTN_H - 8, (lw - 6) / 2, BTN_H), &tr("dlg.edit_dots")),
            text(11, (rx, MARGIN, rw, LABEL_H), &tr("srv.servers")),
            list_view(SERVERS, (rx, 26, rw, by - 26 - 8 - 40), &cols),
            text(TARGET, (rx, by - 8 - 34, rw, 32), ""),
            button(CONNECT, (rx, by, 92, BTN_H), &tr("srv.connect")).style("default ok"),
            button(CONNECT_NEW, (rx + 98, by, 172, BTN_H), &tr("srv.connect_new")),
            button(CLOSE, (W - MARGIN - BTN_W, by, BTN_W, BTN_H), &tr("dlg.close")).style("cancel"),
        ];
        spec(&tr("srv.title"), W, H, c)
    }

    fn init(&mut self, view: &mut DialogView, cx: &mut Cx) {
        let lang = cx.lang;
        view.set_columns(SERVERS, &[(t(lang, "net.col.host"), 200), (t(lang, "net.col.port"), 56), (t(lang, "net.col.tls"), 44)]);
        let current = self.status.and_then(|w| cx.client.window(w)).and_then(|w| w.conn).and_then(|c| cx.client.conn_info(c));
        let keep = current.as_ref().map(|c| c.network.clone()).filter(|n| !n.is_empty());
        self.fill(view, cx, keep);
        let target = match self.status.and_then(|w| cx.client.window(w)) {
            Some(w) => tf(lang, "srv.target", &[("win", &w.title)]),
            None => String::new(),
        };
        view.set_text(TARGET, &target);
        view.set_focus(NETS);
    }

    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, cx: &mut Cx) -> Outcome {
        match msg {
            DialogMsg::Ok(_) | DialogMsg::DoubleClicked(SERVERS) => {
                if self.connect(view, cx, false) {
                    Outcome::Close
                } else {
                    Outcome::Keep
                }
            }
            DialogMsg::Clicked(CONNECT_NEW) => {
                if self.connect(view, cx, true) {
                    Outcome::Close
                } else {
                    Outcome::Keep
                }
            }
            DialogMsg::Cancel => Outcome::Close,
            DialogMsg::SelectionChanged(NETS) => {
                self.fill_servers(view, cx.lang);
                Outcome::Keep
            }
            DialogMsg::Clicked(ADD) => {
                self.edit(None, cx);
                Outcome::Keep
            }
            DialogMsg::Clicked(EDIT) | DialogMsg::DoubleClicked(NETS) => {
                if let Some(i) = sel(view, NETS) {
                    self.edit(Some(i), cx);
                }
                Outcome::Keep
            }
            _ => Outcome::Keep,
        }
    }

    fn refresh(&mut self, view: &mut DialogView, cx: &mut Cx) {
        if cx.client.settings().networks != self.shown {
            let nets = &cx.client.settings().networks;
            // Un réseau ajouté : on le choisit ; sinon on garde la même ligne (le réseau a pu être renommé).
            let keep = if nets.len() > self.shown.len() {
                nets.last().map(|x| x.name.clone())
            } else {
                sel(view, NETS).and_then(|i| nets.get(i)).map(|x| x.name.clone())
            };
            self.fill(view, cx, keep);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testkit::{Kit, assert_translated};

    #[test]
    fn connect_to_chosen_server() {
        let mut k = Kit::new("connect");
        k.client.settings_mut().networks.insert(0, Network::new("Local", vec![Server::new("127.0.0.1", 16667, false), Server::new("localhost", 16667, false)]));
        let mut d = SelectServer::new(&k.client);
        assert_translated(&d);
        let (mut v, _) = k.open(&mut d);
        assert_eq!(v.items(NETS)[0], "Local");
        assert_eq!(v.items(SERVERS).len(), 2);
        v.set_selection(SERVERS, &[1]);
        let (o, _) = k.send(&mut d, &mut v, DialogMsg::Ok(CONNECT));
        assert_eq!(o, Outcome::Close);
        let fx = k.client.take_effects();
        assert!(fx.iter().any(|e| matches!(e, alpirc_core::Effect::Connect { host, port: 16667, .. } if host == "localhost")), "{fx:?}");
    }

    #[test]
    fn edit_writes_settings() {
        let mut k = Kit::new("connect2");
        let mut d = SelectServer::new(&k.client);
        let (mut v, _) = k.open(&mut d);
        v.set_selection(NETS, &[0]);
        let (_, mut a) = k.send(&mut d, &mut v, DialogMsg::Clicked(EDIT));
        let Some(Action::Open(mut ed)) = a.pop() else { panic!() };
        let (mut ev, _) = k.open(ed.as_mut());
        ev.set_text(110, "eu.undernet.org");
        k.send(ed.as_mut(), &mut ev, DialogMsg::Changed(110));
        let (o, a) = k.send(ed.as_mut(), &mut ev, DialogMsg::Ok(1));
        assert_eq!(o, Outcome::Close);
        assert!(a.iter().any(|a| matches!(a, Action::SettingsChanged)));
        assert_eq!(k.client.settings().networks[0].servers[0].host, "eu.undernet.org");
        k.refresh(&mut d, &mut v);
        assert!(v.items(SERVERS)[0].starts_with("eu.undernet.org"));
    }
}
