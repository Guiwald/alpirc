//! Dossier des canaux (Channels Folder, Alt+J) : les favoris (canal, réseau, clé, description, rejoindre à la
//! connexion), à rejoindre d'un double-clic sur la bonne connexion (ouverte au besoin). Comme dans mIRC, les
//! changements de la liste sont enregistrés tout de suite.

use alpirc_core::i18n::Lang;
use alpirc_core::settings::Favorite;
use alpirc_ui::{ControlSpec, DialogMsg, DialogSpec, DialogView};

use super::form::{Field, Form, Val};
use super::*;

const W: i32 = 500;
const H: i32 = 380;

const CHAN: u32 = 10;
const JOIN: u32 = 11;
const FAVS: u32 = 13;
const ADD: u32 = 14;
const EDIT: u32 = 15;
const DELETE: u32 = 16;
const CLOSE: u32 = 2;

/// Le dossier des canaux.
pub struct ChannelsFolder {
    shown: Vec<Favorite>,
}

/// Réseau de la fenêtre active (pour un nouveau favori).
fn current_network(client: &Client) -> String {
    status_window(client).and_then(|w| client.window(w)).and_then(|w| w.conn).and_then(|c| client.conn_info(c)).map(|c| c.network).unwrap_or_default()
}

impl ChannelsFolder {
    pub fn new(client: &Client) -> ChannelsFolder {
        ChannelsFolder { shown: client.settings().favorites.clone() }
    }

    fn fill(&mut self, view: &mut DialogView, cx: &Cx, select: Option<usize>) {
        self.shown = cx.client.settings().favorites.clone();
        let lang = cx.lang;
        let rows: Vec<String> = self
            .shown
            .iter()
            .map(|f| row(&[f.channel.clone(), f.network.clone(), f.description.clone(), if f.join_on_connect { t(lang, "net.yes") } else { String::new() }]))
            .collect();
        let n = rows.len();
        view.set_items(FAVS, &rows);
        let names: Vec<String> = self.shown.iter().map(|f| f.channel.clone()).collect();
        let text = text_of(view, CHAN);
        view.set_items(CHAN, &names);
        view.set_text(CHAN, &text);
        select_row(view, FAVS, select.filter(|&i| i < n));
        self.update_buttons(view);
    }

    fn update_buttons(&self, view: &mut DialogView) {
        let on = sel(view, FAVS).is_some();
        view.set_enabled(EDIT, on);
        view.set_enabled(DELETE, on);
    }

    fn edit(&self, i: Option<usize>, cx: &mut Cx) {
        let lang = cx.lang;
        let f = match i {
            Some(i) => match self.shown.get(i) {
                Some(f) => f.clone(),
                None => return,
            },
            None => Favorite { channel: "#".into(), network: current_network(cx.client), ..Default::default() },
        };
        let nets: Vec<String> = cx.client.settings().networks.iter().map(|n| n.name.clone()).collect();
        let fields = vec![
            Field::text(&t(lang, "fold.channel"), &f.channel),
            Field::combo(&t(lang, "fold.network"), nets, &f.network),
            Field::password(&t(lang, "fold.key"), f.key.as_deref().unwrap_or("")),
            Field::text(&t(lang, "fold.description"), &f.description),
            Field::check(&t(lang, "fold.autojoin"), f.join_on_connect),
        ];
        let orig = i.and_then(|i| self.shown.get(i).cloned());
        let form = Form::new(
            &t(lang, if i.is_some() { "fold.edit_title" } else { "fold.add_title" }),
            "favorite",
            fields,
            Box::new(move |v: &[Val], cx| {
                let mut channel = v[0].text();
                if channel.is_empty() || channel == "#" || channel.contains([' ', ',']) {
                    return Err(t(lang, "fold.err_channel"));
                }
                if !channel.starts_with(['#', '&', '+', '!']) {
                    channel.insert(0, '#');
                }
                let fav = Favorite { channel, network: v[1].text(), key: v[2].opt(), description: v[3].text(), join_on_connect: v[4].on() };
                let favs = &mut cx.client.settings_mut().favorites;
                let same = |a: &Favorite, b: &Favorite| a.channel.eq_ignore_ascii_case(&b.channel) && a.network.eq_ignore_ascii_case(&b.network);
                let pos = orig.as_ref().and_then(|o| favs.iter().position(|x| x == o));
                if favs.iter().enumerate().any(|(k, x)| Some(k) != pos && same(x, &fav)) {
                    return Err(t(lang, "fold.err_exists"));
                }
                match pos {
                    Some(k) => favs[k] = fav,
                    None => favs.push(fav),
                }
                cx.actions.push(Action::SettingsChanged);
                Ok(())
            }),
        );
        cx.actions.push(Action::Open(Box::new(form.width(380))));
    }

