//! Disposition de bureau façon mIRC : barre de menus, barre d'outils, switchbar, treebar, fenêtres internes ;
//! routage des événements et dessin.

use alpirc_core::{ConnState, WinId, WinKind};
use alpirc_script::PopupKind;
use alpirc_ui::widgets::ListViewMsg;
use alpirc_ui::{
    Canvas, ChatMsg, EditMsg, Input, Key, MenuBar, MenuMsg, MouseButton, NickMsg, Point, Rect, SwitchMsg, Toolbar,
    TreebarMsg, WordKind,
};

use crate::views::{self, ViewFocus, WinLines};
use crate::{LayoutKind, PopupSrc, Shell, ShellPlatform, switchbar_pos};

/// Rectangles des parties de la fenêtre principale (pixels logiques).
#[derive(Clone, Copy, Debug, Default)]
pub struct Layout {
    pub menubar: Option<Rect>,
    pub toolbar: Option<Rect>,
    pub switchbar: Option<Rect>,
    pub treebar: Option<Rect>,
    pub mdi: Rect,
}

/// Rectangles du contenu d'une fenêtre.
#[derive(Clone, Copy, Debug, Default)]
pub struct WinRects {
    pub chat: Rect,
    pub nicks: Option<Rect>,
    pub split: Option<Rect>,
    pub editbox: Option<Rect>,
    pub list: Option<Rect>,
}

const SPLIT_W: i32 = 3;

impl Shell {
    /// La disposition de bureau pour la taille actuelle.
    pub fn layout(&self) -> Layout {
        let full = Rect::new(0, 0, self.viewport.w, self.viewport.h);
        let mut rest = full;
        let mut l = Layout::default();
        if self.layout_kind == LayoutKind::Mobile {
            l.mdi = rest;
            return l;
        }
        let d = &self.client.settings().display;
        if !self.native_menus {
            let (top, r) = rest.split_top(MenuBar::height(&self.res));
            l.menubar = Some(top);
            rest = r;
        }
        if d.toolbar {
            let (top, r) = rest.split_top(Toolbar::height());
            l.toolbar = Some(top);
            rest = r;
        }
        match switchbar_pos(&d.switchbar) {
            "off" => {}
            "bottom" => {
                let h = self.switchbar.height_for(&self.res, rest.w);
                let (b, r) = rest.split_bottom(h);
                l.switchbar = Some(b);
                rest = r;
            }
            "left" => {
                let (a, r) = rest.split_left(130);
                l.switchbar = Some(a);
                rest = r;
            }
            "right" => {
                let (a, r) = rest.split_right(130);
                l.switchbar = Some(a);
                rest = r;
            }
            _ => {
                let h = self.switchbar.height_for(&self.res, rest.w);
                let (top, r) = rest.split_top(h);
                l.switchbar = Some(top);
                rest = r;
            }
        }
        if d.treebar {
            let (a, r) = rest.split_left(self.treebar.width);
            l.treebar = Some(a);
            rest = r;
        }
        l.mdi = rest;
        l
    }

    /// Rectangle du contenu d'une fenêtre (absolu).
    fn content_rect(&self, win: WinId) -> Option<Rect> {
        if self.layout_kind == LayoutKind::Mobile {
            return Some(self.mobile_content_rect());
        }
        self.mdi.content_rect(win as u64, self.layout().mdi)
    }

    /// Rectangles des parties d'une fenêtre (absolus).
    pub fn window_rects(&self, win: WinId) -> Option<WinRects> {
        let content = self.content_rect(win)?;
        let w = self.client.window(win)?;
        let v = self.views.get(&win)?;
        let mut r = WinRects::default();
        let mut rest = content;
        if views::has_editbox(w) {
            let (eb, top) = rest.split_bottom(v.editbox.preferred_height(&self.res));
            r.editbox = Some(eb);
            rest = top;
        }
        if views::has_list(w) {
            r.list = Some(rest);
            return Some(r);
        }
        if v.has_nicklist(self.client.settings().display.nicklist) && self.layout_kind == LayoutKind::Desktop {
            let nw = v.nick_width.clamp(60, (rest.w / 2).max(60));
            let (nicks, chat) = rest.split_right(nw);
            let (split, chat) = chat.split_right(SPLIT_W);
            r.nicks = Some(nicks);
            r.split = Some(split);
            rest = chat;
        }
        r.chat = rest;
        Some(r)
    }

