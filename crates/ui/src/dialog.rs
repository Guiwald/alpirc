//! Boîtes de dialogue décrites par des données (comme les dialogues mSL) : [`DialogSpec`] et ses
//! [`ControlSpec`], puis [`DialogView`] qui héberge les widgets, gère le focus (Tab, Maj+Tab, flèches dans un
//! groupe de boutons radio), le bouton par défaut (Entrée) et l'annulation (Échap), les pages d'onglets, et
//! se dessine comme une fenêtre de Windows 98 (barre de titre, bouton de fermeture, déplaçable).
//! [`ModalStack`] empile les boîtes modales.
//!
//! Unités de dialogue (comme Windows) : une unité horizontale vaut le quart de la largeur moyenne d'un
//! caractère de la police de l'interface, une unité verticale le huitième de sa hauteur.

use serde::{Deserialize, Serialize};

use crate::canvas::{Bevel, Canvas};
use crate::geom::{Point, Rect, Size};
use crate::icons::IconId;
use crate::input::{Input, Key, MouseButton, Platform};
use crate::mdi::{CAPTION_BTN, CaptionButton, TITLE_H, paint_caption_button};
use crate::text::{Align, TextStyle, mnemonic_char};
use crate::widgets::*;
use crate::Res;

/// Unités des coordonnées d'une boîte.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DialogUnits {
    /// Pixels logiques.
    #[default]
    Px,
    /// Unités de dialogue (`option dbu` en mSL).
    Dbu,
}

/// Nature d'un contrôle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ControlKind {
    Text,
    Edit,
    Button,
    Check,
    Radio,
    Box,
    List,
    Combo,
    Icon,
    Link,
    /// Une page d'onglet (le texte est son titre) ; les contrôles dont `tab` vaut son id y appartiennent.
    Tab,
    Scroll,
    TextArea,
    ListView,
    Tree,
    Progress,
}

/// Description d'un contrôle.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ControlSpec {
    pub id: u32,
    pub kind: ControlKind,
    /// x, y, largeur, hauteur (dans les unités de la boîte), relatifs à la zone cliente.
    pub rect: (i32, i32, i32, i32),
    pub text: String,
    #[serde(default)]
    pub items: Vec<String>,
    /// Styles mSL : `multi`, `password`, `read`, `default`, `ok`, `cancel`, `flat`, `left`, `center`,
    /// `right`, `group`, `vsbar`, `hsbar`, `autohs`, `autovs`, `drop`, `sort`, `extsel`, `multsel`, `check`,
    /// `radio`, `push`, `limit N`, `disable`, `hide`, `edit`, `smooth`, `vertical`.
    #[serde(default)]
    pub styles: Vec<String>,
    /// Page d'onglet à laquelle le contrôle appartient.
    #[serde(default)]
    pub tab: Option<u32>,
}

impl ControlSpec {
    pub fn new(id: u32, kind: ControlKind, rect: (i32, i32, i32, i32), text: &str) -> ControlSpec {
        ControlSpec { id, kind, rect, text: text.to_string(), items: Vec::new(), styles: Vec::new(), tab: None }
    }

    /// Ajoute des styles (séparés par des espaces : `"default ok"`).
    pub fn style(mut self, s: &str) -> ControlSpec {
        self.styles.extend(s.split_whitespace().map(str::to_string));
        self
    }

    pub fn items<S: AsRef<str>>(mut self, items: &[S]) -> ControlSpec {
        self.items = items.iter().map(|s| s.as_ref().to_string()).collect();
        self
    }

    pub fn in_tab(mut self, tab: u32) -> ControlSpec {
        self.tab = Some(tab);
        self
    }

    fn has(&self, s: &str) -> bool {
        self.styles.iter().any(|x| x.eq_ignore_ascii_case(s) || x.to_lowercase().starts_with(&format!("{} ", s.to_lowercase())))
    }

    /// Valeur du style `limit N`.
    fn limit(&self) -> Option<usize> {
        for (i, s) in self.styles.iter().enumerate() {
            let low = s.to_lowercase();
            if let Some(rest) = low.strip_prefix("limit") {
                let rest = rest.trim();
                if !rest.is_empty() {
                    return rest.parse().ok();
                }
                return self.styles.get(i + 1).and_then(|n| n.parse().ok());
            }
        }
        None
    }
}

/// Description d'une boîte de dialogue.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DialogSpec {
    pub title: String,
    /// Taille de la zone cliente (dans les unités de la boîte).
    pub width: i32,
    pub height: i32,
    #[serde(default)]
    pub units: DialogUnits,
    pub controls: Vec<ControlSpec>,
}

/// Message d'une boîte de dialogue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogMsg {
    /// Clic sur un bouton, une case, un radio, un lien, un onglet, un en-tête de colonne.
    Clicked(u32),
    /// Texte modifié (champ, liste déroulante modifiable), barre de défilement déplacée.
    Changed(u32),
    SelectionChanged(u32),
    DoubleClicked(u32),
    /// Bouton de style `ok` (ou Entrée sur lui) : la boîte doit se fermer en validant.
    Ok(u32),
    /// Échap, bouton de fermeture, bouton de style `cancel`.
    Cancel,
}

/// Conversion des unités de dialogue en pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DluScale {
    /// Largeur moyenne d'un caractère.
    pub base_x: f32,
    /// Hauteur d'un caractère.
    pub base_y: f32,
}

impl DluScale {
    /// Unités de base de la police de l'interface.
    pub fn from_res(res: &Res) -> DluScale {
        DluScale { base_x: res.fonts.avg_char_width(TextStyle::UI), base_y: res.ui_line() as f32 }
    }

    pub fn x(&self, v: i32) -> i32 {
        (v as f32 * self.base_x / 4.0).round() as i32
    }

    pub fn y(&self, v: i32) -> i32 {
        (v as f32 * self.base_y / 8.0).round() as i32
    }

    /// Rectangle en unités de dialogue → pixels.
    pub fn rect(&self, r: (i32, i32, i32, i32)) -> Rect {
        let x0 = self.x(r.0);
        let y0 = self.y(r.1);
        Rect::new(x0, y0, self.x(r.0 + r.2) - x0, self.y(r.1 + r.3) - y0)
    }
}

/// Le widget hébergé par un contrôle.
#[derive(Clone, Debug)]
pub enum Widget {
    Text(Label),
    Edit(TextEdit),
    Button(Button),
    Check(CheckBox),
    Radio(RadioButton),
    Box(GroupBox),
    List(ListBox),
    Combo(ComboBox),
    Icon(Option<IconId>),
    Link(Link),
    /// Page d'onglet (dessinée par la bande d'onglets commune).
    TabPage,
    Scroll(ScrollBar),
    TextArea(TextArea),
    ListView(ListView),
    Tree(TreeView),
    Progress(ProgressBar),
}

/// Un contrôle de la boîte.
#[derive(Clone, Debug)]
pub struct Control {
    pub id: u32,
    pub kind: ControlKind,
    /// Rectangle relatif à la zone cliente, en pixels.
    pub rect: Rect,
    pub widget: Widget,
    pub visible: bool,
    pub enabled: bool,
    pub tab: Option<u32>,
    is_ok: bool,
    is_cancel: bool,
    is_default: bool,
    group: bool,
}