    /// Rejoint le canal du champ, ou le favori choisi.
    fn join(&self, view: &DialogView, cx: &mut Cx) -> bool {
        let typed = text_of(view, CHAN).trim().to_string();
        let fav = if typed.is_empty() || typed == "#" {
            sel(view, FAVS).and_then(|i| self.shown.get(i)).cloned()
        } else {
            // Un favori de ce nom (sur le réseau de la fenêtre active d'abord), sinon le canal tapé sur la
            // connexion active.
            let net = current_network(cx.client);
            let found = self.shown.iter().filter(|f| f.channel.eq_ignore_ascii_case(&typed)).max_by_key(|f| f.network.eq_ignore_ascii_case(&net)).cloned();
            if found.is_none() {
                let chan = if typed.starts_with(['#', '&', '+', '!']) { typed.clone() } else { format!("#{typed}") };
                return match status_window(cx.client) {
                    Some(w) => {
                        cx.actions.push(Action::Command(w, format!("/join {chan}")));
                        true
                    }
                    None => false,
                };
            }
            found
        };
        match fav {
            Some(f) => {
                cx.actions.push(Action::JoinFavorite(f));
                true
            }
            None => false,
        }
    }
}

impl DialogCtl for ChannelsFolder {
    fn kind(&self) -> Option<String> {
        Some("folder".into())
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let tr = |k: &str| t(lang, k);
        let bx = W - MARGIN - 96;
        let cols = [tr("fold.col.channel"), tr("fold.col.network"), tr("fold.col.description"), tr("fold.col.auto")];
        let c: Vec<ControlSpec> = vec![
            text(9, (MARGIN, MARGIN, W - 2 * MARGIN, LABEL_H), &tr("fold.prompt")),
            combo_edit(CHAN, (MARGIN, 26, bx - 8 - MARGIN, EDIT_H + 8 * 16), &[] as &[&str], ""),
            button(JOIN, (bx, 26, 96, BTN_H), &tr("fold.join")).style("default"),
            text(12, (MARGIN, 60, W - 2 * MARGIN, LABEL_H), &tr("fold.favorites")),
            list_view(FAVS, (MARGIN, 78, bx - 8 - MARGIN, H - 78 - MARGIN - BTN_H - 8), &cols),
            button(ADD, (bx, 78, 96, BTN_H), &tr("dlg.add_dots")),
            button(EDIT, (bx, 105, 96, BTN_H), &tr("dlg.edit_dots")),
            button(DELETE, (bx, 132, 96, BTN_H), &tr("dlg.delete")),
            button(CLOSE, (W - MARGIN - BTN_W, H - MARGIN - BTN_H, BTN_W, BTN_H), &tr("dlg.close")).style("cancel"),
        ];
        spec(&tr("fold.title"), W, H, c)
    }