    // -----------------------------------------------------------------------------------------------------------
    // Événements.

    /// Un événement de la plateforme (pixels logiques).
    pub fn input(&mut self, input: Input, p: &mut dyn ShellPlatform) {
        if self.layout_kind == LayoutKind::Mobile {
            self.mobile_input(input, p);
            return;
        }
        // 1. Menu surgissant ouvert : il prend tout.
        if let Some((menu, _)) = self.popup.as_mut() {
            let msg = menu.event(&input, &self.res, p);
            match msg {
                Some(MenuMsg::Activated { id, .. }) => {
                    let (_, src) = self.popup.take().expect("menu ouvert");
                    self.popup_activated(id, src, p);
                }
                Some(MenuMsg::Closed) => self.popup = None,
                _ => {}
            }
            p.request_repaint();
            return;
        }
        // 2. Boîtes de dialogue (modales).
        if !self.modals.is_empty() {
            let bounds = self.dialog_bounds();
            if let Some((key, msg)) = self.modals.event(&input, &self.res, p, bounds) {
                self.dialog_msg(key, msg, p);
            }
            // Raccourci de sortie toujours actif.
            p.request_repaint();
            return;
        }
        let l = self.layout();
        // 3. Barre de menus (et boutons de la fenêtre agrandie).
        if let Some(mb) = l.menubar {
            let titles = self.menubar_titles_rect(mb);
            let was_active = self.menubar.is_active();
            let bounds = self.dialog_bounds();
            if let Some(msg) = self.menubar.event(&input, titles, bounds, &self.res, p) {
                if let MenuMsg::Activated { id, .. } = msg {
                    self.menu_action(id, p);
                    return;
                }
            }
            if was_active || self.menubar.is_active() {
                p.request_repaint();
                return;
            }
            if matches!(input, Input::MouseDown { .. } | Input::MouseUp { .. } | Input::MouseMove { .. }) {
                let r = self.mdi.maximized_controls_event(&input, mb, p);
                if !r.is_empty() {
                    self.mdi_responses(r, p);
                    self.after(p);
                    return;
                }
                if let (Input::MouseDown { pos, .. }, Some(ir)) = (&input, self.mdi.maximized_icon_rect(mb)) {
                    if ir.contains(*pos) {
                        if let Some(a) = self.mdi.active() {
                            self.open_window_menu(a as WinId, Point::new(ir.x, mb.bottom()));
                            p.request_repaint();
                            return;
                        }
                    }
                }
            }
        }
        // 4. Raccourcis globaux.
        if let Input::Key { key, mods, .. } = &input {
            if let Some(id) = self.shortcut(*key, *mods) {
                self.menu_action(id, p);
                return;
            }
        }
        // 5. Barre d'outils, switchbar, treebar.
        if let Some(tr) = l.toolbar {
            // Bouton à bascule (Absent) : la commande, puis l'état réel suit la réponse du serveur.
            if let Some(alpirc_ui::ToolMsg::Clicked(id) | alpirc_ui::ToolMsg::Toggled(id, _)) = self.toolbar.event(&input, tr, &self.res, p) {
                self.menu_action(id, p);
                return;
            }
        }
        if let Some(sr) = l.switchbar {
            self.switchbar.vertical = sr.h > sr.w;
            match self.switchbar.event(&input, sr, &self.res, p) {
                Some(SwitchMsg::Activate(id)) => {
                    let now = p.now_ms();
                    let w = id as WinId;
                    if self.client.active() == Some(w) && self.mdi.child(id).is_some_and(|c| c.state != alpirc_ui::WindowState::Minimized) && !self.mdi.is_maximized() {
                        // Comme mIRC : un clic sur le bouton de la fenêtre active la réduit.
                        self.mdi.minimize(id);
                    } else {
                        if self.mdi.child(id).is_some_and(|c| c.state == alpirc_ui::WindowState::Minimized) {
                            self.mdi.restore(id);
                        }
                        self.activate(w, now);
                    }
                    self.after(p);
                    return;
                }
                Some(SwitchMsg::RightClick(id, pos)) => {
                    self.open_window_menu(id as WinId, pos);
                    p.request_repaint();
                    return;
                }
                Some(SwitchMsg::MiddleClick(id)) => {
                    let now = p.now_ms();
                    self.client.close_window(id as WinId, now);
                    self.after(p);
                    return;
                }
                None => {}
            }
        }
        // La treebar ne prend pas le clavier (comme mIRC) : sinon le focus de la fenêtre du système (FocusIn) lui
        // donnait Entrée et les flèches, perdus pour la ligne de saisie.
        let for_treebar =
            !matches!(input, Input::Key { .. } | Input::Text(_) | Input::ImePreedit { .. } | Input::FocusIn | Input::FocusOut);
        if let Some(tr) = l.treebar.filter(|_| for_treebar) {
            match self.treebar.event(&input, tr, &self.res, p) {
                Some(TreebarMsg::Activate(id)) => {
                    let now = p.now_ms();
                    if self.mdi.child(id).is_some_and(|c| c.state == alpirc_ui::WindowState::Minimized) {
                        self.mdi.restore(id);
                    }
                    self.activate(id as WinId, now);
                    self.after(p);
                    return;
                }
                Some(TreebarMsg::RightClick(id, pos)) => {
                    self.open_window_menu(id as WinId, pos);
                    return;
                }
                Some(TreebarMsg::MiddleClick(id)) => {
                    let now = p.now_ms();
                    self.client.close_window(id as WinId, now);
                    self.after(p);
                    return;
                }
                Some(_) => {
                    p.request_repaint();
                    return;
                }
                None => {}
            }
        }
        // 6. Zone des fenêtres.
        let r = self.mdi.event(&input, l.mdi, &self.res, p);
        if !r.is_empty() {
            self.mdi_responses(r, p);
            self.after(p);
        }
    }