#[derive(Clone, Debug)]
struct TabStrip {
    rect: Rect,
    tabs: Tabs,
    pages: Vec<u32>,
}

/// Bordure du cadre d'une boîte.
const FRAME: i32 = 3;

/// Une boîte de dialogue ouverte.
#[derive(Clone, Debug)]
pub struct DialogView {
    pub title: String,
    /// Rectangle extérieur (cadre compris), absolu dans le canevas.
    pub rect: Rect,
    /// Barre de titre active.
    pub active: bool,
    pub controls: Vec<Control>,
    pub dlu: DluScale,
    focus: Option<usize>,
    strip: Option<TabStrip>,
    drag: Option<(Point, Rect)>,
    close_pressed: Option<bool>,
    open_combo: Option<usize>,
}

fn align_of(spec: &ControlSpec) -> Align {
    if spec.has("center") {
        Align::Center
    } else if spec.has("right") {
        Align::Right
    } else {
        Align::Left
    }
}

impl DialogView {
    /// Construit la boîte (centrée en (0, 0) : appeler [`DialogView::center_in`]).
    pub fn new(spec: &DialogSpec, res: &Res) -> DialogView {
        let dlu = DluScale::from_res(res);
        let conv = |r: (i32, i32, i32, i32)| match spec.units {
            DialogUnits::Px => Rect::new(r.0, r.1, r.2, r.3),
            DialogUnits::Dbu => dlu.rect(r),
        };
        let client = match spec.units {
            DialogUnits::Px => Size::new(spec.width, spec.height),
            DialogUnits::Dbu => Size::new(dlu.x(spec.width), dlu.y(spec.height)),
        };
        let mut controls = Vec::new();
        let mut strip: Option<TabStrip> = None;
        let mut prev_radio = false;
        for cs in &spec.controls {
            let rect = conv(cs.rect);
            let enabled = !cs.has("disable");
            let widget = match cs.kind {
                ControlKind::Text => {
                    let mut l = Label::new(&cs.text);
                    l.align = align_of(cs);
                    l.wrap = !cs.has("nowrap");
                    Widget::Text(l)
                }
                ControlKind::Edit | ControlKind::TextArea if cs.kind == ControlKind::TextArea || cs.has("multi") => {
                    let mut ta = TextArea::new(&cs.text);
                    ta.style = if cs.kind == ControlKind::TextArea { TextStyle::CHAT } else { TextStyle::UI };
                    ta.core.read_only = cs.has("read");
                    ta.core.max_chars = cs.limit();
                    Widget::TextArea(ta)
                }
                ControlKind::Edit => {
                    let mut e = TextEdit::new(&cs.text);
                    e.password = cs.has("pass") || cs.has("password");
                    e.core.read_only = cs.has("read");
                    e.core.max_chars = cs.limit();
                    Widget::Edit(e)
                }
                ControlKind::Button => {
                    let mut b = Button::new(&cs.text);
                    if cs.has("flat") {
                        b.kind = ButtonKind::Flat;
                    }
                    b.is_cancel = cs.has("cancel");
                    Widget::Button(b)
                }
                ControlKind::Check => {
                    let mut c = CheckBox::new(&cs.text, false);
                    c.push_like = cs.has("push");
                    Widget::Check(c)
                }
                ControlKind::Radio => {
                    let mut r = RadioButton::new(&cs.text, false);
                    r.push_like = cs.has("push");
                    Widget::Radio(r)
                }
                ControlKind::Box => Widget::Box(GroupBox::new(&cs.text)),
                ControlKind::List => {
                    let mode = if cs.has("extsel") {
                        SelectMode::Extended
                    } else if cs.has("multsel") {
                        SelectMode::Multi
                    } else {
                        SelectMode::Single
                    };
                    let mut l = ListBox::new(mode);
                    l.sorted = cs.has("sort");
                    l.set_items(&cs.items);
                    Widget::List(l)
                }
                ControlKind::Combo => {
                    let mut c = ComboBox::new(cs.has("edit") || !cs.has("drop"));
                    c.sorted = cs.has("sort");
                    c.set_items(&cs.items);
                    if c.editable {
                        c.edit.set_text(&cs.text);
                    }
                    Widget::Combo(c)
                }
                ControlKind::Icon => Widget::Icon(res.icons.get(&cs.text)),
                ControlKind::Link => Widget::Link(Link::new(&cs.text)),
                ControlKind::Tab => {
                    match strip.as_mut() {
                        Some(s) => {
                            s.tabs.labels.push(cs.text.clone());
                            s.pages.push(cs.id);
                        }
                        None => strip = Some(TabStrip { rect, tabs: Tabs::new(&[cs.text.as_str()]), pages: vec![cs.id] }),
                    }
                    Widget::TabPage
                }
                ControlKind::Scroll => {
                    let mut s = ScrollBar::new(if cs.has("horizontal") || rect.w > rect.h { Orientation::Horizontal } else { Orientation::Vertical });
                    s.set_range(0, 101, 1);
                    Widget::Scroll(s)
                }
                ControlKind::ListView => {
                    let cols: Vec<Column> = cs.text.split('\t').filter(|s| !s.is_empty()).map(|t| Column::new(t, 80)).collect();
                    let mut lv = ListView::new(cols);
                    if cs.has("extsel") {
                        lv.sel.mode = SelectMode::Extended;
                    }
                    lv.set_rows(cs.items.iter().map(|r| listview::ListRow::new(&r.split('\t').collect::<Vec<_>>())).collect());
                    Widget::ListView(lv)
                }
                ControlKind::Tree => {
                    let mut tv = TreeView::new();
                    tv.roots = cs.items.iter().enumerate().map(|(i, s)| TreeNode::new(i as u64, s)).collect();
                    Widget::Tree(tv)
                }
                ControlKind::Progress => {
                    Widget::Progress(ProgressBar { smooth: cs.has("smooth"), ..ProgressBar::default() })
                }
                ControlKind::TextArea => unreachable!(),
            };
            // Liste déroulante : comme en mSL, la hauteur donnée comprend la liste ouverte.
            let rect = match &widget {
                Widget::Combo(_) => {
                    let field = res.ui_line() + 6;
                    Rect::new(rect.x, rect.y, rect.w, rect.h.min(field))
                }
                _ => rect,
            };
            let widget = match widget {
                Widget::Combo(mut c) => {
                    let field = res.ui_line() + 6;
                    let full = conv(cs.rect).h;
                    if full > field {
                        c.drop_rows = ((full - field) / res.ui_line()).max(1) as usize;
                    }
                    Widget::Combo(c)
                }
                w => w,
            };
            let is_radio = cs.kind == ControlKind::Radio;
            let group = cs.has("group") || (is_radio && !prev_radio);
            prev_radio = is_radio;
            controls.push(Control {
                id: cs.id,
                kind: cs.kind,
                rect,
                widget,
                visible: !cs.has("hide"),
                enabled,
                tab: cs.tab,
                is_ok: cs.has("ok"),
                is_cancel: cs.has("cancel"),
                is_default: cs.has("default"),
                group,
            });
        }
        let outer = Size::new(client.w + 2 * FRAME, client.h + 2 * FRAME + TITLE_H + 1);
        let mut d = DialogView {
            title: spec.title.clone(),
            rect: Rect::new(0, 0, outer.w, outer.h),
            active: true,
            controls,
            dlu,
            focus: None,
            strip,
            drag: None,
            close_pressed: None,
            open_combo: None,
        };
        d.focus = d.tab_stops().first().copied();
        d.sync_focus_flags();
        if let Some(i) = d.focus {
            if let Widget::Edit(e) = &mut d.controls[i].widget {
                e.core.select_all();
            }
        }
        d
    }