    fn init(&mut self, view: &mut DialogView, cx: &mut Cx) {
        let lang = cx.lang;
        view.set_columns(
            FAVS,
            &[(t(lang, "fold.col.channel"), 110), (t(lang, "fold.col.network"), 100), (t(lang, "fold.col.description"), 112), (t(lang, "fold.col.auto"), 48)],
        );
        self.fill(view, cx, None);
        view.set_text(CHAN, "#");
        view.set_focus(CHAN);
    }

    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, cx: &mut Cx) -> Outcome {
        match msg {
            DialogMsg::Clicked(JOIN) | DialogMsg::Ok(_) => {
                if self.join(view, cx) {
                    Outcome::Close
                } else {
                    Outcome::Keep
                }
            }
            DialogMsg::DoubleClicked(FAVS) => {
                view.set_text(CHAN, "");
                if self.join(view, cx) { Outcome::Close } else { Outcome::Keep }
            }
            DialogMsg::SelectionChanged(FAVS) => {
                if let Some(f) = sel(view, FAVS).and_then(|i| self.shown.get(i)) {
                    view.set_text(CHAN, &f.channel);
                }
                self.update_buttons(view);
                Outcome::Keep
            }
            DialogMsg::SelectionChanged(CHAN) => {
                // Choisi dans la liste déroulante : le favori correspondant.
                if let Some(i) = sel(view, CHAN) {
                    select_row(view, FAVS, Some(i));
                    self.update_buttons(view);
                }
                Outcome::Keep
            }
            DialogMsg::Clicked(ADD) => {
                self.edit(None, cx);
                Outcome::Keep
            }
            DialogMsg::Clicked(EDIT) => {
                if let Some(i) = sel(view, FAVS) {
                    self.edit(Some(i), cx);
                }
                Outcome::Keep
            }
            DialogMsg::Clicked(DELETE) => {
                if let Some(i) = sel(view, FAVS)
                    && let Some(f) = self.shown.get(i).cloned()
                {
                    cx.client.settings_mut().favorites.retain(|x| *x != f);
                    cx.actions.push(Action::SettingsChanged);
                    self.fill(view, cx, Some(i.saturating_sub(1)));
                }
                Outcome::Keep
            }
            DialogMsg::Cancel => Outcome::Close,
            _ => Outcome::Keep,
        }
    }

    fn refresh(&mut self, view: &mut DialogView, cx: &mut Cx) {
        if cx.client.settings().favorites != self.shown {
            let n = cx.client.settings().favorites.len();
            let pick = if n > self.shown.len() { Some(n - 1) } else { sel(view, FAVS) };
            self.fill(view, cx, pick);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testkit::{Kit, assert_translated};

    #[test]
    fn add_join_delete() {
        let mut k = Kit::new("folder");
        let mut d = ChannelsFolder::new(&k.client);
        assert_translated(&d);
        let (mut v, _) = k.open(&mut d);
        let n0 = v.items(FAVS).len();
        let (_, mut a) = k.send(&mut d, &mut v, DialogMsg::Clicked(ADD));
        let Some(Action::Open(mut f)) = a.pop() else { panic!() };
        let (mut fv, _) = k.open(f.as_mut());
        fv.set_text(11, "dlg-test");
        fv.set_text(13, "Local");
        fv.set_text(15, "clé");
        fv.set_checked(19, true);
        let (o, a) = k.send(f.as_mut(), &mut fv, DialogMsg::Ok(1));
        assert_eq!(o, Outcome::Close);
        assert!(matches!(a.first(), Some(Action::SettingsChanged)));
        k.refresh(&mut d, &mut v);
        assert_eq!(v.items(FAVS).len(), n0 + 1);
        let fav = k.client.settings().favorites.last().unwrap().clone();
        assert_eq!((fav.channel.as_str(), fav.network.as_str(), fav.key.as_deref(), fav.join_on_connect), ("#dlg-test", "Local", Some("clé"), true));
        // Double-clic : rejoindre ce favori.
        assert_eq!(sel(&v, FAVS), Some(n0));
        let (o, a) = k.send(&mut d, &mut v, DialogMsg::DoubleClicked(FAVS));
        assert_eq!(o, Outcome::Close);
        assert!(matches!(a.first(), Some(Action::JoinFavorite(f)) if f.channel == "#dlg-test"));
        // Canal tapé, hors favoris : /join sur la connexion active.
        v.set_text(CHAN, "autre");
        let (_, a) = k.send(&mut d, &mut v, DialogMsg::Clicked(JOIN));
        assert!(matches!(a.first(), Some(Action::Command(_, c)) if c == "/join #autre"));
        // Supprimer.
        v.set_selection(FAVS, &[n0]);
        k.send(&mut d, &mut v, DialogMsg::Clicked(DELETE));
        assert_eq!(k.client.settings().favorites.len(), n0);
    }
}
