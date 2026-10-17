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
//!   demandent un [`Input::Tick`] par [`Platform::schedule_tick`].

pub mod canvas;
pub mod geom;
pub mod icons;
pub mod input;
pub mod text;
pub mod theme;

// pub mod chat;
// pub mod dialog;
// pub mod editbox;
// pub mod mdi;
// pub mod menu;
// pub mod nicklist;
// pub mod switchbar;
// pub mod toolbar;
pub mod widgets;

pub use canvas::{Bevel, Canvas, Image};
pub use geom::{Point, Rect, Size};
pub use icons::{IconId, Icons};
pub use input::{Cursor, Input, Key, Mods, MouseButton, Platform, TestPlatform};
pub use text::{Align, FontKind, FontSpec, Fonts, TextStyle};
pub use theme::Theme;


/// Ressources de dessin partagées : polices, thème, icônes.
pub struct Res {
    pub fonts: Fonts,
    pub theme: Theme,
    pub icons: Icons,
}

impl Res {
    /// Ressources avec le thème classique et les icônes de secours dessinées par programme.
    pub fn new(fonts: Fonts) -> Res {
        Res { fonts, theme: Theme::classic(), icons: Icons::with_fallbacks() }
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
