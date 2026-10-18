//! Les services du système pour l'application : presse-papiers, pointeur, notifications, son, liens, choix de
//! fichiers, dossier des téléchargements.

use std::path::{Path, PathBuf};
use std::rc::Rc;

use alpirc_shell::ShellPlatform;
use alpirc_ui::{Cursor, Platform, Rect};
use winit::event_loop::EventLoopProxy;
use winit::window::{CursorIcon, UserAttentionType, Window};

use crate::UserEvent;

pub struct DesktopPlatform {
    pub window: Option<Rc<Window>>,
    clipboard: Option<arboard::Clipboard>,
    pub repaint: bool,
    pub tick_at: Option<i64>,
    pub exit: bool,
    cursor: Cursor,
    /// Zone de saisie pour la méthode de saisie (pixels logiques).
    pub ime_area: Option<Rect>,
    pub native_menus: bool,
    pub menus_changed: bool,
    /// Fichiers choisis (`pick_file`), rendus au shell par la boucle d'événements (`Shell::file_picked`).
    pub picked: Vec<(u64, Option<PathBuf>)>,
    sound: crate::sound::Sound,
    #[cfg(target_os = "macos")]
    notifier: crate::macnotify::Notifier,
}

impl DesktopPlatform {
    #[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> DesktopPlatform {
        DesktopPlatform {
            window: None,
            clipboard: arboard::Clipboard::new().ok(),
            repaint: true,
            tick_at: None,
            exit: false,
            cursor: Cursor::Default,
            ime_area: None,
            native_menus: cfg!(target_os = "macos"),
            menus_changed: true,
            picked: Vec::new(),
            sound: crate::sound::Sound::new(),
            #[cfg(target_os = "macos")]
            notifier: crate::macnotify::Notifier::new(proxy),
        }
    }
}

fn icon(c: Cursor) -> CursorIcon {
    match c {
        Cursor::Default => CursorIcon::Default,
        Cursor::Text => CursorIcon::Text,
        Cursor::Hand => CursorIcon::Pointer,
        Cursor::ResizeEW => CursorIcon::EwResize,
        Cursor::ResizeNS => CursorIcon::NsResize,
        Cursor::ResizeNWSE => CursorIcon::NwseResize,
        Cursor::ResizeNESW => CursorIcon::NeswResize,
        Cursor::Move => CursorIcon::Move,
    }
}

impl Platform for DesktopPlatform {
    fn clipboard_get(&mut self) -> Option<String> {
        self.clipboard.as_mut()?.get_text().ok()
    }

    fn clipboard_set(&mut self, text: &str) {
        if let Some(c) = self.clipboard.as_mut() {
            let _ = c.set_text(text.to_string());
        }
    }

    fn now_ms(&self) -> i64 {
        crate::wall_ms()
    }

    fn set_cursor(&mut self, c: Cursor) {
        if c != self.cursor {
            self.cursor = c;
            if let Some(w) = &self.window {
                w.set_cursor(icon(c));
            }
        }
    }

    fn set_ime_area(&mut self, rect: Rect) {
        if self.ime_area != Some(rect) {
            self.ime_area = Some(rect);
            if let Some(w) = &self.window {
                w.set_ime_cursor_area(
                    winit::dpi::LogicalPosition::new(rect.x as f64, rect.y as f64),
                    winit::dpi::LogicalSize::new(rect.w.max(1) as f64, rect.h.max(1) as f64),
                );
            }
        }
    }

    fn request_repaint(&mut self) {
        self.repaint = true;
    }

    fn schedule_tick(&mut self, at_ms: i64) {
        self.tick_at = Some(self.tick_at.map_or(at_ms, |t| t.min(at_ms)));
    }
}

impl ShellPlatform for DesktopPlatform {
    fn notify(&mut self, title: &str, body: &str) {
        // macOS : centre de notifications du système depuis AlpIRC.app, sinon osascript (voir macnotify.rs).
        #[cfg(target_os = "macos")]
        self.notifier.notify(title, body);
        #[cfg(target_os = "linux")]
        {
            let (title, body) = (title.to_string(), body.to_string());
            std::thread::spawn(move || {
                let _ = notify_rust::Notification::new().appname("AlpIRC").summary(&title).body(&body).icon("alpirc").show();
            });
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let _ = (title, body);
    }

    /// Bip : celui du système sur Mac ; ailleurs, une note courte synthétisée (pas de bip du système sous Linux).
    fn beep(&mut self) {
        #[cfg(target_os = "macos")]
        objc2_app_kit::NSBeep();
        #[cfg(not(target_os = "macos"))]
        self.sound.beep();
    }

    /// Joue un son (wav, mp3, ogg, flac) sans attendre ; un nouveau son remplace celui qui joue.
    fn play_sound(&mut self, path: &Path) {
        log::info!("son : {}", path.display());
        self.sound.play_file(path);
    }

    fn flash(&mut self) {
        if let Some(w) = &self.window {
            w.request_user_attention(Some(UserAttentionType::Informational));
        }
    }

    fn open_url(&mut self, url: &str) {
        let url = if url.starts_with("www.") { format!("http://{url}") } else { url.to_string() };
        if !(url.starts_with("http://") || url.starts_with("https://") || url.starts_with("ftp://")) {
            return;
        }
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(&url).spawn();
        #[cfg(target_os = "linux")]
        let _ = std::process::Command::new("xdg-open").arg(&url).spawn();
    }

    fn set_title(&mut self, title: &str) {
        if let Some(w) = &self.window {
            w.set_title(title);
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    fn native_menubar(&self) -> bool {
        self.native_menus
    }

    fn menus_changed(&mut self) {
        self.menus_changed = true;
    }

    /// Boîte de choix du système (bloquante, dans le fil de l'interface) ; la réponse attend dans `picked`.
    fn pick_file(&mut self, request: u64, save: bool, suggested_name: Option<&str>) {
        #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
        let path = {
            let mut d = rfd::FileDialog::new();
            if let Some(w) = &self.window {
                d = d.set_parent(w.as_ref());
            }
            if let Some(n) = suggested_name {
                d = d.set_file_name(n);
            }
            if save { d.save_file() } else { d.pick_file() }
        };
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        let path = {
            let _ = (save, suggested_name);
            None
        };
        self.picked.push((request, path));
        self.repaint = true;
    }

    /// `Téléchargements/AlpIRC` (créé à la première réception).
    fn downloads_dir(&self) -> Option<PathBuf> {
        let dirs = directories::UserDirs::new()?;
        dirs.download_dir().map(|d| d.join("AlpIRC"))
    }

    /// Ouvre un dossier (ou un fichier) dans le Finder ou le gestionnaire de fichiers.
    fn open_path(&mut self, path: &Path) {
        if !path.exists() {
            return;
        }
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(path).spawn();
        #[cfg(target_os = "linux")]
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}
