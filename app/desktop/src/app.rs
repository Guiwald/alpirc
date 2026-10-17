//! La boucle d'événements : fenêtre winit, affichage softbuffer, événements traduits pour le shell.

use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::Instant;

use alpirc_shell::Shell;
use alpirc_ui::{Canvas, Input, MouseButton, Point, Size};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Ime, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowId};

use crate::platform::DesktopPlatform;
use crate::{UserEvent, keys, wall_ms};

/// Intervalle maximal entre deux battements du moteur (file anti-flood, timers des scripts…).
const CORE_TICK_MS: i64 = 250;
/// Délai et distance du double-clic.
const DOUBLE_CLICK_MS: u128 = 450;

pub struct App {
    shell: Shell,
    plat: DesktopPlatform,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    proxy: EventLoopProxy<UserEvent>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    canvas: Option<Canvas>,
    mods: ModifiersState,
    /// Position du pointeur (pixels logiques).
    pointer: Point,
    last_click: Option<(Instant, Point, MouseButton, u8)>,
    next_core_tick: i64,
    started: bool,
    open_url: Option<String>,
    #[cfg(target_os = "macos")]
    menus: Option<crate::macmenu::NativeMenus>,
}

impl App {
    pub fn new(shell: Shell, proxy: EventLoopProxy<UserEvent>, open_url: Option<String>) -> App {
        App {
            shell,
            plat: DesktopPlatform::new(),
            proxy,
            surface: None,
            canvas: None,
            mods: ModifiersState::default(),
            pointer: Point::new(0, 0),
            last_click: None,
            next_core_tick: 0,
            started: false,
            open_url,
            #[cfg(target_os = "macos")]
            menus: None,
        }
    }

    fn scale(&self) -> f64 {
        self.plat.window.as_ref().map(|w| w.scale_factor()).unwrap_or(1.0)
    }

    fn create_window(&mut self, el: &ActiveEventLoop) {
        let mut attrs = Window::default_attributes()
            .with_title("AlpIRC")
            .with_inner_size(LogicalSize::new(1024.0, 720.0))
            .with_min_inner_size(LogicalSize::new(480.0, 320.0));
        if let Some(icon) = window_icon() {
            attrs = attrs.with_window_icon(Some(icon));
        }
        #[cfg(target_os = "linux")]
        {
            use winit::platform::x11::WindowAttributesExtX11;
            attrs = attrs.with_name("alpirc", "AlpIRC");
        }
        let window = match el.create_window(attrs) {
            Ok(w) => Rc::new(w),
            Err(e) => {
                eprintln!("AlpIRC : fenêtre impossible ({e})");
                el.exit();
                return;
            }
        };
        window.set_ime_allowed(true);
        let ctx = softbuffer::Context::new(window.clone()).expect("affichage softbuffer");
        let surface = softbuffer::Surface::new(&ctx, window.clone()).expect("surface softbuffer");
        self.surface = Some(surface);
        self.plat.window = Some(window);
    }

    fn redraw(&mut self) {
        let Some(window) = self.plat.window.clone() else { return };
        let size = window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else { return };
        let scale = self.scale() as f32;
        let logical = Size::new((size.width as f32 / scale).floor() as i32, (size.height as f32 / scale).floor() as i32);
        self.shell.set_viewport(logical);
        let reuse = self.canvas.as_ref().is_some_and(|c| c.width() == size.width && c.height() == size.height && (c.scale() - scale).abs() < 0.001);
        if !reuse {
            self.canvas = Some(Canvas::new(size.width, size.height, scale));
        }
        let canvas = self.canvas.as_mut().expect("canevas");
        self.shell.paint(canvas);
        let Some(surface) = self.surface.as_mut() else { return };
        if surface.resize(w, h).is_err() {
            return;
        }
        if let Ok(mut buf) = surface.buffer_mut() {
            let n = buf.len().min(canvas.pixels.len());
            buf[..n].copy_from_slice(&canvas.pixels[..n]);
            let _ = buf.present();
        }
    }

    fn to_logical(&self, x: f64, y: f64) -> Point {
        let s = self.scale();
        Point::new((x / s).floor() as i32, (y / s).floor() as i32)
    }

    fn send(&mut self, input: Input) {
        self.shell.input(input, &mut self.plat);
    }

    #[cfg(target_os = "macos")]
    fn sync_menus(&mut self) {
        if !self.plat.menus_changed {
            return;
        }
        self.plat.menus_changed = false;
        let model = self.shell.menu_model();
        let lang = self.shell.language();
        match self.menus.as_mut() {
            Some(m) => m.update(&model, lang),
            None => {
                let proxy = self.proxy.clone();
                self.menus = Some(crate::macmenu::NativeMenus::new(&model, lang, move || {
                    let _ = proxy.send_event(UserEvent::Menu);
                }));
            }
        }
    }