    /// Raccourcis clavier globaux → commande de menu.
    fn shortcut(&self, key: Key, mods: alpirc_ui::Mods) -> Option<u32> {
        use crate::menus::ids;
        let cmd = mods.command();
        match key {
            Key::Char(c) if mods.alt && !cmd => match c.to_ascii_lowercase() {
                'o' => Some(ids::OPTIONS),
                'r' => Some(ids::SCRIPTS),
                'b' => Some(ids::ABOOK),
                'j' => Some(ids::FAV_FOLDER),
                'k' => Some(ids::COLORS),
                _ => None,
            },
            Key::Char(c) if cmd && !mods.alt => match c.to_ascii_lowercase() {
                'n' if !mods.shift => Some(ids::NEW_STATUS),
                'f' => Some(ids::FIND),
                'w' if mods.meta => Some(ids::CLOSE),
                '=' | '+' => Some(ids::FONT_BIGGER),
                '-' => Some(ids::FONT_SMALLER),
                _ => None,
            },
            Key::F(1) => Some(ids::HELP),
            _ => None,
        }
    }

    /// Événement pour le contenu d'une fenêtre (coordonnées absolues).
    pub(crate) fn content_event(&mut self, win: WinId, input: &Input, p: &mut dyn ShellPlatform) {
        let Some(r) = self.window_rects(win) else { return };
        let bounds = self.dialog_bounds();
        let Some(w) = self.client.window(win) else { return };
        let is_list = views::has_list(w);
        let kind = w.kind;
        // `on KEYDOWN`/`on KEYUP` des scripts (voir `keys.rs`) ; `/haltdef` : la touche n'est pas traitée.
        if matches!(input, Input::Key { .. }) && self.script_key(win, input, p) {
            return;
        }
        // Séparation discussion / pseudos.
        if let Some(split) = r.split {
            let v = self.views.get_mut(&win).expect("vue");
            match input {
                Input::MouseDown { pos, button: MouseButton::Left, .. } if split.contains(*pos) => {
                    v.split_drag = Some((pos.x, v.nick_width));
                    p.set_cursor(alpirc_ui::Cursor::ResizeEW);
                    return;
                }
                Input::MouseMove { pos } if v.split_drag.is_some() => {
                    let (x0, w0) = v.split_drag.unwrap();
                    v.nick_width = (w0 - (pos.x - x0)).clamp(60, 400);
                    p.request_repaint();
                    return;
                }
                Input::MouseUp { .. } if v.split_drag.is_some() => {
                    v.split_drag = None;
                    return;
                }
                Input::MouseMove { pos } if split.contains(*pos) => p.set_cursor(alpirc_ui::Cursor::ResizeEW),
                _ => {}
            }
        }

        let mut chat_msgs = Vec::new();
        let mut nick_msgs = Vec::new();
        let mut edit_msgs = Vec::new();
        let mut list_msgs = Vec::new();
        {
            let Shell { views, client, res, .. } = self;
            let Some(v) = views.get_mut(&win) else { return };
            let Some(w) = client.window(win) else { return };
            let nick_entries = v.nicks.entries().to_vec();
            let src = WinLines { win: w, client, nicks: &nick_entries };
            let is_mouse = matches!(input, Input::MouseDown { .. } | Input::MouseUp { .. } | Input::MouseMove { .. } | Input::Wheel { .. } | Input::MouseLeave);
            let is_tick = matches!(input, Input::Tick);
            if is_list {
                if let Some(lr) = r.list {
                    if is_mouse || is_tick || v.focus == ViewFocus::List || matches!(input, Input::Key { .. }) {
                        if let Some(m) = v.list.event(input, lr, res, p) {
                            list_msgs.push(m);
                        }
                    }
                }
            }
            if is_mouse || is_tick {
                if !is_list {
                    if let Some(m) = v.chat.event(input, r.chat, res, p, &src) {
                        chat_msgs.push(m);
                    }
                }
                if let Some(nr) = r.nicks {
                    if let Some(m) = v.nicks.event(input, nr, res, p) {
                        nick_msgs.push(m);
                    }
                }
                if let Some(er) = r.editbox {
                    if let Some(m) = v.editbox.event(input, er, bounds, res, p) {
                        edit_msgs.push(m);
                    }
                }
                if let Input::MouseDown { pos, .. } = input {
                    if r.nicks.is_some_and(|nr| nr.contains(*pos)) {
                        v.nicks.focused = true;
                    } else {
                        v.nicks.focused = false;
                    }
                    if r.editbox.is_some() {
                        v.set_focus(ViewFocus::Editbox);
                    }
                }
            } else if !is_list {
                // Clavier : défilement vers la discussion, le reste vers la ligne de saisie.
                let to_chat = matches!(input, Input::Key { key: Key::PageUp | Key::PageDown, .. })
                    || matches!(input, Input::Key { key: Key::Home | Key::End, mods, .. } if mods.command());
                if to_chat {
                    if let Some(m) = v.chat.event(input, r.chat, res, p, &src) {
                        chat_msgs.push(m);
                    }
                } else if let Some(er) = r.editbox {
                    // Ctrl+C sans sélection dans la saisie : copier la sélection de la discussion.
                    let copy = matches!(input, Input::Key { key: Key::Char('c'), mods, .. } if mods.command());
                    let chat_sel = if copy { v.chat.selected_text(&src).filter(|s| !s.is_empty()) } else { None };
                    if let (Some(sel), true) = (chat_sel, v.editbox.edit.core.selection().is_empty()) {
                        p.clipboard_set(&sel);
                    } else if let Some(m) = v.editbox.event(input, er, bounds, res, p) {
                        edit_msgs.push(m);
                    }
                }
            }
        }
        let now = p.now_ms();
        for m in chat_msgs {
            match m {
                ChatMsg::DoubleClick(word, k) => match k {
                    WordKind::Url => p.open_url(&word),
                    WordKind::Channel => self.client.command(win, &format!("/join {word}"), now),
                    WordKind::Nick => self.client.command(win, &format!("/query {word}"), now),
                    WordKind::Word => {}
                },
                ChatMsg::OpenUrl(u) => p.open_url(&u),
                ChatMsg::JoinChannel(c) => self.client.command(win, &format!("/join {c}"), now),
                ChatMsg::RightClick { pos, word } => match word {
                    Some((nick, WordKind::Nick)) if kind == WinKind::Channel => {
                        self.open_script_popup(PopupKind::Nicklist, win, vec![nick], pos);
                    }
                    _ => {
                        let pk = match kind {
                            WinKind::Channel => PopupKind::Channel,
                            WinKind::Query | WinKind::DccChat => PopupKind::Query,
                            WinKind::Custom => PopupKind::Custom,
                            _ => PopupKind::Status,
                        };
                        let sel = if pk == PopupKind::Query { vec![self.client.window(win).map(|w| w.name.clone()).unwrap_or_default()] } else { Vec::new() };
                        self.open_script_popup(pk, win, sel, pos);
                    }
                },
                ChatMsg::Selected(text) => {
                    if !text.is_empty() {
                        p.clipboard_set(&text);
                    }
                }
            }
        }
        for m in nick_msgs {
            match m {
                NickMsg::DoubleClick(n) => self.client.command(win, &format!("/query {n}"), now),
                NickMsg::RightClick { pos, nicks } => self.open_script_popup(PopupKind::Nicklist, win, nicks, pos),
                NickMsg::SelectionChanged => {}
            }
        }
        for m in edit_msgs {
            self.edit_msg(win, m, p);
        }
        for m in list_msgs {
            self.list_msg(win, kind, m, p);
        }
        p.request_repaint();
    }

