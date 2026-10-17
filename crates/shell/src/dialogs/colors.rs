//! Boîte « Couleurs » (Alt+K), à la manière de mIRC : la liste des éléments (texte des actions, des arrivées,
//! des avis…, fonds de la discussion, de la ligne de saisie et de la liste des pseudos) et les 16 couleurs de
//! la palette ; un clic sur une couleur la donne à l'élément choisi. La couleur choisie se modifie en
//! hexadécimal (`#RRGGBB`). Les jeux de couleurs fournis (classique, sombre) se choisissent en bas ; un
//! aperçu montre le résultat.

use alpirc_core::i18n::Lang;
use alpirc_core::settings::{EventColors, MIRC_PALETTE};
use alpirc_ui::dialog::Widget;
use alpirc_ui::widgets::Swatch;
use alpirc_ui::{ControlKind, ControlSpec, DialogMsg, DialogSpec, DialogView};

use super::*;

const W: i32 = 560;
const H: i32 = 392;

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

/// Jeux de couleurs proposés.
const SCHEMES: [&str; 2] = ["classic", "dark"];

/// La boîte Couleurs.
pub struct Colors {
    palette: [u32; 16],
    colors: EventColors,
    theme: String,
    item: usize,
    swatch: usize,
}

impl Colors {
    pub fn new(client: &Client) -> Colors {
        let d = &client.settings().display;
        let custom = d.palette != MIRC_PALETTE || d.colors != EventColors::default();
        // Thème sombre aux couleurs d'origine : on part de son jeu de couleurs (celui que l'interface montre).
        let (palette, colors) = match alpirc_core::settings::theme(&d.theme) {
            Some(t) if !custom && d.theme != "classic" => (t.palette, t.colors),
            _ => (d.palette, d.colors.clone()),
        };
        Colors { palette, colors, theme: d.theme.clone(), item: 10, swatch: 0 }
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

    fn apply_scheme(&mut self, name: &str) {
        if let Some(t) = alpirc_core::settings::theme(name) {
            self.palette = t.palette;
            self.colors = t.colors;
            self.theme = t.name.to_string();
        }
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
        let mut c: Vec<ControlSpec> = vec![
            text(9, (MARGIN, MARGIN, 190, LABEL_H), &tr("col.items")),
            list_box(ITEMS, (MARGIN, 26, 190, by - 26 - 8)).items(&items),
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
        let schemes: Vec<String> = SCHEMES.iter().map(|s| tr(&format!("col.scheme.{s}"))).collect();
        // Le jeu de couleurs sous la liste des éléments, OK et Annuler à droite.
        c.push(field_label(180, MARGIN, by, 50, &tr("col.scheme")));
        c.push(drop_list(SCHEME, (MARGIN + 52, by, 100, EDIT_H + 2 * 16 + 4), &schemes));
        c.push(button(RESET, (MARGIN + 158, by, 72, BTN_H), &tr("col.reset")));
        c.extend(ok_cancel(lang, W, H, OK, CANCEL, None));
        spec(&tr("col.title"), W, H, c)
    }

    fn init(&mut self, view: &mut DialogView, _cx: &mut Cx) {
        view.set_selection(ITEMS, &[self.item]);
        view.set_selection(SCHEME, &[SCHEMES.iter().position(|s| *s == self.theme).unwrap_or(0)]);
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
            DialogMsg::SelectionChanged(SCHEME) | DialogMsg::Clicked(RESET) => {
                let s = SCHEMES[sel(view, SCHEME).unwrap_or(0).min(SCHEMES.len() - 1)];
                self.apply_scheme(s);
                self.show(view);
                Outcome::Keep
            }
            _ => Outcome::Keep,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testkit::{Kit, assert_translated};

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
        // Jeu sombre.
        let mut d = Colors::new(&k.client);
        let (mut v, _) = k.open(&mut d);
        v.set_selection(SCHEME, &[1]);
        k.send(&mut d, &mut v, DialogMsg::SelectionChanged(SCHEME));
        k.send(&mut d, &mut v, DialogMsg::Ok(OK));
        let disp = &k.client.settings().display;
        assert_eq!(disp.theme, "dark");
        assert_eq!(disp.colors.background, 1);
    }
}