    fn after_events(&mut self, el: &ActiveEventLoop) {
        // Fichiers choisis dans la boîte du système.
        for (request, path) in std::mem::take(&mut self.plat.picked) {
            self.shell.file_picked(request, path, &mut self.plat);
        }
        #[cfg(target_os = "macos")]
        {
            let ids: Vec<u32> = self.menus.as_ref().map(|m| m.poll()).unwrap_or_default();
            for id in ids {
                if id == alpirc_shell::menus::ids::EXIT {
                    self.shell.request_exit(&mut self.plat);
                } else {
                    self.shell.menu_action(id, &mut self.plat);
                }
            }
            self.sync_menus();
        }
        let now = wall_ms();
        if now >= self.next_core_tick {
            let next = self.shell.tick(&mut self.plat);
            self.next_core_tick = next.unwrap_or(now + CORE_TICK_MS).min(now + CORE_TICK_MS).max(now + 10);
        }
        if let Some(t) = self.plat.tick_at {
            if now >= t {
                self.plat.tick_at = None;
                self.send(Input::Tick);
            }
        }
        if self.plat.exit {
            el.exit();
            return;
        }
        if std::mem::take(&mut self.plat.repaint) {
            if let Some(w) = &self.plat.window {
                w.request_redraw();
            }
        }
        let wake = self.plat.tick_at.map_or(self.next_core_tick, |t| t.min(self.next_core_tick));
        let wait = (wake - wall_ms()).clamp(1, CORE_TICK_MS) as u64;
        el.set_control_flow(ControlFlow::WaitUntil(Instant::now() + std::time::Duration::from_millis(wait)));
    }
}

/// L'icône de la fenêtre (Linux ; macOS prend celle du paquet).
fn window_icon() -> Option<winit::window::Icon> {
    let png = alpirc_shell::assets::app_icon_png();
    let decoder = png::Decoder::new(std::io::Cursor::new(png));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => buf[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => buf[..info.buffer_size()].chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        _ => return None,
    };
    winit::window::Icon::from_rgba(rgba, info.width, info.height).ok()
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.started {
            return;
        }
        self.started = true;
        self.create_window(el);
        self.shell.start(&mut self.plat);
        if let Some(url) = self.open_url.take() {
            self.shell.open_irc_url(&url, &mut self.plat);
        }
        self.after_events(el);
    }

    fn user_event(&mut self, el: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Net(ev) => self.shell.on_net(ev, &mut self.plat),
            UserEvent::Menu => {}
        }
        self.after_events(el);
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => self.plat.repaint = true,
            WindowEvent::CloseRequested => self.shell.request_exit(&mut self.plat),
            WindowEvent::Focused(f) => {
                self.shell.set_app_focused(f, &mut self.plat);
                self.send(if f { Input::FocusIn } else { Input::FocusOut });
            }
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = self.to_logical(position.x, position.y);
                self.send(Input::MouseMove { pos: self.pointer });
            }
            WindowEvent::CursorLeft { .. } => self.send(Input::MouseLeave),
            WindowEvent::MouseInput { state, button, .. } => {
                let b = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    winit::event::MouseButton::Middle => MouseButton::Middle,
                    winit::event::MouseButton::Back => MouseButton::Other(4),
                    winit::event::MouseButton::Forward => MouseButton::Other(5),
                    winit::event::MouseButton::Other(n) => MouseButton::Other(n),
                };
                let mods = keys::mods(self.mods);
                let pos = self.pointer;
                match state {
                    ElementState::Pressed => {
                        let clicks = match self.last_click {
                            Some((t, p, lb, n))
                                if lb == b && t.elapsed().as_millis() < DOUBLE_CLICK_MS && (p.x - pos.x).abs() <= 3 && (p.y - pos.y).abs() <= 3 =>
                            {
                                (n % 3) + 1
                            }
                            _ => 1,
                        };
                        self.last_click = Some((Instant::now(), pos, b, clicks));
                        // Ctrl+clic sur Mac = clic droit.
                        let b = if cfg!(target_os = "macos") && b == MouseButton::Left && mods.ctrl { MouseButton::Right } else { b };
                        self.send(Input::MouseDown { pos, button: b, mods, clicks });
                    }
                    ElementState::Released => self.send(Input::MouseUp { pos, button: b, mods }),
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x, y),
                    MouseScrollDelta::PixelDelta(p) => {
                        let s = self.scale();
                        ((p.x / s / 16.0) as f32, (p.y / s / 16.0) as f32)
                    }
                };
                let mods = keys::mods(self.mods);
                self.send(Input::Wheel { pos: self.pointer, dx, dy, mods });
            }
            WindowEvent::KeyboardInput { event, is_synthetic: false, .. } => {
                if event.state == ElementState::Pressed {
                    let mods = keys::mods(self.mods);
                    if let Some((key, text)) = keys::map(&event, mods) {
                        self.send(Input::Key { key, mods, text, repeat: event.repeat });
                    }
                }
            }
            WindowEvent::Ime(ime) => match ime {
                Ime::Preedit(text, cursor) => self.send(Input::ImePreedit { text, cursor }),
                Ime::Commit(text) => self.send(Input::Text(text)),
                Ime::Enabled | Ime::Disabled => {}
            },
            // Fichier lâché : envoyé par DCC au pseudo du privé actif (sinon boîte « Envoyer un fichier »).
            WindowEvent::DroppedFile(path) => self.shell.file_dropped(path, &mut self.plat),
            _ => {}
        }
        self.after_events(el);
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        self.after_events(el);
    }
}
