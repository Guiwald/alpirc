//! `alpirc-ui` : la boîte à outils graphique d'AlpIRC (ADR 0005, 0008).
//!
//! Widgets façon Windows 98, fenêtres internes (MDI), switchbar, treebar, barre d'outils, menus, boîtes de
//! dialogue pilotées par des données, et la vue de discussion. La crate ne dépend d'aucune plateforme : elle
//! dessine dans un [`Canvas`] (pixels physiques `0x00RRGGBB`, prêts pour softbuffer) et reçoit des
//! [`Input`] abstraits. L'application (winit, Android) fait le reste.
//!
//! Conventions :
//! - géométrie en pixels **logiques** (`i32`) ; le canevas convertit selon son échelle ;
//! - chaque composant a un état, une méthode `paint(&self, canvas, res, rect, …)` et une méthode
//!   `event(&mut self, input, rect, res, platform, …)` qui renvoie un message (`Option<…Msg>` le plus souvent) ;
//! - [`Res`] regroupe ce qu'il faut pour dessiner : polices, thème, icônes ;
//! - les événements de souris peuvent être diffusés à tous les composants (chacun ignore ce qui tombe hors de
//!   son rectangle, sauf pendant un glisser qu'il a commencé), les touches ne vont qu'à celui qui a le focus ;
//! - les composants animés (curseur clignotant, répétition des flèches, défilement pendant un glisser)
//!   demandent un [`Input::Tick`] par [`Platform::schedule_tick`] ;
//! - les surcouches (menu ouvert, liste déroulante ouverte, palette Ctrl+K, info-bulle) se dessinent en
//!   dernier par leur méthode `paint_popup` / `paint_tooltip`, et reçoivent les événements en premier.
//!
//! # Composer une image
//!
//! ```no_run
//! use alpirc_ui::*;
//!
//! // Les octets des polices viennent de l'application (intégrés au binaire, ou lus une fois).
//! fn leak(p: &str) -> &'static [u8] {
//!     Box::leak(std::fs::read(p).unwrap().into_boxed_slice())
//! }
//! let fonts = Fonts::new(
//!     leak("assets/fonts/FSEX300.ttf"),
//!     leak("assets/fonts/PixelOperator.ttf"),
//!     leak("assets/fonts/PixelOperator-Bold.ttf"),
//! );
//! fonts.load_system_fallbacks(); // emoji, CJK…
//! let res = Res::new(fonts);
//! let mut canvas = Canvas::new(2048, 1536, 2.0); // pixels physiques, échelle Retina
//! let mut platform = TestPlatform::new(); // l'application fournit sa propre `Platform`
//!
//! let mut chat = ChatView::new();
//! let mut src = VecSource::new(); // ou la fenêtre d'alpirc-core, qui implémente `ChatSource`
//! src.push("[21:02] <Guiwald> salut \x02#lyon\x02 !");
//! let mut editbox = Editbox::new();
//! editbox.focused = true;
//!
//! // Disposition, en pixels logiques.
//! let full = canvas.bounds();
//! let (eb_rect, chat_rect) = full.split_bottom(editbox.preferred_height(&res));
//!
//! // Événements : la souris à tous, le clavier au composant qui a le focus.
//! let key = Input::key(Key::Char('a'), Mods::NONE);
//! if let Some(EditMsg::Submit(line)) = editbox.event(&key, eb_rect, full, &res, &mut platform) {
//!     println!("envoyer {line}");
//! }
//! let _ = chat.event(&Input::key(Key::PageUp, Mods::NONE), chat_rect, &res, &mut platform, &src);
//!
//! // Dessin : le contenu, puis les surcouches.
//! chat.paint(&mut canvas, &res, chat_rect, &src);
//! editbox.paint(&mut canvas, &res, eb_rect);
//! editbox.paint_popup(&mut canvas, &res);
//! // `canvas.pixels` (0x00RRGGBB) part tel quel vers softbuffer.
//! ```

// Style : on garde parfois des `if` imbriqués pour la lisibilité.
#![allow(clippy::collapsible_if)]

pub mod canvas;
pub mod geom;
pub mod icons;
pub mod input;
pub mod text;
pub mod theme;

pub mod chat;
pub mod dialog;
pub mod editbox;
pub mod mdi;
pub mod menu;
pub mod nicklist;
pub mod switchbar;
pub mod toolbar;
pub mod widgets;

pub use canvas::{Bevel, Canvas, Image};
pub use geom::{Point, Rect, Size};
pub use icons::{IconId, Icons};
pub use input::{Cursor, Input, Key, Mods, MouseButton, Platform, TestPlatform};
pub use text::{Align, FontKind, FontSpec, Fonts, TextStyle};
pub use theme::Theme;
pub use menu::{Menu, MenuBar, MenuItem, MenuMsg};
pub use chat::{ChatLine, ChatMsg, ChatSource, ChatView, IndentMode, VecSource, WordKind, draw_mirc_line};
pub use editbox::{ColorPicker, EditMsg, Editbox, PickerMsg};
pub use nicklist::{NickEntry, NickList, NickMsg};
pub use switchbar::{Activity, SwitchItem, SwitchMsg, Switchbar, Treebar, TreebarMsg, TreebarNode};
pub use toolbar::{ToolItem, ToolMsg, Toolbar};
pub use mdi::{CaptionButton, MdiArea, MdiChild, MdiResponse, WindowState};
pub use dialog::{ControlKind, ControlSpec, DialogMsg, DialogSpec, DialogUnits, DialogView, DluScale, ModalStack};


/// Ressources de dessin partagées : polices, thème, icônes.
pub struct Res {
    pub fonts: Fonts,
    pub theme: Theme,
    pub icons: Icons,
    /// Hauteur minimale des lignes des listes (listes simples, listes à colonnes, arbres), en pixels logiques :
    /// 0 sur ordinateur ; plus haut sur un téléphone, pour des zones tactiles plus grandes.
    pub min_row_h: i32,
}

impl Res {
    /// Ressources avec le thème classique et les icônes de secours dessinées par programme.
    pub fn new(fonts: Fonts) -> Res {
        Res { fonts, theme: Theme::classic(), icons: Icons::with_fallbacks(), min_row_h: 0 }
    }

    /// Hauteur d'une ligne de texte de l'interface.
    pub fn ui_line(&self) -> i32 {
        self.fonts.line_height(FontKind::Ui)
    }
}

#[cfg(test)]
pub(crate) fn test_res() -> Res {
    Res::new(text::tests::fonts())
}
