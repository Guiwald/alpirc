//! Boîte « Couleurs » (Alt+K), à la manière de mIRC : la liste des éléments (texte des actions, des arrivées,
//! des avis…, fonds de la discussion, de la ligne de saisie et de la liste des pseudos) et les 16 couleurs de
//! la palette ; un clic sur une couleur la donne à l'élément choisi. La couleur choisie se modifie en
//! hexadécimal (`#RRGGBB`). Un aperçu montre le résultat.
//!
//! Thèmes, en bas à gauche : les thèmes fournis (Classique, Sombre), qui ne se modifient pas, puis ceux de
//! l'utilisateur. En choisir un charge ses couleurs ; « Enregistrer » garde les couleurs montrées sous un nom (un
//! thème de l'utilisateur, remplacé s'il existe) ; « Supprimer » retire le thème de l'utilisateur choisi ;
//! « Rétablir » recharge les couleurs du thème choisi. Enregistrer et supprimer valent tout de suite ; les
//! couleurs et le thème choisis, seulement à OK.

use std::cell::RefCell;
use std::rc::Rc;

use alpirc_core::i18n::Lang;
use alpirc_core::settings::{DisplayOptions, EventColors, ThemeNameError, UserTheme};
use alpirc_ui::dialog::Widget;
use alpirc_ui::widgets::Swatch;
use alpirc_ui::{ControlKind, ControlSpec, DialogMsg, DialogSpec, DialogView};

use super::form::{Field, Form, Val};
use super::*;

const W: i32 = 560;
const H: i32 = 440;

const OK: u32 = 1;
const CANCEL: u32 = 2;
const ITEMS: u32 = 10;
/// Pastilles : `SW + i` ; leurs numéros : `SW_L + i`.
const SW: u32 = 100;
const SW_L: u32 = 120;
const HEX: u32 = 141;
const PREV_BG: u32 = 150;
const PREV: u32 = 151;
const LIST_BG: u32 = 160;
const LIST: u32 = 161;
const EDIT_PREV: u32 = 170;
const SCHEME: u32 = 181;
const RESET: u32 = 182;
const SAVE: u32 = 183;
const DELETE: u32 = 184;

/// Un élément de la liste : clé du texte, lecture et écriture de son numéro de couleur.
struct Item {
    key: &'static str,
    get: fn(&EventColors) -> u8,
    set: fn(&mut EventColors, u8),
}

macro_rules! item {
    ($key:expr, $f:ident) => {
        Item { key: $key, get: |c| c.$f, set: |c, v| c.$f = v }
    };
}

const ITEM_LIST: [Item; 26] = [
    item!("col.item.action", action),
    item!("col.item.ctcp", ctcp),
    item!("col.item.highlight", highlight),
    item!("col.item.info", info),
    item!("col.item.info2", info2),
    item!("col.item.invite", invite),
    item!("col.item.join", join),
    item!("col.item.kick", kick),
    item!("col.item.mode", mode),
    item!("col.item.nick", nick),
    item!("col.item.normal", normal),
    item!("col.item.notice", notice),
    item!("col.item.notify", notify),
    item!("col.item.other", other),
    item!("col.item.own", own),
    item!("col.item.part", part),
    item!("col.item.quit", quit),
    item!("col.item.topic", topic),
    item!("col.item.wallops", wallops),
    item!("col.item.whois", whois),
    item!("col.item.error", error),
    item!("col.item.background", background),
    item!("col.item.editbox", editbox_bg),
    item!("col.item.editbox_text", editbox_text),
    item!("col.item.listbox", listbox_bg),
    item!("col.item.listbox_text", listbox_text),
];