    pub(crate) fn edit_msg(&mut self, win: WinId, m: EditMsg, p: &mut dyn ShellPlatform) {
        let now = p.now_ms();
        match m {
            EditMsg::Submit(text) => {
                self.client.input(win, &text, now);
                if let Some(v) = self.views.get_mut(&win) {
                    v.chat.scroll_to_bottom();
                }
            }
            EditMsg::SubmitPlain(text) => {
                self.client.command(win, &format!("/say {text}"), now);
                if let Some(v) = self.views.get_mut(&win) {
                    v.chat.scroll_to_bottom();
                }
            }
            EditMsg::PasteLines(lines) => {
                let threshold = self.client.settings().irc.confirm_paste_lines.max(1) as usize;
                if lines.len() >= threshold && lines.len() > 1 {
                    let name = self.client.window(win).map(|w| w.name.clone()).unwrap_or_default();
                    self.open_dialog(Box::new(crate::dialogs::PasteConfirm { win, win_name: name, lines }), p);
                } else {
                    for l in lines {
                        self.client.input(win, &l, now);
                    }
                }
            }
            EditMsg::TabComplete { word, cycle, .. } => {
                let idx = if cycle && self.tab_cycle.0 == win { self.tab_cycle.1 + 1 } else { 0 };
                self.tab_cycle = (win, idx);
                if let Some(rep) = self.client.tab_complete(win, &word, idx).or_else(|| if idx > 0 { self.client.tab_complete(win, &word, 0) } else { None }) {
                    if let Some(v) = self.views.get_mut(&win) {
                        v.editbox.complete(&rep);
                    }
                    if self.client.tab_complete(win, &word, idx).is_none() {
                        self.tab_cycle = (win, 0);
                    }
                }
            }
            EditMsg::Changed => {}
        }
        self.after(p);
    }

