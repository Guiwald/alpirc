//! Événements d'entrée abstraits et services fournis par l'application ([`Platform`]).
//!
//! L'application (winit sur ordinateur, `NativeActivity` sur Android) traduit ses événements en [`Input`] et
//! les passe aux widgets ; les positions sont en pixels **logiques** du canevas.
//!
//! Convention de routage : les événements de souris peuvent être diffusés à tous les widgets (chacun ignore ce
//! qui tombe hors de son rectangle, sauf s'il « tient » la souris après un clic) ; les touches ne vont qu'au
//! widget qui a le focus.

use crate::geom::{Point, Rect};

/// Une touche, déjà résolue selon la disposition du clavier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    /// Un caractère ; en minuscule de base pour les raccourcis (Ctrl+B donne `Char('b')`).
    Char(char),
    Enter,
    Escape,
    Tab,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Insert,
    /// Touche de fonction F1 à F24.
    F(u8),
    /// Toute autre touche.
    Other,
}

/// Touches de modification.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    /// Cmd sur Mac, touche Windows/Super ailleurs.
    pub meta: bool,
}

impl Mods {
    pub const NONE: Mods = Mods { shift: false, ctrl: false, alt: false, meta: false };
    pub const SHIFT: Mods = Mods { shift: true, ctrl: false, alt: false, meta: false };
    pub const CTRL: Mods = Mods { shift: false, ctrl: true, alt: false, meta: false };
    pub const ALT: Mods = Mods { shift: false, ctrl: false, alt: true, meta: false };
    pub const META: Mods = Mods { shift: false, ctrl: false, alt: false, meta: true };

    /// « Commande » : Ctrl, ou Cmd sur Mac. Les raccourcis (copier, coller, gras…) répondent aux deux.
    pub const fn command(&self) -> bool {
        self.ctrl || self.meta
    }

    /// Aucune touche de modification.
    pub const fn is_empty(&self) -> bool {
        !self.shift && !self.ctrl && !self.alt && !self.meta
    }
}

/// Bouton de la souris.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u16),
}

/// Événement d'entrée.
#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    MouseMove {
        pos: Point,
    },
    /// `clicks` : 1, 2 (double-clic) ou 3 (triple-clic), calculé par l'application.
    MouseDown {
        pos: Point,
        button: MouseButton,
        mods: Mods,
        clicks: u8,
    },
    MouseUp {
        pos: Point,
        button: MouseButton,
        mods: Mods,
    },
    /// Molette, en lignes (`dy` positif = vers le haut, comme winit).
    Wheel {
        pos: Point,
        dx: f32,
        dy: f32,
        mods: Mods,
    },
    /// Appui sur une touche ; `text` est le texte produit, s'il y en a un.
    Key {
        key: Key,
        mods: Mods,
        text: Option<String>,
        repeat: bool,
    },
    /// Texte validé (méthode de saisie, champ natif d'Android, collage tapé).
    Text(String),
    /// Composition en cours de la méthode de saisie (souligné dans le champ).
    ImePreedit {
        text: String,
        cursor: Option<(usize, usize)>,
    },
    FocusIn,
    FocusOut,
    MouseLeave,
    /// Battement d'horloge : à envoyer quand un widget l'a demandé par [`Platform::schedule_tick`]
    /// (clignotement du curseur, répétition des flèches de défilement, défilement pendant un glisser).
    Tick,
}

impl Input {
    /// Position de la souris portée par l'événement, s'il y en a une.
    pub fn pos(&self) -> Option<Point> {
        match self {
            Input::MouseMove { pos } | Input::MouseDown { pos, .. } | Input::MouseUp { pos, .. } | Input::Wheel { pos, .. } => {
                Some(*pos)
            }
            _ => None,
        }
    }

