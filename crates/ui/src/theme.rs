//! Thèmes : toutes les couleurs de l'interface (façon Windows 98) et des discussions (palette mIRC).
//!
//! Les couleurs sont en `0xRRGGBB`. Les couleurs des discussions (fond, texte, liste des pseudos, ligne de
//! saisie) sont des **numéros** de la palette de base (0 à 15), comme dans mIRC : changer la palette change
//! tout ce qui s'y réfère.

use alpirc_proto::format::{self, Color};
use serde::{Deserialize, Serialize};

/// Jeu de couleurs complet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    /// Nom affiché du thème.
    pub name: String,

    // --- Relief façon Windows 98 ---
    /// Face des boutons et des boîtes (`COLOR_3DFACE`).
    pub face: u32,
    /// Bord éclairé extérieur (`COLOR_3DHILIGHT`, blanc).
    pub highlight: u32,
    /// Bord éclairé intérieur (`COLOR_3DLIGHT`).
    pub light: u32,
    /// Ombre (`COLOR_3DSHADOW`).
    pub shadow: u32,
    /// Ombre foncée (`COLOR_3DDKSHADOW`, noir).
    pub dark: u32,
    /// Texte sur les faces (boutons, étiquettes, menus).
    pub text: u32,
    /// Texte grisé (dessiné en relief : `highlight` décalé puis cette couleur).
    pub disabled_text: u32,

    // --- Champs et listes ---
    /// Fond des champs de saisie et des listes.
    pub window: u32,
    /// Texte des champs et des listes.
    pub window_text: u32,
    /// Fond de la sélection.
    pub selection: u32,
    /// Texte sélectionné.
    pub selection_text: u32,
    /// Fond de la sélection quand le widget n'a pas le focus.
    pub selection_inactive: u32,
    /// Texte sélectionné sans le focus.
    pub selection_inactive_text: u32,

    // --- Barres de titre ---
    pub title_active_from: u32,
    pub title_active_to: u32,
    pub title_active_text: u32,
    pub title_inactive_from: u32,
    pub title_inactive_to: u32,
    pub title_inactive_text: u32,

    // --- Divers ---
    /// Fond de la zone MDI (« espace de travail »).
    pub workspace: u32,
    /// Fond des info-bulles.
    pub tooltip: u32,
    /// Texte des info-bulles.
    pub tooltip_text: u32,
    /// Liens (contrôle `link` des dialogues).
    pub link: u32,
    /// Blocs de la barre de progression.
    pub progress: u32,
    /// Fond des menus.
    pub menu: u32,
    /// Texte des menus.
    pub menu_text: u32,
    /// Fond de l'élément de menu survolé.
    pub menu_highlight: u32,
    /// Texte de l'élément de menu survolé.
    pub menu_highlight_text: u32,

    // --- Discussions (palette mIRC) ---
    /// Les 16 couleurs de base de mIRC (redéfinissables) ; 16 à 98 viennent de [`format::PALETTE`].
    pub palette: [u32; 16],
    /// Fond des fenêtres de discussion (numéro de palette).
    pub chat_bg: u8,
    /// Texte par défaut des discussions (numéro de palette).
    pub chat_fg: u8,
    /// Fond de la liste des pseudos.
    pub nicklist_bg: u8,
    /// Texte de la liste des pseudos.
    pub nicklist_fg: u8,
    /// Fond de la ligne de saisie.
    pub editbox_bg: u8,
    /// Texte de la ligne de saisie.
    pub editbox_fg: u8,
    /// Pseudos absents (away) dans la liste.
    pub nicklist_away: u8,
    /// Switchbar : activité « événement » (arrivées, départs…).
    pub activity_event: u8,
    /// Switchbar : nouveau message.
    pub activity_message: u8,
    /// Switchbar : on vous a nommé.
    pub activity_highlight: u8,
    /// Ligne de repère (« marker line »).
    pub marker_line: u32,
    /// Fond des occurrences de la recherche.
    pub search_bg: u32,
    /// Texte des occurrences de la recherche.
    pub search_fg: u32,
}

impl Default for Theme {
    fn default() -> Theme {
        Theme::classic()
    }
}

/// Palette de base de mIRC.
pub const MIRC_BASE16: [u32; 16] = [
    0xFFFFFF, 0x000000, 0x00007F, 0x009300, 0xFF0000, 0x7F0000, 0x9C009C, 0xFC7F00, 0xFFFF00, 0x00FC00, 0x009393, 0x00FFFF,
    0x0000FC, 0xFF00FF, 0x7F7F7F, 0xD2D2D2,
];

impl Theme {
    /// Windows 98 et couleurs par défaut de mIRC : discussions noir sur blanc.
    pub fn classic() -> Theme {
        Theme {
            name: "Classique".into(),
            face: 0xC0C0C0,
            highlight: 0xFFFFFF,
            light: 0xDFDFDF,
            shadow: 0x808080,
            dark: 0x000000,
            text: 0x000000,
            disabled_text: 0x808080,
            window: 0xFFFFFF,
            window_text: 0x000000,
            selection: 0x000080,
            selection_text: 0xFFFFFF,
            selection_inactive: 0xC0C0C0,
            selection_inactive_text: 0x000000,
            title_active_from: 0x000080,
            title_active_to: 0x1084D0,
            title_active_text: 0xFFFFFF,
            title_inactive_from: 0x808080,
            title_inactive_to: 0xB5B5B5,
            title_inactive_text: 0xC0C0C0,
            workspace: 0x808080,
            tooltip: 0xFFFFE1,
            tooltip_text: 0x000000,
            link: 0x0000FF,
            progress: 0x000080,
            menu: 0xC0C0C0,
            menu_text: 0x000000,
            menu_highlight: 0x000080,
            menu_highlight_text: 0xFFFFFF,
            palette: MIRC_BASE16,
            chat_bg: 0,
            chat_fg: 1,
            nicklist_bg: 0,
            nicklist_fg: 1,
            editbox_bg: 0,
            editbox_fg: 1,
            nicklist_away: 14,
            activity_event: 3,
            activity_message: 4,
            activity_highlight: 12,
            marker_line: 0xFF0000,
            search_bg: 0xFFFF00,
            search_fg: 0x000000,
        }
    }

