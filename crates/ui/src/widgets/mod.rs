//! Widgets façon Windows 98.
//!
//! Chaque widget a un état public, `paint(&self, canvas, res, rect)` et
//! `event(&mut self, input, rect, res, platform) -> Option<…Msg>`. Le rectangle est donné à chaque appel :
//! la disposition appartient à l'appelant (une boîte de dialogue, l'application).

pub mod button;
pub mod check;
pub mod combo;
pub mod edit;
pub mod label;
pub mod listbox;
pub mod listview;
pub mod progress;
pub mod scrollbar;
pub mod tabs;
pub mod textarea;
pub mod textedit;
pub mod tooltip;
pub mod treeview;

pub use button::{Button, ButtonKind, ButtonMsg};
pub use check::{CheckBox, CheckMsg, RadioButton, RadioMsg};
pub use combo::{ComboBox, ComboMsg};
pub use edit::{EditCore, EditResult};
pub use label::{GroupBox, Label, Link, LinkMsg};
pub use listbox::{ListBox, ListItem, ListMsg, ListSel, SelectMode};
pub use listview::{Column, ListView, ListViewMsg};
pub use progress::ProgressBar;
pub use scrollbar::{Orientation, ScrollBar, ScrollMsg};
pub use tabs::{Tabs, TabsMsg};
pub use textarea::{TextArea, TextAreaMsg};
pub use textedit::{TextEdit, TextEditMsg};
pub use tooltip::{Tooltip, TooltipState};
pub use treeview::{TreeNode, TreeView, TreeViewMsg};

use crate::canvas::Canvas;

/// Dessine un motif de pixels logiques (une chaîne par ligne) ; `color` donne la couleur de chaque caractère
/// (`None` : transparent).
pub fn draw_pattern(canvas: &mut Canvas, x: i32, y: i32, rows: &[&str], color: impl Fn(u8) -> Option<u32>) {
    for (dy, row) in rows.iter().enumerate() {
        for (dx, ch) in row.bytes().enumerate() {
            if let Some(c) = color(ch) {
                canvas.pixel(x + dx as i32, y + dy as i32, c);
            }
        }
    }
}

/// Direction d'une flèche.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrowDir {
    Up,
    Down,
    Left,
    Right,
}

/// Petite flèche pleine de Windows 98 (7x4 ou 4x7), centrée dans le rectangle.
pub fn draw_arrow(canvas: &mut Canvas, r: crate::Rect, dir: ArrowDir, color: u32) {
    let (w, h) = match dir {
        ArrowDir::Up | ArrowDir::Down => (7, 4),
        ArrowDir::Left | ArrowDir::Right => (4, 7),
    };
    let x0 = r.x + (r.w - w) / 2;
    let y0 = r.y + (r.h - h) / 2;
    for i in 0..4 {
        let len = 7 - 2 * i;
        match dir {
            ArrowDir::Down => canvas.hline(x0 + i, y0 + i, len, color),
            ArrowDir::Up => canvas.hline(x0 + i, y0 + 3 - i, len, color),
            ArrowDir::Right => canvas.vline(x0 + i, y0 + i, len, color),
            ArrowDir::Left => canvas.vline(x0 + 3 - i, y0 + i, len, color),
        }
    }
}

/// Coche de 7x7 (cases à cocher, menus).
pub const CHECK_MARK: [&str; 7] = ["......k", ".....kk", "k...kkk", "kk.kkk.", "kkkkk..", ".kkk...", "..k...."];

/// Point de bouton radio de menu (5x5).
pub const MENU_DOT: [&str; 5] = [".kkk.", "kkkkk", "kkkkk", "kkkkk", ".kkk."];

/// Remplit un rectangle d'un damier de deux couleurs (piste des barres de défilement, bouton actif de la
/// barre des tâches), à l'échelle du pixel logique.
pub fn dither(canvas: &mut Canvas, r: crate::Rect, a: u32, b: u32) {
    canvas.fill(r, a);
    for y in r.y..r.bottom() {
        let mut x = r.x + ((y + r.x) & 1);
        while x < r.right() {
            canvas.pixel(x, y, b);
            x += 2;
        }
    }
}

/// Répétition clavier/souris façon Windows : premier délai, puis cadence.
pub const REPEAT_DELAY_MS: i64 = 400;
/// Cadence de répétition.
pub const REPEAT_RATE_MS: i64 = 50;
/// Demi-période du clignotement du curseur de saisie.
pub const CARET_BLINK_MS: i64 = 530;