/// Lignes de l'aperçu : (clé du texte, couleur).
fn preview_lines(c: &EventColors) -> [(&'static str, u8); 6] {
    [
        ("col.prev.normal", c.normal),
        ("col.prev.action", c.action),
        ("col.prev.join", c.join),
        ("col.prev.notice", c.notice),
        ("col.prev.highlight", c.highlight),
        ("col.prev.own", c.own),
    ]
}

/// Nom affiché d'un thème : les thèmes fournis traduits, ceux de l'utilisateur tels quels.
fn theme_label(lang: Lang, name: &str) -> String {
    match alpirc_core::settings::theme(name) {
        Some(th) => t(lang, &format!("col.scheme.{}", th.name)),
        None => name.to_string(),
    }
}

/// Ce que la petite boîte « Enregistrer » rend à la grande : le nom du thème enregistré.
#[derive(Default)]
struct Saved {
    name: Option<String>,
    version: u64,
}

/// La boîte Couleurs.
pub struct Colors {
    palette: [u32; 16],
    colors: EventColors,
    /// Thème choisi (nom des réglages).
    theme: String,
    item: usize,
    swatch: usize,
    /// Thèmes proposés : fournis, puis ceux de l'utilisateur.
    names: Vec<String>,
    saved: Rc<RefCell<Saved>>,
    seen: u64,
}

impl Colors {
    pub fn new(client: &Client) -> Colors {
        let d = &client.settings().display;
        // Les couleurs que l'interface montre (thème fourni choisi par le menu, couleurs retouchées…).
        let (palette, colors) = d.effective_colors();
        Colors { palette, colors, theme: d.theme.clone(), item: 10, swatch: 0, names: d.theme_names(), saved: Default::default(), seen: 0 }
    }

    fn rgb(&self, i: u8) -> u32 {
        self.palette.get(i as usize).copied().unwrap_or(0)
    }

    /// Met la boîte à jour : pastilles, choix, aperçu.
    fn show(&mut self, view: &mut DialogView) {
        let cur = (ITEM_LIST[self.item].get)(&self.colors) as usize;
        self.swatch = cur.min(15);
        for i in 0..16 {
            let id = SW + i as u32;
            view.set_text(id, &Swatch::hex(self.palette[i]));
            view.set_checked(id, i == self.swatch);
        }
        view.set_text(HEX, &Swatch::hex(self.palette[self.swatch]));
        self.show_preview(view);
    }

    fn show_preview(&self, view: &mut DialogView) {
        let c = &self.colors;
        let set = |view: &mut DialogView, id: u32, fg: Option<u32>, bg: Option<u32>| {
            if let Some(Widget::Text(l)) = view.widget_mut(id) {
                l.color = fg;
                l.background = bg;
            }
        };
        set(view, PREV_BG, None, Some(self.rgb(c.background)));
        for (k, (_, col)) in preview_lines(c).iter().enumerate() {
            set(view, PREV + k as u32, Some(self.rgb(*col)), None);
        }
        set(view, LIST_BG, None, Some(self.rgb(c.listbox_bg)));
        for k in 0..3 {
            set(view, LIST + k, Some(self.rgb(c.listbox_text)), None);
        }
        set(view, EDIT_PREV, Some(self.rgb(c.editbox_text)), Some(self.rgb(c.editbox_bg)));
    }

    /// Charge les couleurs d'un thème (fourni ou de l'utilisateur) et le choisit.
    fn apply_scheme(&mut self, name: &str, d: &DisplayOptions) {
        if let Some((palette, colors)) = d.theme_colors(name) {
            self.palette = palette;
            self.colors = colors;
            self.theme = d.user_theme(name).map(|u| u.name.clone()).or_else(|| alpirc_core::settings::theme(name).map(|t| t.name.into())).unwrap_or_default();
        }
    }

    /// Thème de base (allure des fenêtres) des couleurs montrées : le thème fourni choisi, ou la base du thème de
    /// l'utilisateur choisi.
    fn base(&self, d: &DisplayOptions) -> String {
        match d.user_theme(&self.theme) {
            Some(u) => u.base.clone(),
            None => alpirc_core::settings::theme(&self.theme).map(|t| t.name.to_string()).unwrap_or_else(|| "classic".into()),
        }
    }

    /// Remplit la liste des thèmes et y choisit le thème en cours.
    fn fill_schemes(&mut self, view: &mut DialogView, lang: Lang, d: &DisplayOptions) {
        self.names = d.theme_names();
        let labels: Vec<String> = self.names.iter().map(|n| plain(&theme_label(lang, n))).collect();
        view.set_items(SCHEME, &labels);
        self.select_scheme(view, d);
    }

    fn select_scheme(&self, view: &mut DialogView, d: &DisplayOptions) {
        let i = self.names.iter().position(|n| n.eq_ignore_ascii_case(&self.theme)).unwrap_or(0);
        view.set_selection(SCHEME, &[i]);
        view.set_enabled(DELETE, d.user_theme(&self.theme).is_some());
    }

    /// « Enregistrer » : demande un nom, puis garde les couleurs montrées sous ce nom.
    fn save(&self, cx: &mut Cx) {
        let lang = cx.lang;
        let d = &cx.client.settings().display;
        let users: Vec<String> = d.user_themes.iter().map(|u| u.name.clone()).collect();
        let current = d.user_theme(&self.theme).map(|u| u.name.clone()).unwrap_or_default();
        let theme = UserTheme { name: String::new(), base: self.base(d), palette: self.palette, colors: self.colors.clone() };
        let saved = self.saved.clone();
        let form = Form::new(
            &t(lang, "col.save_title"),
            "colors-save",
            vec![Field::combo(&t(lang, "col.save_name"), users, &current)],
            Box::new(move |v: &[Val], cx: &mut Cx| {
                let t = UserTheme { name: v[0].text(), ..theme.clone() };
                match cx.client.settings_mut().display.store_theme(t) {
                    Ok(name) => {
                        let mut s = saved.borrow_mut();
                        s.name = Some(name);
                        s.version += 1;
                        cx.actions.push(Action::SettingsChanged);
                        Ok(())
                    }
                    Err(ThemeNameError::Empty) => Err(super::t(lang, "col.err_empty")),
                    Err(ThemeNameError::Builtin) => Err(super::t(lang, "col.err_builtin")),
                }
            }),
        );
        cx.actions.push(Action::Open(Box::new(form)));
    }
}

impl DialogCtl for Colors {
    fn kind(&self) -> Option<String> {
        Some("colors".into())
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let tr = |k: &str| t(lang, k);
        let items: Vec<String> = ITEM_LIST.iter().map(|i| plain(&tr(i.key))).collect();
        let by = H - MARGIN - BTN_H;
        // Sous la liste des éléments : le thème (liste, puis Enregistrer et Supprimer, puis Rétablir).
        let (y1, y2) = (by - 60, by - 30);
        let mut c: Vec<ControlSpec> = vec![
            text(9, (MARGIN, MARGIN, 190, LABEL_H), &tr("col.items")),
            list_box(ITEMS, (MARGIN, 26, 190, y1 - 26 - 10)).items(&items),
            group(190, (208, MARGIN, W - 208 - MARGIN, 172), &tr("col.palette")),
        ];
        for i in 0..16 {
            let (col, r) = (i % 8, i / 8);
            let x = 222 + col * 40;
            let y = 30 + r * 52;
            c.push(ControlSpec::new(SW + i as u32, ControlKind::Swatch, (x, y, 34, 30), &Swatch::hex(self.palette[i as usize])));
            c.push(text(SW_L + i as u32, (x, y + 32, 34, 14), &i.to_string()).style("center nowrap"));
        }
        c.push(field_label(140, 222, 142, 196, &tr("col.rgb")));
        c.push(edit(HEX, (422, 142, 90, EDIT_H), "").style("limit 7"));
        c.push(group(191, (208, 188, W - 208 - MARGIN, 160), &tr("col.preview")));
        let pw = W - 208 - MARGIN - 24 - 90;
        c.push(text(PREV_BG, (220, 208, pw, 104), ""));
        for (k, (s, _)) in preview_lines(&self.colors).iter().enumerate() {
            c.push(text(PREV + k as u32, (224, 210 + k as i32 * 16, pw - 8, 16), &tr(s)).style("nowrap"));
        }
        c.push(text(LIST_BG, (226 + pw, 208, 84, 104), ""));
        for (k, n) in ["@Guiwald", "+Elisa", "Nemo"].iter().enumerate() {
            c.push(text(LIST + k as u32, (230 + pw, 210 + k as i32 * 16, 78, 16), n).style("nowrap"));
        }
        c.push(text(EDIT_PREV, (220, 316, W - 208 - MARGIN - 24, 22), &tr("col.editbox_sample")).style("nowrap"));
        let schemes: Vec<String> = self.names.iter().map(|n| plain(&theme_label(lang, n))).collect();
        c.push(field_label(180, MARGIN, y1, 50, &tr("col.scheme")));
        c.push(drop_list(SCHEME, (MARGIN + 52, y1, 138, EDIT_H + 8 * 16 + 4), &schemes));
        c.push(button(SAVE, (MARGIN, y2, 92, BTN_H), &tr("col.save")));
        c.push(button(DELETE, (MARGIN + 98, y2, 92, BTN_H), &tr("col.delete")));
        c.push(button(RESET, (MARGIN, by, 92, BTN_H), &tr("col.reset")));
        c.extend(ok_cancel(lang, W, H, OK, CANCEL, None));
        spec(&tr("col.title"), W, H, c)
    }

    fn init(&mut self, view: &mut DialogView, cx: &mut Cx) {
        view.set_selection(ITEMS, &[self.item]);
        let d = cx.client.settings().display.clone();
        self.select_scheme(view, &d);
        // L'aperçu dans la police des discussions.
        for id in (PREV..PREV + 6).chain([EDIT_PREV, LIST, LIST + 1, LIST + 2]) {
            if let Some(Widget::Text(l)) = view.widget_mut(id) {
                l.style = alpirc_ui::TextStyle::CHAT;
            }
        }
        self.show(view);
        view.set_focus(ITEMS);
    }

    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, cx: &mut Cx) -> Outcome {
        match msg {
            DialogMsg::Ok(_) => {
                let d = &mut cx.client.settings_mut().display;
                let changed = d.palette != self.palette || d.colors != self.colors || d.theme != self.theme;
                if changed {
                    d.palette = self.palette;
                    d.colors = self.colors.clone();
                    d.theme = self.theme.clone();
                    cx.actions.push(Action::SettingsChanged);
                }
                Outcome::Close
            }
            DialogMsg::Cancel => Outcome::Close,
            DialogMsg::SelectionChanged(ITEMS) => {
                if let Some(i) = sel(view, ITEMS) {
                    self.item = i.min(ITEM_LIST.len() - 1);
                    self.show(view);
                }
                Outcome::Keep
            }
            DialogMsg::Clicked(id) | DialogMsg::DoubleClicked(id) if (SW..SW + 16).contains(&id) => {
                let i = (id - SW) as u8;
                (ITEM_LIST[self.item].set)(&mut self.colors, i);
                self.show(view);
                if matches!(msg, DialogMsg::DoubleClicked(_)) {
                    view.set_focus(HEX);
                }
                Outcome::Keep
            }
            DialogMsg::Changed(HEX) => {
                if let Some(c) = Swatch::parse_hex(&text_of(view, HEX)) {
                    self.palette[self.swatch] = c;
                    view.set_text(SW + self.swatch as u32, &Swatch::hex(c));
                    self.show_preview(view);
                }
                Outcome::Keep
            }
            DialogMsg::SelectionChanged(SCHEME) => {
                if let Some(name) = sel(view, SCHEME).and_then(|i| self.names.get(i).cloned()) {
                    let d = cx.client.settings().display.clone();
                    self.apply_scheme(&name, &d);
                    self.select_scheme(view, &d);
                    self.show(view);
                }
                Outcome::Keep
            }
            DialogMsg::Clicked(RESET) => {
                let d = cx.client.settings().display.clone();
                let name = self.theme.clone();
                self.apply_scheme(&name, &d);
                self.show(view);
                Outcome::Keep
            }
            DialogMsg::Clicked(SAVE) => {
                self.save(cx);
                Outcome::Keep
            }
            DialogMsg::Clicked(DELETE) => {
                let d = &mut cx.client.settings_mut().display;
                let base = self.base(d);
                if d.delete_theme(&self.theme) {
                    // Les couleurs montrées restent ; le thème choisi devient la base du thème supprimé.
                    self.theme = base;
                    cx.actions.push(Action::SettingsChanged);
                    let d = d.clone();
                    self.fill_schemes(view, cx.lang, &d);
                }
                Outcome::Keep
            }
            _ => Outcome::Keep,
        }
    }

    fn refresh(&mut self, view: &mut DialogView, cx: &mut Cx) {
        let (v, name) = {
            let s = self.saved.borrow();
            (s.version, s.name.clone())
        };
        if v != self.seen {
            self.seen = v;
            if let Some(n) = name {
                self.theme = n;
            }
            let d = cx.client.settings().display.clone();
            self.fill_schemes(view, cx.lang, &d);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testkit::{Kit, assert_translated};
    use alpirc_core::settings::{DARK_PALETTE, MIRC_PALETTE};

    #[test]
    fn pick_colors_edit_palette_and_scheme() {
        let mut k = Kit::new("colors");
        let mut d = Colors::new(&k.client);
        assert_translated(&d);
        let (mut v, _) = k.open(&mut d);
        // « Texte normal » choisi : sa pastille (1, noir) est cochée.
        assert!(v.checked(SW + 1));
        // Action → rouge (4).
        v.set_selection(ITEMS, &[0]);
        k.send(&mut d, &mut v, DialogMsg::SelectionChanged(ITEMS));
        assert!(v.checked(SW + 6));
        k.send(&mut d, &mut v, DialogMsg::Clicked(SW + 4));
        assert!(v.checked(SW + 4) && !v.checked(SW + 6));
        // Le rouge devient orange vif.
        v.set_text(HEX, "#FF6600");
        k.send(&mut d, &mut v, DialogMsg::Changed(HEX));
        assert_eq!(v.text(SW + 4).as_deref(), Some("#FF6600"));
        let (o, a) = k.send(&mut d, &mut v, DialogMsg::Ok(OK));
        assert_eq!(o, Outcome::Close);
        assert!(matches!(a.first(), Some(Action::SettingsChanged)));
        let disp = &k.client.settings().display;
        assert_eq!(disp.colors.action, 4);
        assert_eq!(disp.palette[4], 0xFF6600);
        // Thème sombre : la palette de l'interface sombre, mêmes numéros.
        let mut d = Colors::new(&k.client);
        let (mut v, _) = k.open(&mut d);
        assert!(!v.enabled(DELETE), "un thème fourni ne se supprime pas");
        v.set_selection(SCHEME, &[1]);
        k.send(&mut d, &mut v, DialogMsg::SelectionChanged(SCHEME));
        k.send(&mut d, &mut v, DialogMsg::Ok(OK));
        let disp = &k.client.settings().display;
        assert_eq!(disp.theme, "dark");
        assert_eq!((disp.palette, disp.colors.background), (DARK_PALETTE, 0));
    }

    #[test]
    fn save_select_and_delete_user_themes() {
        let mut k = Kit::new("colors2");
        k.client.settings_mut().display.select_theme("dark");
        let mut d = Colors::new(&k.client);
        let (mut v, _) = k.open(&mut d);
        assert_eq!(v.items(SCHEME), vec!["Classic", "Dark"]);
        // Retoucher le fond, puis enregistrer sous « Nuit ».
        v.set_selection(ITEMS, &[21]);
        k.send(&mut d, &mut v, DialogMsg::SelectionChanged(ITEMS));
        k.send(&mut d, &mut v, DialogMsg::Clicked(SW + 2));
        let (_, mut actions) = k.send(&mut d, &mut v, DialogMsg::Clicked(SAVE));
        let Some(Action::Open(mut f)) = actions.pop() else { panic!("boîte Enregistrer attendue") };
        let (mut fv, _) = k.open(f.as_mut());
        // Un nom réservé est refusé.
        fv.set_text(11, "Sombre");
        let (out, _) = k.send(f.as_mut(), &mut fv, DialogMsg::Ok(1));
        assert_eq!(out, Outcome::Keep);
        fv.set_text(11, "Nuit");
        let (out, a) = k.send(f.as_mut(), &mut fv, DialogMsg::Ok(1));
        assert_eq!(out, Outcome::Close);
        assert!(a.iter().any(|a| matches!(a, Action::SettingsChanged)));
        k.refresh(&mut d, &mut v);
        assert_eq!(v.items(SCHEME), vec!["Classic", "Dark", "Nuit"]);
        assert_eq!(sel(&v, SCHEME), Some(2));
        assert!(v.enabled(DELETE));
        let u = k.client.settings().display.user_theme("nuit").unwrap().clone();
        assert_eq!((u.base.as_str(), u.colors.background, u.palette), ("dark", 2, DARK_PALETTE));
        // Enregistré tout de suite ; le choix ne compte qu'à OK.
        assert_eq!(k.client.settings().display.theme, "dark");
        k.send(&mut d, &mut v, DialogMsg::Ok(OK));
        assert_eq!(k.client.settings().display.theme, "Nuit");
        // Rouvrir : Classique charge ses couleurs ; revenir à Nuit les recharge.
        let mut d = Colors::new(&k.client);
        let (mut v, _) = k.open(&mut d);
        assert_eq!(sel(&v, SCHEME), Some(2));
        v.set_selection(SCHEME, &[0]);
        k.send(&mut d, &mut v, DialogMsg::SelectionChanged(SCHEME));
        assert_eq!((d.palette, d.colors.background), (MIRC_PALETTE, 0));
        assert!(!v.enabled(DELETE));
        v.set_selection(SCHEME, &[2]);
        k.send(&mut d, &mut v, DialogMsg::SelectionChanged(SCHEME));
        assert_eq!(d.colors.background, 2);
        // Supprimer : la liste perd le thème, le choix revient à sa base, les couleurs restent.
        k.send(&mut d, &mut v, DialogMsg::Clicked(DELETE));
        assert_eq!(v.items(SCHEME), vec!["Classic", "Dark"]);
        assert_eq!(sel(&v, SCHEME), Some(1));
        assert!(k.client.settings().display.user_themes.is_empty());
        assert_eq!(k.client.settings().display.theme, "dark");
        assert_eq!(d.colors.background, 2);
    }
}