    /// Mode sombre : chrome gris foncé (toujours en relief), discussions claires sur fond sombre, palette de
    /// base remappée pour rester lisible.
    pub fn dark() -> Theme {
        Theme {
            name: "Sombre".into(),
            face: 0x3A3A3A,
            highlight: 0x6A6A6A,
            light: 0x4C4C4C,
            shadow: 0x242424,
            dark: 0x0C0C0C,
            text: 0xE0E0E0,
            disabled_text: 0x7A7A7A,
            window: 0x1E1E1E,
            window_text: 0xE0E0E0,
            selection: 0x2F5FA8,
            selection_text: 0xFFFFFF,
            selection_inactive: 0x505050,
            selection_inactive_text: 0xE0E0E0,
            title_active_from: 0x1C3A6E,
            title_active_to: 0x3D6FB0,
            title_active_text: 0xFFFFFF,
            title_inactive_from: 0x404040,
            title_inactive_to: 0x5A5A5A,
            title_inactive_text: 0xA0A0A0,
            workspace: 0x262626,
            tooltip: 0x45453A,
            tooltip_text: 0xF0F0E0,
            link: 0x7FB2FF,
            progress: 0x3D6FB0,
            menu: 0x3A3A3A,
            menu_text: 0xE0E0E0,
            menu_highlight: 0x2F5FA8,
            menu_highlight_text: 0xFFFFFF,
            palette: [
                0xE8E8E8, // 0 blanc → texte clair
                0x181818, // 1 noir → fond
                0x6C8CFF, // 2 bleu marine → bleu lisible
                0x4CC24C, // 3 vert
                0xFF5C5C, // 4 rouge
                0xC8764E, // 5 marron
                0xC77DE0, // 6 violet
                0xFFA040, // 7 orange
                0xFFE65C, // 8 jaune
                0x7CFF7C, // 9 vert clair
                0x3CC0C0, // 10 sarcelle
                0x7CFFFF, // 11 cyan
                0x7C9CFF, // 12 bleu
                0xFF7CFF, // 13 rose
                0x9A9A9A, // 14 gris
                0x5C5C5C, // 15 gris clair → gris moyen
            ],
            chat_bg: 1,
            chat_fg: 0,
            nicklist_bg: 1,
            nicklist_fg: 0,
            editbox_bg: 1,
            editbox_fg: 0,
            nicklist_away: 14,
            activity_event: 3,
            activity_message: 4,
            activity_highlight: 8,
            marker_line: 0xFF4040,
            search_bg: 0xB08800,
            search_fg: 0x000000,
        }
    }

    /// Couleur RGB d'un numéro de palette mIRC (0 à 98).
    pub fn color(&self, index: u8) -> u32 {
        format::rgb(Color::Index(index), &self.palette)
    }

    /// Couleur RGB d'une couleur mIRC (numéro ou valeur exacte).
    pub fn mirc_rgb(&self, c: Color) -> u32 {
        format::rgb(c, &self.palette)
    }

    /// Fond des discussions (RGB).
    pub fn chat_background(&self) -> u32 {
        self.color(self.chat_bg)
    }

    /// Texte par défaut des discussions (RGB).
    pub fn chat_foreground(&self) -> u32 {
        self.color(self.chat_fg)
    }

    /// Lit un thème TOML/JSON déjà désérialisé : rien de spécial, mais garantit les valeurs par défaut.
    pub fn named(name: &str) -> Option<Theme> {
        match name {
            "classic" | "classique" => Some(Theme::classic()),
            "dark" | "sombre" => Some(Theme::dark()),
            _ => None,
        }
    }
}

/// Mélange deux couleurs : `t` = 0 donne `a`, 255 donne `b`.
pub fn mix(a: u32, b: u32, t: u32) -> u32 {
    let t = t.min(255);
    let ch = |s: u32| -> u32 {
        let ca = (a >> s) & 0xFF;
        let cb = (b >> s) & 0xFF;
        ((ca * (255 - t) + cb * t + 127) / 255) << s
    };
    ch(16) | ch(8) | ch(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_lookup() {
        let t = Theme::classic();
        assert_eq!(t.color(4), 0xFF0000);
        assert_eq!(t.color(52), format::PALETTE[52]);
        assert_eq!(t.chat_background(), 0xFFFFFF);
        let d = Theme::dark();
        assert_ne!(d.chat_background(), d.chat_foreground());
        assert_eq!(mix(0x000000, 0xFFFFFF, 255), 0xFFFFFF);
        assert_eq!(mix(0x000000, 0xFFFFFF, 0), 0x000000);
    }
}
