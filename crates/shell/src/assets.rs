//! Ressources intégrées au programme : polices et icônes en pixel-art (`assets/`).

use alpirc_ui::{Fonts, Icons};

pub const FONT_CHAT: &[u8] = include_bytes!("../../../assets/fonts/FSEX300.ttf");
pub const FONT_UI: &[u8] = include_bytes!("../../../assets/fonts/PixelOperator.ttf");
pub const FONT_UI_BOLD: &[u8] = include_bytes!("../../../assets/fonts/PixelOperator-Bold.ttf");

/// Icônes : (nom dans le registre, PNG).
pub const ICONS: &[(&str, &[u8])] = &[
    ("app/alpirc-16", include_bytes!("../../../assets/icons/app/alpirc-16.png")),
    ("app/alpirc-32", include_bytes!("../../../assets/icons/app/alpirc-32.png")),
    ("app/alpirc-notification", include_bytes!("../../../assets/icons/app/alpirc-notification.png")),
    ("toolbar/abook", include_bytes!("../../../assets/icons/toolbar/abook.png")),
    ("toolbar/about", include_bytes!("../../../assets/icons/toolbar/about.png")),
    ("toolbar/away", include_bytes!("../../../assets/icons/toolbar/away.png")),
    ("toolbar/cascade", include_bytes!("../../../assets/icons/toolbar/cascade.png")),
    ("toolbar/chanlist", include_bytes!("../../../assets/icons/toolbar/chanlist.png")),
    ("toolbar/colors", include_bytes!("../../../assets/icons/toolbar/colors.png")),
    ("toolbar/connect", include_bytes!("../../../assets/icons/toolbar/connect.png")),
    ("toolbar/dcc_chat", include_bytes!("../../../assets/icons/toolbar/dcc_chat.png")),
    ("toolbar/dcc_send", include_bytes!("../../../assets/icons/toolbar/dcc_send.png")),
    ("toolbar/disconnect", include_bytes!("../../../assets/icons/toolbar/disconnect.png")),
    ("toolbar/favorites", include_bytes!("../../../assets/icons/toolbar/favorites.png")),
    ("toolbar/find", include_bytes!("../../../assets/icons/toolbar/find.png")),
    ("toolbar/help", include_bytes!("../../../assets/icons/toolbar/help.png")),
    ("toolbar/log", include_bytes!("../../../assets/icons/toolbar/log.png")),
    ("toolbar/notify", include_bytes!("../../../assets/icons/toolbar/notify.png")),
    ("toolbar/options", include_bytes!("../../../assets/icons/toolbar/options.png")),
    ("toolbar/relay", include_bytes!("../../../assets/icons/toolbar/relay.png")),
    ("toolbar/scripts", include_bytes!("../../../assets/icons/toolbar/scripts.png")),
    ("toolbar/tile_h", include_bytes!("../../../assets/icons/toolbar/tile_h.png")),
    ("toolbar/tile_v", include_bytes!("../../../assets/icons/toolbar/tile_v.png")),
    ("toolbar/transfers", include_bytes!("../../../assets/icons/toolbar/transfers.png")),
    ("toolbar/urls", include_bytes!("../../../assets/icons/toolbar/urls.png")),
    ("window/chanlist", include_bytes!("../../../assets/icons/window/chanlist.png")),
    ("window/channel", include_bytes!("../../../assets/icons/window/channel.png")),
    ("window/custom", include_bytes!("../../../assets/icons/window/custom.png")),
    ("window/dcc_chat", include_bytes!("../../../assets/icons/window/dcc_chat.png")),
    ("window/network_connected", include_bytes!("../../../assets/icons/window/network_connected.png")),
    ("window/network_disconnected", include_bytes!("../../../assets/icons/window/network_disconnected.png")),
    ("window/notify", include_bytes!("../../../assets/icons/window/notify.png")),
    ("window/query", include_bytes!("../../../assets/icons/window/query.png")),
    ("window/script_editor", include_bytes!("../../../assets/icons/window/script_editor.png")),
    ("window/status", include_bytes!("../../../assets/icons/window/status.png")),
    ("window/transfers", include_bytes!("../../../assets/icons/window/transfers.png")),
    ("window/urls", include_bytes!("../../../assets/icons/window/urls.png")),
    ("ui/arrow_down", include_bytes!("../../../assets/icons/ui/arrow_down.png")),
    ("ui/arrow_left", include_bytes!("../../../assets/icons/ui/arrow_left.png")),
    ("ui/arrow_right", include_bytes!("../../../assets/icons/ui/arrow_right.png")),
    ("ui/arrow_up", include_bytes!("../../../assets/icons/ui/arrow_up.png")),
    ("ui/check", include_bytes!("../../../assets/icons/ui/check.png")),
    ("ui/close_x", include_bytes!("../../../assets/icons/ui/close_x.png")),
    ("ui/error", include_bytes!("../../../assets/icons/ui/error.png")),
    ("ui/folder_closed", include_bytes!("../../../assets/icons/ui/folder_closed.png")),
    ("ui/folder_open", include_bytes!("../../../assets/icons/ui/folder_open.png")),
    ("ui/info", include_bytes!("../../../assets/icons/ui/info.png")),
    ("ui/maximize", include_bytes!("../../../assets/icons/ui/maximize.png")),
    ("ui/minimize", include_bytes!("../../../assets/icons/ui/minimize.png")),
    ("ui/msg_error_32", include_bytes!("../../../assets/icons/ui/msg_error_32.png")),
    ("ui/msg_info_32", include_bytes!("../../../assets/icons/ui/msg_info_32.png")),
    ("ui/msg_question_32", include_bytes!("../../../assets/icons/ui/msg_question_32.png")),
    ("ui/msg_warning_32", include_bytes!("../../../assets/icons/ui/msg_warning_32.png")),
    ("ui/radio_dot", include_bytes!("../../../assets/icons/ui/radio_dot.png")),
    ("ui/restore", include_bytes!("../../../assets/icons/ui/restore.png")),
    ("ui/warning", include_bytes!("../../../assets/icons/ui/warning.png")),
];

/// Les polices d'AlpIRC (sans les polices de repli du système).
pub fn fonts() -> Fonts {
    Fonts::new(FONT_CHAT, FONT_UI, FONT_UI_BOLD)
}

/// Enregistre toutes les icônes.
pub fn register_icons(icons: &mut Icons) {
    for (name, png) in ICONS {
        icons.register_png(name, png);
    }
}

/// L'icône de l'application en 32×32 (PNG), pour la fenêtre du système.
pub fn app_icon_png() -> &'static [u8] {
    ICONS.iter().find(|(n, _)| *n == "app/alpirc-32").map(|(_, b)| *b).unwrap_or(&[])
}
