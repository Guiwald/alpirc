//! Les services du système pour l'application : presse-papiers, pointeur, notifications, son, liens.

use std::path::Path;
use std::rc::Rc;

use alpirc_shell::ShellPlatform;
use alpirc_ui::{Cursor, Platform, Rect};
use winit::window::{CursorIcon, UserAttentionType, Window};

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
}

impl DesktopPlatform {
    pub fn new() -> DesktopPlatform {
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

/// Échappe une chaîne pour AppleScript.
#[cfg(target_os = "macos")]
fn applescript_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

impl ShellPlatform for DesktopPlatform {
    fn notify(&mut self, title: &str, body: &str) {
        let (title, body) = (title.to_string(), body.to_string());
        #[cfg(target_os = "macos")]
        {
            let script = format!("display notification {} with title {}", applescript_string(&body), applescript_string(&title));
            let _ = std::process::Command::new("osascript").args(["-e", &script]).spawn();
        }
        #[cfg(target_os = "linux")]
        {
            std::thread::spawn(move || {
                let _ = notify_rust::Notification::new().appname("AlpIRC").summary(&title).body(&body).icon("alpirc").show();
            });
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let _ = (title, body);
    }

    fn beep(&mut self) {
        #[cfg(target_os = "macos")]
        objc2_app_kit::NSBeep();
    }

    fn play_sound(&mut self, path: &Path) {
        // Lecture des sons : arrive avec les sons d'événements ; en attendant, le bip du système.
        log::info!("son demandé : {}", path.display());
        self.beep();
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
}