    /// Centre la boîte dans `bounds`.
    pub fn center_in(&mut self, bounds: Rect) {
        self.rect = bounds.center_size(self.rect.size()).clamp_inside(&bounds);
    }

    /// Zone cliente (absolue).
    pub fn client_rect(&self) -> Rect {
        Rect::new(self.rect.x + FRAME, self.rect.y + FRAME + TITLE_H + 1, self.rect.w - 2 * FRAME, self.rect.h - 2 * FRAME - TITLE_H - 1)
    }

    fn title_rect(&self) -> Rect {
        Rect::new(self.rect.x + FRAME, self.rect.y + FRAME, self.rect.w - 2 * FRAME, TITLE_H)
    }

    fn close_rect(&self) -> Rect {
        let t = self.title_rect();
        Rect::new(t.right() - 2 - CAPTION_BTN.w, t.y + (TITLE_H - CAPTION_BTN.h) / 2, CAPTION_BTN.w, CAPTION_BTN.h)
    }

    fn index(&self, id: u32) -> Option<usize> {
        self.controls.iter().position(|c| c.id == id)
    }

    /// Rectangle absolu d'un contrôle.
    pub fn control_rect(&self, id: u32) -> Option<Rect> {
        let c = self.client_rect();
        self.index(id).map(|i| self.controls[i].rect.offset(c.x, c.y))
    }

    fn abs(&self, i: usize) -> Rect {
        let c = self.client_rect();
        self.controls[i].rect.offset(c.x, c.y)
    }

    fn selected_page(&self) -> Option<u32> {
        self.strip.as_ref().map(|s| s.pages[s.tabs.selected.min(s.pages.len() - 1)])
    }

    /// Le contrôle est-il affiché (visible, et sur la page d'onglet choisie) ?
    pub fn is_shown(&self, i: usize) -> bool {
        let c = &self.controls[i];
        c.visible && (c.tab.is_none() || c.tab == self.selected_page())
    }

    fn focusable(&self, i: usize) -> bool {
        let c = &self.controls[i];
        if !self.is_shown(i) || !c.enabled {
            return false;
        }
        match &c.widget {
            Widget::Text(_) | Widget::Box(_) | Widget::Icon(_) | Widget::Progress(_) => false,
            Widget::Edit(e) => e.enabled,
            Widget::TabPage => self.strip.as_ref().is_some_and(|s| s.pages.first() == Some(&c.id)),
            _ => true,
        }
    }

    /// Groupe de radios contenant le contrôle `i` (index des radios du groupe).
    fn radio_group(&self, i: usize) -> Vec<usize> {
        if self.controls[i].kind != ControlKind::Radio {
            return Vec::new();
        }
        let mut s = i;
        while s > 0 && !self.controls[s].group && self.controls[s - 1].kind == ControlKind::Radio {
            s -= 1;
        }
        let mut out = vec![s];
        let mut k = s + 1;
        while k < self.controls.len() && self.controls[k].kind == ControlKind::Radio && !self.controls[k].group {
            out.push(k);
            k += 1;
        }
        out
    }

