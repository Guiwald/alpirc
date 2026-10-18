//! Ressources intégrées au programme : polices, icônes en pixel-art et sons des événements (`assets/`).

use std::path::Path;

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

/// Sons fournis (originaux, calculés par `assets/sounds/src/make_sounds.py`) : (nom du fichier, WAV). Les noms
/// sont ceux de `alpirc_core::settings::BUILTIN_SOUNDS`.
pub const SOUNDS: &[(&str, &[u8])] = &[
    ("alpirc-highlight.wav", include_bytes!("../../../assets/sounds/alpirc-highlight.wav")),
    ("alpirc-query.wav", include_bytes!("../../../assets/sounds/alpirc-query.wav")),
    ("alpirc-notify.wav", include_bytes!("../../../assets/sounds/alpirc-notify.wav")),
    ("alpirc-dcc.wav", include_bytes!("../../../assets/sounds/alpirc-dcc.wav")),
    ("alpirc-connect.wav", include_bytes!("../../../assets/sounds/alpirc-connect.wav")),
    ("alpirc-disconnect.wav", include_bytes!("../../../assets/sounds/alpirc-disconnect.wav")),
    ("alpirc-kick.wav", include_bytes!("../../../assets/sounds/alpirc-kick.wav")),
    ("alpirc-invite.wav", include_bytes!("../../../assets/sounds/alpirc-invite.wav")),
    ("alpirc-message.wav", include_bytes!("../../../assets/sounds/alpirc-message.wav")),
    ("alpirc-ding.wav", include_bytes!("../../../assets/sounds/alpirc-ding.wav")),
];

/// Installe les sons fournis dans `<données>/sounds` s'ils manquent : un fichier déjà là (même modifié ou remplacé
/// par l'utilisateur) n'est jamais écrasé. Rend le nombre de fichiers écrits.
pub fn install_sounds(data_dir: &Path) -> usize {
    let dir = data_dir.join("sounds");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        log::warn!("dossier des sons impossible à créer ({}) : {e}", dir.display());
        return 0;
    }
    let mut n = 0;
    for (name, data) in SOUNDS {
        let path = dir.join(name);
        // `create_new` : jamais d'écrasement, même si le fichier apparaît entre-temps.
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut f) => {
                use std::io::Write;
                if let Err(e) = f.write_all(data) {
                    log::warn!("son non installé ({}) : {e}", path.display());
                    drop(f);
                    let _ = std::fs::remove_file(&path);
                } else {
                    n += 1;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => log::warn!("son non installé ({}) : {e}", path.display()),
        }
    }
    n
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sounds_are_small_pcm_wavs_named_like_the_settings() {
        let names: Vec<&str> = SOUNDS.iter().map(|(n, _)| *n).collect();
        assert_eq!(names, alpirc_core::settings::BUILTIN_SOUNDS);
        for (name, wav) in SOUNDS {
            let u16_at = |i: usize| u16::from_le_bytes([wav[i], wav[i + 1]]);
            assert!(wav.starts_with(b"RIFF") && &wav[8..16] == b"WAVEfmt ", "{name}");
            assert_eq!((u16_at(20), u16_at(22), u16_at(34)), (1, 1, 16), "{name} : PCM mono 16 bits");
            assert!(wav.len() < 64 * 1024, "{name} : court");
        }
    }

    #[test]
    fn install_never_overwrites() {
        let dir = std::env::temp_dir().join(format!("alpirc-sounds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sounds")).unwrap();
        std::fs::write(dir.join("sounds/alpirc-ding.wav"), b"le mien").unwrap();
        assert_eq!(install_sounds(&dir), SOUNDS.len() - 1);
        assert_eq!(std::fs::read(dir.join("sounds/alpirc-ding.wav")).unwrap(), b"le mien");
        assert_eq!(std::fs::read(dir.join("sounds/alpirc-query.wav")).unwrap(), SOUNDS[1].1);
        assert_eq!(install_sounds(&dir), 0, "déjà là");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