    fn list_msg(&mut self, win: WinId, kind: WinKind, m: ListViewMsg, p: &mut dyn ShellPlatform) {
        let now = p.now_ms();
        match m {
            ListViewMsg::DoubleClicked(i) => {
                let Some(v) = self.views.get(&win) else { return };
                let Some(row) = v.list.rows.get(i) else { return };
                let first = row.cells.first().cloned().unwrap_or_default();
                match kind {
                    WinKind::ChanList => self.client.command(win, &format!("/join {first}"), now),
                    WinKind::Urls => p.open_url(&first),
                    WinKind::Notify => self.notify_double_click(win, &first, now),
                    _ => {}
                }
            }
            ListViewMsg::HeaderClicked(_) => {
                if let Some(v) = self.views.get_mut(&win) {
                    v.list.sort_rows();
                }
            }
            ListViewMsg::RightClicked(Some(i), pos) if kind == WinKind::Notify => {
                let nick = self.views.get(&win).and_then(|v| v.list.rows.get(i)).and_then(|r| r.cells.first().cloned());
                if let Some(nick) = nick {
                    self.open_notify_popup(win, nick, pos);
                }
            }
            ListViewMsg::RightClicked(_, pos) if kind == WinKind::Custom => {
                self.open_script_popup(PopupKind::Custom, win, Vec::new(), pos);
            }
            _ => {}
        }
        self.after(p);
    }

    // -----------------------------------------------------------------------------------------------------------
    // Dessin.

    fn menubar_titles_rect(&self, mb: Rect) -> Rect {
        let left = self.mdi.maximized_icon_rect(mb).map_or(mb.x, |r| r.right() + 2);
        let right = self.mdi.maximized_controls_rect(mb).map_or(mb.right(), |r| r.x);
        Rect::from_corners(left, mb.y, right.max(left), mb.bottom())
    }