    /// Arrêts de tabulation, dans l'ordre (un seul radio par groupe : le coché, ou le premier).
    pub fn tab_stops(&self) -> Vec<usize> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.controls.len() {
            if self.controls[i].kind == ControlKind::Radio {
                let g = self.radio_group(i);
                let shown: Vec<usize> = g.iter().copied().filter(|&k| self.focusable(k)).collect();
                if let Some(&k) = shown.iter().find(|&&k| matches!(&self.controls[k].widget, Widget::Radio(r) if r.checked)).or(shown.first()) {
                    out.push(k);
                }
                i = *g.last().unwrap() + 1;
                continue;
            }
            if self.focusable(i) {
                out.push(i);
            }
            i += 1;
        }
        out
    }

    /// Identifiant du contrôle qui a le focus.
    pub fn focus(&self) -> Option<u32> {
        self.focus.map(|i| self.controls[i].id)
    }

    /// Donne le focus à un contrôle.
    pub fn set_focus(&mut self, id: u32) {
        if let Some(i) = self.index(id) {
            if self.focusable(i) {
                self.focus = Some(i);
                self.sync_focus_flags();
            }
        }
    }

    fn sync_focus_flags(&mut self) {
        let f = self.focus;
        let page_focus = f.is_some_and(|i| self.controls[i].kind == ControlKind::Tab);
        for (i, c) in self.controls.iter_mut().enumerate() {
            let on = f == Some(i);
            match &mut c.widget {
                Widget::Edit(e) => e.focused = on,
                Widget::Button(b) => b.focused = on,
                Widget::Check(x) => x.focused = on,
                Widget::Radio(x) => x.focused = on,
                Widget::List(x) => x.focused = on,
                Widget::Combo(x) => x.focused = on,
                Widget::Link(x) => x.focused = on,
                Widget::TextArea(x) => x.focused = on,
                Widget::ListView(x) => x.focused = on,
                Widget::Tree(x) => x.focused = on,
                _ => {}
            }
        }
        if let Some(s) = self.strip.as_mut() {
            s.tabs.focused = page_focus;
        }
    }

    fn move_focus(&mut self, to: Option<usize>, res: &Res, platform: &mut dyn Platform) {
        if to == self.focus {
            return;
        }
        if let Some(old) = self.focus {
            let r = self.abs(old);
            self.send(old, &Input::FocusOut, r, res, platform);
        }
        self.focus = to;
        self.sync_focus_flags();
        if let Some(new) = to {
            let r = self.abs(new);
            self.send(new, &Input::FocusIn, r, res, platform);
            // Tabuler vers un champ sélectionne tout son texte, comme Windows.
            if let Widget::Edit(e) = &mut self.controls[new].widget {
                e.core.select_all();
            }
        }
        platform.request_repaint();
    }

    /// Bouton par défaut : style `default`, sinon le bouton `ok`.
    fn default_button(&self) -> Option<usize> {
        self.controls
            .iter()
            .position(|c| c.is_default && c.kind == ControlKind::Button)
            .or_else(|| self.controls.iter().position(|c| c.is_ok && c.kind == ControlKind::Button))
    }

    fn click_msg(&self, i: usize) -> DialogMsg {
        let c = &self.controls[i];
        if c.is_ok {
            DialogMsg::Ok(c.id)
        } else if c.is_cancel {
            DialogMsg::Cancel
        } else {
            DialogMsg::Clicked(c.id)
        }
    }

    // --- Accès par identifiant ---

    /// Texte d'un contrôle (étiquette, champ, bouton, élément choisi d'une liste…).
    pub fn text(&self, id: u32) -> Option<String> {
        let c = &self.controls[self.index(id)?];
        Some(match &c.widget {
            Widget::Text(l) => l.text.clone(),
            Widget::Edit(e) => e.text().to_string(),
            Widget::Button(b) => b.text.clone(),
            Widget::Check(x) => x.text.clone(),
            Widget::Radio(x) => x.text.clone(),
            Widget::Box(x) => x.text.clone(),
            Widget::List(l) => l.selected().map(|i| l.items[i].text.clone()).unwrap_or_default(),
            Widget::Combo(x) => x.text(),
            Widget::Link(x) => x.text.clone(),
            Widget::TextArea(x) => x.text().to_string(),
            Widget::TabPage => {
                let s = self.strip.as_ref()?;
                let k = s.pages.iter().position(|&p| p == id)?;
                s.tabs.labels[k].clone()
            }
            _ => String::new(),
        })
    }

    /// Change le texte d'un contrôle.
    pub fn set_text(&mut self, id: u32, text: &str) {
        let Some(i) = self.index(id) else { return };
        match &mut self.controls[i].widget {
            Widget::Text(l) => l.text = text.to_string(),
            Widget::Edit(e) => e.set_text(text),
            Widget::Button(b) => b.text = text.to_string(),
            Widget::Check(x) => x.text = text.to_string(),
            Widget::Radio(x) => x.text = text.to_string(),
            Widget::Box(x) => x.text = text.to_string(),
            Widget::Combo(x) => x.edit.set_text(text),
            Widget::Link(x) => x.text = text.to_string(),
            Widget::TextArea(x) => x.set_text(text),
            Widget::TabPage => {
                if let Some(s) = self.strip.as_mut() {
                    if let Some(k) = s.pages.iter().position(|&p| p == id) {
                        s.tabs.labels[k] = text.to_string();
                    }
                }
            }
            _ => {}
        }
    }

    /// Éléments d'une liste, liste déroulante, liste à colonnes (cellules séparées par des tabulations).
    pub fn items(&self, id: u32) -> Vec<String> {
        let Some(i) = self.index(id) else { return Vec::new() };
        match &self.controls[i].widget {
            Widget::List(l) => l.items.iter().map(|x| x.text.clone()).collect(),
            Widget::Combo(c) => c.items.clone(),
            Widget::ListView(lv) => lv.rows.iter().map(|r| r.cells.join("\t")).collect(),
            _ => Vec::new(),
        }
    }

    /// Remplace les éléments.
    pub fn set_items<S: AsRef<str>>(&mut self, id: u32, items: &[S]) {
        let Some(i) = self.index(id) else { return };
        match &mut self.controls[i].widget {
            Widget::List(l) => l.set_items(items),
            Widget::Combo(c) => c.set_items(items),
            Widget::ListView(lv) => lv.set_rows(items.iter().map(|r| listview::ListRow::new(&r.as_ref().split('\t').collect::<Vec<_>>())).collect()),
            Widget::Tree(t) => t.roots = items.iter().enumerate().map(|(k, s)| TreeNode::new(k as u64, s.as_ref())).collect(),
            _ => {}
        }
    }

    /// Ajoute un élément à une liste ou une liste déroulante.
    pub fn add_item(&mut self, id: u32, item: &str) {
        let Some(i) = self.index(id) else { return };
        match &mut self.controls[i].widget {
            Widget::List(l) => {
                l.add(ListItem::new(item));
            }
            Widget::Combo(c) => {
                c.items.push(item.to_string());
                if c.sorted {
                    c.items.sort_by_key(|a| a.to_lowercase());
                }
            }
            Widget::ListView(lv) => {
                lv.rows.push(listview::ListRow::new(&item.split('\t').collect::<Vec<_>>()));
                lv.sel.resize(lv.rows.len());
            }
            _ => {}
        }
    }

    /// Éléments choisis (index) ; pour une page d'onglets : l'onglet choisi.
    pub fn selection(&self, id: u32) -> Vec<usize> {
        let Some(i) = self.index(id) else { return Vec::new() };
        match &self.controls[i].widget {
            Widget::List(l) => l.sel.selected(),
            Widget::Combo(c) => c.selected.into_iter().collect(),
            Widget::ListView(lv) => lv.sel.selected(),
            Widget::Tree(t) => t.selected.map(|s| s as usize).into_iter().collect(),
            Widget::TabPage => self.strip.as_ref().map(|s| vec![s.tabs.selected]).unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    /// Choisit des éléments ; pour une page d'onglet, l'affiche.
    pub fn set_selection(&mut self, id: u32, sel: &[usize]) {
        let Some(i) = self.index(id) else { return };
        match &mut self.controls[i].widget {
            Widget::List(l) => {
                l.sel.clear();
                for &k in sel {
                    l.sel.set(k, true);
                }
                if let Some(&k) = sel.first() {
                    l.sel.cursor = k.min(l.items.len().saturating_sub(1));
                }
            }
            Widget::Combo(c) => c.select(sel.first().copied()),
            Widget::ListView(lv) => {
                lv.sel.clear();
                for &k in sel {
                    lv.sel.set(k, true);
                }
            }
            Widget::Tree(t) => t.selected = sel.first().map(|&k| k as u64),
            Widget::TabPage => {
                if let Some(s) = self.strip.as_mut() {
                    if let Some(k) = s.pages.iter().position(|&p| p == id) {
                        s.tabs.selected = k;
                    }
                }
            }
            _ => {}
        }
    }

    pub fn checked(&self, id: u32) -> bool {
        self.index(id).is_some_and(|i| match &self.controls[i].widget {
            Widget::Check(c) => c.checked,
            Widget::Radio(r) => r.checked,
            Widget::Button(b) => b.toggled,
            _ => false,
        })
    }

    /// Coche ou décoche (pour un radio : décoche les autres du groupe).
    pub fn set_checked(&mut self, id: u32, on: bool) {
        let Some(i) = self.index(id) else { return };
        if on && self.controls[i].kind == ControlKind::Radio {
            for k in self.radio_group(i) {
                if let Widget::Radio(r) = &mut self.controls[k].widget {
                    r.checked = k == i;
                }
            }
            return;
        }
        match &mut self.controls[i].widget {
            Widget::Check(c) => c.checked = on,
            Widget::Radio(r) => r.checked = on,
            Widget::Button(b) => b.toggled = on,
            _ => {}
        }
    }

    pub fn enabled(&self, id: u32) -> bool {
        self.index(id).is_some_and(|i| self.controls[i].enabled)
    }

    pub fn set_enabled(&mut self, id: u32, on: bool) {
        if let Some(i) = self.index(id) {
            self.controls[i].enabled = on;
            if !on && self.focus == Some(i) {
                self.focus = self.tab_stops().first().copied();
                self.sync_focus_flags();
            }
        }
    }

    pub fn visible(&self, id: u32) -> bool {
        self.index(id).is_some_and(|i| self.controls[i].visible)
    }

    pub fn set_visible(&mut self, id: u32, on: bool) {
        if let Some(i) = self.index(id) {
            self.controls[i].visible = on;
            if !on && self.focus == Some(i) {
                self.focus = self.tab_stops().first().copied();
                self.sync_focus_flags();
            }
        }
    }

    /// Valeur d'une barre de progression ou de défilement.
    pub fn value(&self, id: u32) -> i32 {
        self.index(id).map_or(0, |i| match &self.controls[i].widget {
            Widget::Progress(p) => p.value,
            Widget::Scroll(s) => s.pos,
            _ => 0,
        })
    }

    pub fn set_value(&mut self, id: u32, v: i32) {
        if let Some(i) = self.index(id) {
            match &mut self.controls[i].widget {
                Widget::Progress(p) => p.value = v,
                Widget::Scroll(s) => s.pos = s.clamp(v),
                _ => {}
            }
        }
    }

    /// Accès direct au widget (pour ce que les accesseurs ne couvrent pas).
    pub fn widget_mut(&mut self, id: u32) -> Option<&mut Widget> {
        let i = self.index(id)?;
        Some(&mut self.controls[i].widget)
    }

    // --- Dessin ---

    pub fn paint(&self, canvas: &mut Canvas, res: &Res) {
        let t = &res.theme;
        canvas.fill(self.rect, t.face);
        canvas.bevel(self.rect, Bevel::Window, t);
        let tr = self.title_rect();
        let (from, to, fg) = if self.active {
            (t.title_active_from, t.title_active_to, t.title_active_text)
        } else {
            (t.title_inactive_from, t.title_inactive_to, t.title_inactive_text)
        };
        canvas.gradient_h(tr, from, to);
        let cr = self.close_rect();
        let title = res.fonts.ellipsize(&self.title, TextStyle::UI_BOLD, cr.x - tr.x - 6);
        res.fonts.draw_text(canvas, tr.x + 3, tr.y + (TITLE_H - res.ui_line()) / 2, &title, TextStyle::UI_BOLD, fg);
        paint_caption_button(canvas, res, cr, CaptionButton::Close, self.close_pressed == Some(true));
        let client = self.client_rect();
        let default_btn = self.default_button();
        let focus_on_button = self.focus.is_some_and(|i| self.controls[i].kind == ControlKind::Button);
        canvas.with_clip(client, |canvas| {
            if let Some(s) = &self.strip {
                let mut tabs = s.tabs.clone();
                tabs.enabled = true;
                tabs.paint(canvas, res, s.rect.offset(client.x, client.y));
            }
            for (i, c) in self.controls.iter().enumerate() {
                if !self.is_shown(i) {
                    continue;
                }
                let r = c.rect.offset(client.x, client.y);
                match &c.widget {
                    Widget::Text(w) => {
                        let mut w = w.clone();
                        w.enabled = c.enabled;
                        w.paint(canvas, res, r)
                    }
                    Widget::Edit(w) => {
                        let mut w = w.clone();
                        w.enabled = c.enabled;
                        w.paint(canvas, res, r)
                    }
                    Widget::Button(w) => {
                        let mut w = w.clone();
                        w.enabled = c.enabled;
                        w.is_default = default_btn == Some(i) && !focus_on_button;
                        w.paint(canvas, res, r)
                    }
                    Widget::Check(w) => {
                        let mut w = w.clone();
                        w.enabled = c.enabled;
                        w.paint(canvas, res, r)
                    }
                    Widget::Radio(w) => {
                        let mut w = w.clone();
                        w.enabled = c.enabled;
                        w.paint(canvas, res, r)
                    }
                    Widget::Box(w) => {
                        let mut w = w.clone();
                        w.enabled = c.enabled;
                        w.paint(canvas, res, r)
                    }
                    Widget::List(w) => {
                        let mut w = w.clone();
                        w.enabled = c.enabled;
                        w.paint(canvas, res, r)
                    }
                    Widget::Combo(w) => {
                        let mut w = w.clone();
                        w.enabled = c.enabled;
                        w.paint(canvas, res, r)
                    }
                    Widget::Icon(id) => {
                        if let Some(img) = id.and_then(|i| res.icons.image(i)) {
                            canvas.blit(img, r.x, r.y);
                        }
                    }
                    Widget::Link(w) => {
                        let mut w = w.clone();
                        w.enabled = c.enabled;
                        w.paint(canvas, res, r)
                    }
                    Widget::TabPage => {}
                    Widget::Scroll(w) => w.paint(canvas, res, r),
                    Widget::TextArea(w) => {
                        let mut w = w.clone();
                        w.enabled = c.enabled;
                        w.paint(canvas, res, r)
                    }
                    Widget::ListView(w) => {
                        let mut w = w.clone();
                        w.enabled = c.enabled;
                        w.paint(canvas, res, r)
                    }
                    Widget::Tree(w) => {
                        let mut w = w.clone();
                        w.enabled = c.enabled;
                        w.paint(canvas, res, r)
                    }
                    Widget::Progress(w) => w.paint(canvas, res, r),
                }
            }
        });
        if let Some(i) = self.open_combo {
            if let Widget::Combo(c) = &self.controls[i].widget {
                c.paint_popup(canvas, res);
            }
        }
    }

    // --- Événements ---

    /// Envoie un événement au widget `i` ; renvoie le message de la boîte qui en résulte.
    fn send(&mut self, i: usize, input: &Input, r: Rect, res: &Res, platform: &mut dyn Platform) -> Option<DialogMsg> {
        if !self.controls[i].enabled && !matches!(input, Input::FocusOut) {
            return None;
        }
        let id = self.controls[i].id;
        let bounds = self.rect;
        match &mut self.controls[i].widget {
            Widget::Edit(w) => match w.event(input, r, res, platform)? {
                TextEditMsg::Changed => Some(DialogMsg::Changed(id)),
                TextEditMsg::Submit => None,
            },
            Widget::Button(w) => {
                w.event(input, r, res, platform)?;
                Some(self.click_msg(i))
            }
            Widget::Check(w) => {
                w.event(input, r, res, platform)?;
                Some(DialogMsg::Clicked(id))
            }
            Widget::Radio(w) => {
                w.event(input, r, res, platform)?;
                self.set_checked(id, true);
                Some(DialogMsg::Clicked(id))
            }
            Widget::List(w) => match w.event(input, r, res, platform)? {
                ListMsg::SelectionChanged => Some(DialogMsg::SelectionChanged(id)),
                ListMsg::DoubleClicked(_) => Some(DialogMsg::DoubleClicked(id)),
                ListMsg::RightClicked(..) => None,
            },
            Widget::Combo(w) => {
                let m = w.event(input, r, bounds, res, platform);
                let open = w.is_open();
                self.open_combo = if open { Some(i) } else { None };
                match m? {
                    ComboMsg::SelectionChanged(_) => Some(DialogMsg::SelectionChanged(id)),
                    ComboMsg::EditChanged => Some(DialogMsg::Changed(id)),
                    ComboMsg::Submit => self.press_default(res, platform),
                    ComboMsg::Dropped(_) => None,
                }
            }
            Widget::Link(w) => {
                w.event(input, r, res, platform)?;
                Some(DialogMsg::Clicked(id))
            }
            Widget::Scroll(w) => {
                w.event(input, r, res, platform)?;
                Some(DialogMsg::Changed(id))
            }
            Widget::TextArea(w) => {
                w.event(input, r, res, platform)?;
                Some(DialogMsg::Changed(id))
            }
            Widget::ListView(w) => match w.event(input, r, res, platform)? {
                ListViewMsg::SelectionChanged => Some(DialogMsg::SelectionChanged(id)),
                ListViewMsg::DoubleClicked(_) => Some(DialogMsg::DoubleClicked(id)),
                ListViewMsg::HeaderClicked(_) => Some(DialogMsg::Clicked(id)),
                _ => None,
            },
            Widget::Tree(w) => match w.event(input, r, res, platform)? {
                TreeViewMsg::Selected(_) => Some(DialogMsg::SelectionChanged(id)),
                TreeViewMsg::Activated(_) => Some(DialogMsg::DoubleClicked(id)),
                _ => None,
            },
            _ => None,
        }
    }

    fn press_default(&mut self, _res: &Res, platform: &mut dyn Platform) -> Option<DialogMsg> {
        let i = self.default_button()?;
        if !self.controls[i].enabled || !self.is_shown(i) {
            return None;
        }
        platform.request_repaint();
        Some(self.click_msg(i))
    }

    fn tab_event(&mut self, input: &Input, res: &Res, platform: &mut dyn Platform) -> Option<DialogMsg> {
        let client = self.client_rect();
        let s = self.strip.as_mut()?;
        let r = s.rect.offset(client.x, client.y);
        let TabsMsg::Selected(k) = s.tabs.event(input, r, res, platform)?;
        let page = s.pages[k];
        // Le focus quitte un contrôle devenu invisible.
        if self.focus.is_some_and(|f| !self.is_shown(f)) {
            self.focus = self.index(self.strip.as_ref().unwrap().pages[0]);
            self.sync_focus_flags();
        }
        Some(DialogMsg::Clicked(page))
    }

    fn mnemonic_target(&self, c: char) -> Option<usize> {
        let c = c.to_lowercase().next()?;
        let label_of = |w: &Widget| -> Option<String> {
            Some(match w {
                Widget::Text(l) => l.text.clone(),
                Widget::Box(b) => b.text.clone(),
                Widget::Button(b) => b.text.clone(),
                Widget::Check(x) => x.text.clone(),
                Widget::Radio(x) => x.text.clone(),
                _ => return None,
            })
        };
        (0..self.controls.len()).find(|&i| self.is_shown(i) && self.controls[i].enabled && label_of(&self.controls[i].widget).is_some_and(|l| mnemonic_char(&l) == Some(c)))
    }

    fn accepts_chars(&self) -> bool {
        self.focus.is_some_and(|i| {
            matches!(&self.controls[i].widget, Widget::Edit(_) | Widget::TextArea(_) | Widget::List(_) | Widget::ListView(_) | Widget::Tree(_))
                || matches!(&self.controls[i].widget, Widget::Combo(c) if c.editable)
        })
    }

    /// Traite un événement ; `bounds` : zone où la boîte peut être déplacée (le canevas).
    pub fn event(&mut self, input: &Input, res: &Res, platform: &mut dyn Platform, bounds: Rect) -> Option<DialogMsg> {
        // Liste déroulante ouverte : elle passe d'abord.
        if let Some(i) = self.open_combo {
            let r = self.abs(i);
            let m = self.send(i, input, r, res, platform);
            if matches!(input, Input::Tick) {
                // Le battement va aussi aux autres.
            } else {
                return m;
            }
        }
        match input {
            Input::MouseDown { pos, button: MouseButton::Left, .. } if self.close_rect().contains(*pos) => {
                self.close_pressed = Some(true);
                platform.request_repaint();
                return None;
            }
            Input::MouseMove { pos } if self.close_pressed.is_some() => {
                let over = self.close_rect().contains(*pos);
                if self.close_pressed != Some(over) {
                    self.close_pressed = Some(over);
                    platform.request_repaint();
                }
                return None;
            }
            Input::MouseUp { pos, .. } if self.close_pressed.is_some() => {
                self.close_pressed = None;
                platform.request_repaint();
                return self.close_rect().contains(*pos).then_some(DialogMsg::Cancel);
            }
            Input::MouseDown { pos, button: MouseButton::Left, .. } if self.title_rect().contains(*pos) => {
                self.drag = Some((*pos, self.rect));
                return None;
            }
            Input::MouseMove { pos } if self.drag.is_some() => {
                let (start, r0) = self.drag.unwrap();
                let mut r = r0.offset(pos.x - start.x, pos.y - start.y);
                r.y = r.y.clamp(bounds.y, (bounds.bottom() - TITLE_H - FRAME).max(bounds.y));
                r.x = r.x.clamp(bounds.x - r.w + 40, bounds.right() - 40);
                if r != self.rect {
                    self.rect = r;
                    platform.request_repaint();
                }
                return None;
            }
            Input::MouseUp { .. } if self.drag.is_some() => {
                self.drag = None;
                return None;
            }
            _ => {}
        }
        match input {
            Input::MouseDown { pos, .. } => {
                // Bande d'onglets.
                let client = self.client_rect();
                if let Some(s) = &self.strip {
                    let sr = s.rect.offset(client.x, client.y);
                    let tabs_h = s.tabs.strip_height(res);
                    if Rect::new(sr.x, sr.y, sr.w, tabs_h).contains(*pos) {
                        let first = self.index(s.pages[0]);
                        self.move_focus(first, res, platform);
                        return self.tab_event(input, res, platform);
                    }
                }
                let hit = (0..self.controls.len()).rev().find(|&i| {
                    self.is_shown(i) && self.abs(i).contains(*pos) && !matches!(self.controls[i].widget, Widget::Box(_) | Widget::TabPage)
                })?;
                if self.focusable(hit) {
                    self.move_focus(Some(hit), res, platform);
                }
                let r = self.abs(hit);
                self.send(hit, input, r, res, platform)
            }
            Input::MouseMove { .. } | Input::MouseUp { .. } | Input::MouseLeave | Input::Wheel { .. } => {
                let mut msg = None;
                for i in 0..self.controls.len() {
                    if !self.is_shown(i) {
                        continue;
                    }
                    let r = self.abs(i);
                    if let Input::Wheel { pos, .. } = input {
                        if !r.contains(*pos) {
                            continue;
                        }
                    }
                    if let Some(m) = self.send(i, input, r, res, platform) {
                        msg = Some(m);
                    }
                }
                msg
            }
            Input::Tick => {
                for i in 0..self.controls.len() {
                    if self.is_shown(i) {
                        let r = self.abs(i);
                        self.send(i, input, r, res, platform);
                    }
                }
                None
            }
            Input::Key { key, mods, text, .. } => {
                let focus_kind = self.focus.map(|i| self.controls[i].kind);
                let in_textarea = matches!(self.focus.map(|i| &self.controls[i].widget), Some(Widget::TextArea(_)));
                match key {
                    Key::Tab if mods.ctrl && self.strip.is_some() => return self.tab_event(input, res, platform),
                    Key::Tab if !mods.command() => {
                        let stops = self.tab_stops();
                        if stops.is_empty() {
                            return None;
                        }
                        let cur = self.focus.and_then(|f| stops.iter().position(|&s| s == f));
                        let next = match (cur, mods.shift) {
                            (None, false) => 0,
                            (None, true) => stops.len() - 1,
                            (Some(c), false) => (c + 1) % stops.len(),
                            (Some(c), true) => (c + stops.len() - 1) % stops.len(),
                        };
                        self.move_focus(Some(stops[next]), res, platform);
                        return None;
                    }
                    Key::Escape => {
                        return Some(DialogMsg::Cancel);
                    }
                    Key::Enter if !in_textarea && !mods.command() => {
                        if focus_kind == Some(ControlKind::Button) {
                            let i = self.focus.unwrap();
                            return Some(self.click_msg(i));
                        }
                        if let Some(i) = self.focus {
                            if let Widget::List(_) | Widget::ListView(_) | Widget::Tree(_) = self.controls[i].widget {
                                if self.default_button().is_none() {
                                    let r = self.abs(i);
                                    return self.send(i, input, r, res, platform);
                                }
                            }
                        }
                        return self.press_default(res, platform);
                    }
                    Key::Up | Key::Down | Key::Left | Key::Right if focus_kind == Some(ControlKind::Radio) => {
                        let f = self.focus.unwrap();
                        let g: Vec<usize> = self.radio_group(f).into_iter().filter(|&k| self.focusable(k)).collect();
                        let pos = g.iter().position(|&k| k == f)?;
                        let fwd = matches!(key, Key::Down | Key::Right);
                        let next = g[if fwd { (pos + 1) % g.len() } else { (pos + g.len() - 1) % g.len() }];
                        self.move_focus(Some(next), res, platform);
                        let id = self.controls[next].id;
                        self.set_checked(id, true);
                        return Some(DialogMsg::Clicked(id));
                    }
                    Key::Char(c) if (mods.alt && !mods.command()) || (mods.is_empty() && !self.accepts_chars() && text.is_some()) => {
                        if let Some(i) = self.mnemonic_target(*c) {
                            let r = self.abs(i);
                            return match self.controls[i].kind {
                                ControlKind::Text | ControlKind::Box => {
                                    let next = self.tab_stops().into_iter().find(|&k| k > i);
                                    self.move_focus(next, res, platform);
                                    None
                                }
                                ControlKind::Button => {
                                    self.move_focus(Some(i), res, platform);
                                    Some(self.click_msg(i))
                                }
                                ControlKind::Check | ControlKind::Radio => {
                                    self.move_focus(Some(i), res, platform);
                                    self.send(i, &Input::key(Key::Char(' '), Default::default()), r, res, platform)
                                }
                                _ => None,
                            };
                        }
                        if mods.alt {
                            return None;
                        }
                    }
                    _ => {}
                }
                if focus_kind == Some(ControlKind::Tab) {
                    return self.tab_event(input, res, platform);
                }
                let i = self.focus?;
                let r = self.abs(i);
                self.send(i, input, r, res, platform)
            }
            Input::Text(_) | Input::ImePreedit { .. } => {
                let i = self.focus?;
                let r = self.abs(i);
                self.send(i, input, r, res, platform)
            }
            Input::FocusIn | Input::FocusOut => {
                self.active = matches!(input, Input::FocusIn);
                platform.request_repaint();
                None
            }
        }
    }
}

/// Pile de boîtes modales : seule celle du dessus reçoit les événements.
#[derive(Clone, Debug, Default)]
pub struct ModalStack {
    /// Boîtes, avec une clé choisie par l'application.
    pub dialogs: Vec<(u64, DialogView)>,
}

impl ModalStack {
    pub fn new() -> ModalStack {
        ModalStack::default()
    }

    /// Ouvre une boîte centrée dans `bounds`.
    pub fn push(&mut self, key: u64, mut dialog: DialogView, bounds: Rect) {
        dialog.center_in(bounds);
        for (_, d) in &mut self.dialogs {
            d.active = false;
        }
        dialog.active = true;
        self.dialogs.push((key, dialog));
    }

    /// Ferme la boîte du dessus.
    pub fn pop(&mut self) -> Option<(u64, DialogView)> {
        let d = self.dialogs.pop();
        if let Some((_, top)) = self.dialogs.last_mut() {
            top.active = true;
        }
        d
    }

    /// Ferme la boîte de clé `key`.
    pub fn close(&mut self, key: u64) -> Option<DialogView> {
        let i = self.dialogs.iter().position(|(k, _)| *k == key)?;
        let (_, d) = self.dialogs.remove(i);
        if let Some((_, top)) = self.dialogs.last_mut() {
            top.active = true;
        }
        Some(d)
    }

    pub fn top_mut(&mut self) -> Option<&mut DialogView> {
        self.dialogs.last_mut().map(|(_, d)| d)
    }

    pub fn get_mut(&mut self, key: u64) -> Option<&mut DialogView> {
        self.dialogs.iter_mut().find(|(k, _)| *k == key).map(|(_, d)| d)
    }

    pub fn is_empty(&self) -> bool {
        self.dialogs.is_empty()
    }

    /// Dessine toutes les boîtes, de la plus ancienne à celle du dessus.
    pub fn paint(&self, canvas: &mut Canvas, res: &Res) {
        for (_, d) in &self.dialogs {
            d.paint(canvas, res);
        }
    }

    /// Passe l'événement à la boîte du dessus ; renvoie sa clé et son message.
    pub fn event(&mut self, input: &Input, res: &Res, platform: &mut dyn Platform, bounds: Rect) -> Option<(u64, DialogMsg)> {
        let (k, d) = self.dialogs.last_mut()?;
        let key = *k;
        d.event(input, res, platform, bounds).map(|m| (key, m))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Mods, TestPlatform};

    fn spec() -> DialogSpec {
        use ControlKind::*;
        DialogSpec {
            title: "Connexion".into(),
            width: 200,
            height: 120,
            units: DialogUnits::Dbu,
            controls: vec![
                ControlSpec::new(1, Text, (5, 5, 40, 8), "&Pseudo :"),
                ControlSpec::new(2, Edit, (50, 4, 100, 12), "Guiwald").style("limit 9"),
                ControlSpec::new(3, Radio, (5, 20, 60, 10), "&Undernet").style("group"),
                ControlSpec::new(4, Radio, (5, 30, 60, 10), "&EFnet"),
                ControlSpec::new(5, Radio, (5, 40, 60, 10), "&IRCnet"),
                ControlSpec::new(6, Check, (5, 55, 90, 10), "In&visible"),
                ControlSpec::new(7, List, (100, 20, 90, 50), "").items(&["#lyon", "#grenoble"]).style("extsel"),
                ControlSpec::new(8, Button, (90, 100, 50, 14), "&OK").style("default ok"),
                ControlSpec::new(9, Button, (145, 100, 50, 14), "Annuler").style("cancel"),
            ],
        }
    }

    #[test]
    fn dlu_conversion() {
        let res = crate::test_res();
        let s = DluScale::from_res(&res);
        assert!(s.base_x > 5.0 && s.base_x < 9.0, "{}", s.base_x);
        assert_eq!(s.base_y, 16.0);
        assert_eq!(s.y(8), 16);
        assert_eq!(s.x(4), s.base_x.round() as i32);
        let d = DialogView::new(&spec(), &res);
        assert_eq!(d.client_rect().h, s.y(120));
        let r = d.controls[1].rect;
        assert_eq!((r.x, r.y, r.h), (s.x(50), s.y(4), s.y(16) - s.y(4)));
    }

    #[test]
    fn focus_traversal_and_radio_group() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let mut d = DialogView::new(&spec(), &res);
        let b = Rect::new(0, 0, 800, 600);
        d.center_in(b);
        assert_eq!(d.focus(), Some(2), "premier arrêt : le champ");
        // Tab : un seul arrêt pour le groupe de radios (aucun coché : le premier).
        let tab = Input::key(Key::Tab, Mods::NONE);
        d.event(&tab, &res, &mut p, b);
        assert_eq!(d.focus(), Some(3));
        // Flèches dans le groupe : choisissent.
        assert_eq!(d.event(&Input::key(Key::Down, Mods::NONE), &res, &mut p, b), Some(DialogMsg::Clicked(4)));
        assert!(d.checked(4) && !d.checked(3));
        assert_eq!(d.event(&Input::key(Key::Up, Mods::NONE), &res, &mut p, b), Some(DialogMsg::Clicked(3)));
        assert_eq!(d.event(&Input::key(Key::Up, Mods::NONE), &res, &mut p, b), Some(DialogMsg::Clicked(5)), "boucle");
        d.event(&tab, &res, &mut p, b);
        assert_eq!(d.focus(), Some(6));
        d.event(&tab, &res, &mut p, b);
        d.event(&tab, &res, &mut p, b);
        d.event(&tab, &res, &mut p, b);
        d.event(&tab, &res, &mut p, b);
        assert_eq!(d.focus(), Some(2), "on fait le tour");
        // Maj+Tab : retour au bouton Annuler ; le radio coché est l'arrêt du groupe.
        d.event(&Input::key(Key::Tab, Mods::SHIFT), &res, &mut p, b);
        assert_eq!(d.focus(), Some(9));
        assert_eq!(d.tab_stops().iter().map(|&i| d.controls[i].id).collect::<Vec<_>>(), vec![2, 5, 6, 7, 8, 9]);
        // Entrée sur Annuler : Cancel ; Entrée ailleurs : le bouton par défaut.
        assert_eq!(d.event(&Input::key(Key::Enter, Mods::NONE), &res, &mut p, b), Some(DialogMsg::Cancel));
        d.set_focus(2);
        assert_eq!(d.event(&Input::key(Key::Enter, Mods::NONE), &res, &mut p, b), Some(DialogMsg::Ok(8)));
        assert_eq!(d.event(&Input::key(Key::Escape, Mods::NONE), &res, &mut p, b), Some(DialogMsg::Cancel));
        // Alt+V : la case.
        let alt_i = Input::Key { key: Key::Char('v'), mods: Mods::ALT, text: None, repeat: false };
        assert_eq!(d.event(&alt_i, &res, &mut p, b), Some(DialogMsg::Clicked(6)));
        assert!(d.checked(6));
        // Alt+P (étiquette) : focus au champ suivant.
        d.set_focus(9);
        d.event(&Input::Key { key: Key::Char('p'), mods: Mods::ALT, text: None, repeat: false }, &res, &mut p, b);
        assert_eq!(d.focus(), Some(2));
    }

    #[test]
    fn getters_setters_and_typing() {
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let mut d = DialogView::new(&spec(), &res);
        let b = Rect::new(0, 0, 800, 600);
        d.center_in(b);
        // Le champ a la limite de 9 caractères ; le focus y sélectionne tout.
        d.event(&Input::key(Key::Char('x'), Mods::NONE), &res, &mut p, b);
        assert_eq!(d.text(2).as_deref(), Some("x"));
        for c in "abcdefghijk".chars() {
            d.event(&Input::key(Key::Char(c), Mods::NONE), &res, &mut p, b);
        }
        assert_eq!(d.text(2).unwrap().chars().count(), 9);
        d.set_text(2, "Elisa");
        assert_eq!(d.text(2).as_deref(), Some("Elisa"));
        d.add_item(7, "#chambery");
        assert_eq!(d.items(7).len(), 3);
        d.set_selection(7, &[0, 2]);
        assert_eq!(d.selection(7), vec![0, 2]);
        d.set_enabled(8, false);
        assert_eq!(d.event(&Input::key(Key::Enter, Mods::NONE), &res, &mut p, b), None);
        d.set_visible(6, false);
        assert!(!d.tab_stops().iter().any(|&i| d.controls[i].id == 6));
        // Clic sur le bouton Annuler.
        let r = d.control_rect(9).unwrap();
        d.event(&Input::click(r.x + 5, r.y + 5), &res, &mut p, b);
        assert_eq!(d.event(&Input::release(r.x + 5, r.y + 5), &res, &mut p, b), Some(DialogMsg::Cancel));
        // Déplacer la boîte par sa barre de titre.
        let r0 = d.rect;
        d.event(&Input::click(r0.x + 20, r0.y + 8), &res, &mut p, b);
        d.event(&Input::moved(r0.x + 50, r0.y + 18), &res, &mut p, b);
        d.event(&Input::release(r0.x + 50, r0.y + 18), &res, &mut p, b);
        assert_eq!(d.rect, r0.offset(30, 10));
    }

    #[test]
    fn tab_pages_and_modal_stack() {
        use ControlKind::*;
        let res = crate::test_res();
        let mut p = TestPlatform::new();
        let spec = DialogSpec {
            title: "Options".into(),
            width: 300,
            height: 200,
            units: DialogUnits::Px,
            controls: vec![
                ControlSpec::new(10, Tab, (5, 5, 290, 160), "Général"),
                ControlSpec::new(11, Tab, (5, 5, 290, 160), "DCC"),
                ControlSpec::new(12, Check, (20, 40, 150, 18), "Se reconnecter").in_tab(10),
                ControlSpec::new(13, Check, (20, 40, 150, 18), "Accepter").in_tab(11),
                ControlSpec::new(14, Button, (220, 170, 75, 23), "OK").style("ok"),
            ],
        };
        let mut stack = ModalStack::new();
        let b = Rect::new(0, 0, 800, 600);
        stack.push(1, DialogView::new(&spec, &res), b);
        let d = stack.top_mut().unwrap();
        assert!(d.is_shown(2) && !d.is_shown(3));
        let tr = d.control_rect(10).unwrap();
        let tab2 = d.strip.as_ref().unwrap().tabs.tab_rects(&res, tr)[1];
        assert_eq!(stack.event(&Input::click(tab2.x + 4, tab2.y + 4), &res, &mut p, b), Some((1, DialogMsg::Clicked(11))));
        let d = stack.top_mut().unwrap();
        assert!(!d.is_shown(2) && d.is_shown(3));
        assert_eq!(d.selection(10), vec![1]);
        stack.push(2, DialogView::new(&spec, &res), b);
        assert!(!stack.dialogs[0].1.active);
        assert_eq!(stack.event(&Input::key(Key::Escape, Mods::NONE), &res, &mut p, b), Some((2, DialogMsg::Cancel)));
        stack.pop();
        assert!(stack.dialogs[0].1.active);
    }
}