    /// Le même événement, la position décalée de `(dx, dy)`.
    pub fn translated(&self, dx: i32, dy: i32) -> Input {
        let mut e = self.clone();
        match &mut e {
            Input::MouseMove { pos } | Input::MouseDown { pos, .. } | Input::MouseUp { pos, .. } | Input::Wheel { pos, .. } => {
                *pos = pos.offset(dx, dy);
            }
            _ => {}
        }
        e
    }

    /// Raccourci pour fabriquer un appui de touche sans texte.
    pub fn key(key: Key, mods: Mods) -> Input {
        let text = match key {
            Key::Char(c) if !mods.command() && !mods.alt => Some(c.to_string()),
            _ => None,
        };
        Input::Key { key, mods, text, repeat: false }
    }

    /// Raccourci : clic gauche simple.
    pub fn click(x: i32, y: i32) -> Input {
        Input::MouseDown { pos: Point::new(x, y), button: MouseButton::Left, mods: Mods::NONE, clicks: 1 }
    }

    /// Raccourci : relâchement du bouton gauche.
    pub fn release(x: i32, y: i32) -> Input {
        Input::MouseUp { pos: Point::new(x, y), button: MouseButton::Left, mods: Mods::NONE }
    }

    /// Raccourci : déplacement de la souris.
    pub fn moved(x: i32, y: i32) -> Input {
        Input::MouseMove { pos: Point::new(x, y) }
    }
}

/// Forme du pointeur de la souris.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Cursor {
    #[default]
    Default,
    Text,
    Hand,
    ResizeEW,
    ResizeNS,
    ResizeNWSE,
    ResizeNESW,
    Move,
}

/// Services que l'application rend aux widgets.
pub trait Platform {
    /// Contenu texte du presse-papiers.
    fn clipboard_get(&mut self) -> Option<String>;
    /// Remplace le contenu du presse-papiers.
    fn clipboard_set(&mut self, text: &str);
    /// Horloge monotone en millisecondes.
    fn now_ms(&self) -> i64;
    /// Forme du pointeur.
    fn set_cursor(&mut self, c: Cursor);
    /// Zone du curseur de saisie (pour placer la fenêtre de la méthode de saisie), en pixels logiques.
    fn set_ime_area(&mut self, rect: Rect);
    /// Demande un nouveau dessin.
    fn request_repaint(&mut self);
    /// Demande un [`Input::Tick`] à l'instant `at_ms` (horloge de [`Platform::now_ms`]). L'application peut
    /// regrouper les demandes et n'envoyer qu'un battement pour la plus proche. Par défaut : redessiner.
    fn schedule_tick(&mut self, at_ms: i64) {
        let _ = at_ms;
        self.request_repaint();
    }
}

/// Une plateforme factice, pour les tests et les exemples : presse-papiers en mémoire, horloge réglable.
#[derive(Debug, Default, Clone)]
pub struct TestPlatform {
    pub clipboard: Option<String>,
    pub now: i64,
    pub cursor: Cursor,
    pub ime_area: Option<Rect>,
    pub repaints: u32,
    /// Prochain battement demandé.
    pub tick_at: Option<i64>,
}

impl TestPlatform {
    pub fn new() -> TestPlatform {
        TestPlatform::default()
    }
}

impl Platform for TestPlatform {
    fn clipboard_get(&mut self) -> Option<String> {
        self.clipboard.clone()
    }
    fn clipboard_set(&mut self, text: &str) {
        self.clipboard = Some(text.to_string());
    }
    fn now_ms(&self) -> i64 {
        self.now
    }
    fn set_cursor(&mut self, c: Cursor) {
        self.cursor = c;
    }
    fn set_ime_area(&mut self, rect: Rect) {
        self.ime_area = Some(rect);
    }
    fn request_repaint(&mut self) {
        self.repaints += 1;
    }
    fn schedule_tick(&mut self, at_ms: i64) {
        self.tick_at = Some(self.tick_at.map_or(at_ms, |t| t.min(at_ms)));
    }
}