    /// Dessine toute la fenêtre (pixels logiques ; le canevas applique l'échelle).
    pub fn paint(&mut self, canvas: &mut Canvas) {
        if self.layout_kind == LayoutKind::Mobile {
            self.mobile_paint(canvas);
            return;
        }
        let l = self.layout();
        let t = &self.res.theme;
        canvas.fill(Rect::new(0, 0, self.viewport.w, self.viewport.h), t.face);
        if let Some(mb) = l.menubar {
            canvas.fill(mb, t.menu);
            if let (Some(ir), Some(a)) = (self.mdi.maximized_icon_rect(mb), self.mdi.active()) {
                if let Some(img) = self.mdi.child(a).and_then(|c| c.icon).and_then(|i| self.res.icons.image(i)) {
                    canvas.blit(img, ir.x + (ir.w - 16) / 2, ir.y + (ir.h - 16) / 2);
                }
            }
            self.menubar.paint(canvas, &self.res, self.menubar_titles_rect(mb));
            self.mdi.paint_maximized_controls(canvas, &self.res, mb);
        }
        if let Some(tr) = l.toolbar {
            self.toolbar.paint(canvas, &self.res, tr);
        }
        if let Some(sr) = l.switchbar {
            self.switchbar.paint(canvas, &self.res, sr);
        }
        if let Some(tr) = l.treebar {
            self.treebar.paint(canvas, &self.res, tr);
        }
        canvas.fill(l.mdi, t.workspace);
        {
            let Shell { mdi, views, client, res, .. } = self;
            let show_nicks = client.settings().display.nicklist;
            mdi.paint(canvas, res, l.mdi, &mut |id, c, rect| {
                let win = id as WinId;
                let (Some(v), Some(w)) = (views.get(&win), client.window(win)) else { return };
                paint_window(c, res, client, w, v, rect, show_nicks);
            });
        }
        // Surcouches.
        if l.menubar.is_some() {
            self.menubar.paint_popup(canvas, &self.res);
        }
        let full = Rect::new(0, 0, self.viewport.w, self.viewport.h);
        if l.toolbar.is_some() {
            self.toolbar.paint_tooltip(canvas, &self.res, full);
        }
        if l.switchbar.is_some() {
            self.switchbar.paint_tooltip(canvas, &self.res, full);
        }
        if let Some(v) = self.client.active().and_then(|a| self.views.get(&a)) {
            v.editbox.paint_popup(canvas, &self.res);
        }
        if let Some((menu, _)) = &self.popup {
            menu.paint(canvas, &self.res);
        }
        self.modals.paint(canvas, &self.res);
    }
}

/// Dessine le contenu d'une fenêtre dans `rect`.
pub(crate) fn paint_window(
    c: &mut Canvas,
    res: &alpirc_ui::Res,
    client: &alpirc_core::Client,
    w: &alpirc_core::Window,
    v: &views::WinView,
    rect: Rect,
    show_nicks: bool,
) {
    let mut rest = rect;
    let mut eb_rect = None;
    if views::has_editbox(w) {
        let (eb, top) = rest.split_bottom(v.editbox.preferred_height(res));
        eb_rect = Some(eb);
        rest = top;
    }
    if views::has_list(w) {
        v.list.paint(c, res, rest);
    } else {
        let entries = v.nicks.entries();
        let src = WinLines { win: w, client, nicks: entries };
        if v.has_nicklist(show_nicks) {
            let nw = v.nick_width.clamp(60, (rest.w / 2).max(60));
            let (nicks, chat) = rest.split_right(nw);
            let (split, chat) = chat.split_right(SPLIT_W);
            c.fill(split, res.theme.face);
            v.chat.paint(c, res, chat, &src);
            v.nicks.paint(c, res, nicks);
        } else {
            v.chat.paint(c, res, rest, &src);
        }
    }
    if let Some(er) = eb_rect {
        v.editbox.paint(c, res, er);
    }
}

/// État de connexion d'une fenêtre Status (pour l'icône).
#[allow(dead_code)]
pub(crate) fn is_connected(client: &alpirc_core::Client, w: &alpirc_core::Window) -> bool {
    w.conn.and_then(|c| client.conn_info(c)).is_some_and(|c| c.state != ConnState::Disconnected)
}

// `PopupSrc` est utilisé par `input` via `popup_activated`.
#[allow(dead_code)]
fn _uses(_: PopupSrc) {}
