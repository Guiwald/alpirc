//! La boucle de l'appli du téléphone, commune à Android et au simulateur : fenêtre winit, affichage softbuffer
//! du [`Shell`] en disposition [`LayoutKind::Mobile`], toucher (ou souris) transmis au shell qui en fait des
//! gestes, événements du réseau et du système, service de connexion, champ de saisie natif.

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Instant;

use alpirc_core::ConnState;
use alpirc_net::{Net, NetEvent};
use alpirc_shell::tr::t;
use alpirc_shell::{LayoutKind, Shell, ShellConfig, ShellPlatform};
use alpirc_ui::{Canvas, Cursor, Input, MouseButton, Platform, Point, Rect, Size};
use winit::application::ApplicationHandler;
use winit::event::{ElementState, Ime, MouseScrollDelta, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowId};

use crate::background::BackgroundTimer;
use crate::host::{Host, Prefs, ServiceState, SysEvent, auto_scale, parse_event};
use crate::keys;

/// Intervalle maximal entre deux battements du moteur (file anti-flood, timers des scripts…).
const CORE_TICK_MS: i64 = 250;

/// Événements envoyés à la boucle depuis d'autres fils.
#[derive(Debug)]
pub enum UserEvent {
    Net(NetEvent),
    /// Événement texte du code Java (voir [`parse_event`]).
    Sys(String),
}

/// Heure réelle en millisecondes depuis 1970.
pub fn wall_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Les services de la plateforme pour le shell, au-dessus de l'hôte (Android ou simulateur).
pub struct Plat {
    pub host: Box<dyn Host>,
    pub repaint: bool,
    pub tick_at: Option<i64>,
    pub exit: bool,
    pub prefs: Prefs,
    /// Taille de l'interface en vigueur (pixels du téléphone par pixel logique).
    pub scale: f32,
    pub scale_changed: bool,
    data_dir: PathBuf,
    /// Demandes de fichier en cours : jeton donné au sélecteur du système → numéro de demande du shell.
    picks: HashMap<u32, u64>,
    next_pick: u32,
    /// Réponses à rendre au shell ([`Shell::file_picked`]) au prochain tour de boucle.
    answers: Vec<(u64, Option<PathBuf>)>,
}

impl Platform for Plat {
    fn clipboard_get(&mut self) -> Option<String> {
        self.host.clipboard_get()
    }
    fn clipboard_set(&mut self, text: &str) {
        self.host.clipboard_set(text);
    }
    fn now_ms(&self) -> i64 {
        wall_ms()
    }
    fn set_cursor(&mut self, _c: Cursor) {}
    fn set_ime_area(&mut self, _rect: Rect) {}
    fn request_repaint(&mut self) {
        self.repaint = true;
    }
    fn schedule_tick(&mut self, at_ms: i64) {
        self.tick_at = Some(self.tick_at.map_or(at_ms, |t| t.min(at_ms)));
    }
}

impl ShellPlatform for Plat {
    fn notify(&mut self, title: &str, body: &str) {
        self.host.notify(title, body);
    }
    /// Bip : `sounds/beep.wav` s'il existe, sinon le bip du système ; et une courte vibration si elle est
    /// choisie (menu ⋮, Téléphone).
    fn beep(&mut self) {
        let wav = self.data_dir.join("sounds").join("beep.wav");
        if wav.is_file() {
            self.host.play_sound(&wav);
        } else {
            self.host.beep_tone();
        }
        if self.prefs.vibrate {
            self.host.vibrate(40);
        }
    }
    /// Son d'un événement (`/splay`, sons des réglages) : wav, mp3 ou ogg, au volume des notifications.
    fn play_sound(&mut self, path: &std::path::Path) {
        if path.is_file() {
            self.host.play_sound(path);
        } else {
            log::warn!("son introuvable : {}", path.display());
        }
    }
    /// `/splay stop`.
    fn stop_sound(&mut self) {
        self.host.stop_sound();
    }
    fn flash(&mut self) {}
    fn open_url(&mut self, url: &str) {
        let url = if url.starts_with("www.") { format!("http://{url}") } else { url.to_string() };
        let ok = ["http://", "https://", "ftp://", "mailto:", "irc://", "ircs://"].iter().any(|s| url.to_ascii_lowercase().starts_with(s));
        if ok {
            self.host.open_url(&url);
        }
    }
    fn set_title(&mut self, _title: &str) {}
    fn exit(&mut self) {
        self.exit = true;
    }
    fn downloads_dir(&self) -> Option<PathBuf> {
        self.host.downloads_dir()
    }
    /// Ouvrir : le sélecteur de documents du système (copie locale). Enregistrer : directement dans le dossier
    /// des fichiers reçus, sous le nom proposé.
    fn pick_file(&mut self, request: u64, save: bool, suggested_name: Option<&str>) {
        if save {
            let path = self.host.downloads_dir().map(|d| {
                let _ = std::fs::create_dir_all(&d);
                let name = suggested_name.and_then(|n| Path::new(n).file_name()).map(|n| n.to_owned()).unwrap_or_else(|| "alpirc.txt".into());
                d.join(name)
            });
            self.answers.push((request, path));
        } else {
            self.next_pick = self.next_pick.wrapping_add(1);
            let token = self.next_pick;
            if self.host.pick_file(token) {
                self.picks.insert(token, request);
            } else {
                self.answers.push((request, None));
            }
        }
        self.repaint = true;
    }
    fn open_path(&mut self, path: &Path) {
        self.host.open_path(path);
    }
    fn ui_scale(&self) -> Option<f32> {
        Some(self.scale)
    }
    fn set_ui_scale(&mut self, scale: f32) {
        if (scale - self.scale).abs() > 0.001 {
            self.scale = scale;
            self.prefs.scale = Some(scale);
            self.prefs.save(&self.data_dir);
            self.scale_changed = true;
            self.repaint = true;
        }
    }
}

/// Simulateur en pixels du téléphone (`ALPIRC_SIM_PHYSICAL=1`) : fenêtre de 1080×2400 dessinée à la taille de
/// l'interface, exactement comme sur le Pixel 8 (pour des captures fidèles ; il faut un écran assez haut).
fn sim_physical() -> bool {
    std::env::var_os("ALPIRC_SIM_PHYSICAL").is_some_and(|v| v != "0" && !v.is_empty())
}

/// Le doigt suivi (un seul : les autres sont ignorés).
#[derive(Clone, Copy, Debug)]
struct Finger {
    id: u64,
    pos: Point,
}

/// L'appli du téléphone.
pub struct MobileApp {
    shell: Shell,
    plat: Plat,
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
    canvas: Option<Canvas>,
    mods: ModifiersState,
    /// Souris (simulateur) : position en pixels logiques, bouton gauche enfoncé.
    pointer: Point,
    finger: Option<Finger>,
    started: bool,
    next_core_tick: i64,
    /// Événements du système arrivés avant le démarrage.
    pending: Vec<SysEvent>,
    /// Dernier état envoyé au champ natif, au clavier, au service.
    input_state: Option<(bool, String)>,
    keyboard: bool,
    service: Option<ServiceState>,
    foreground: bool,
    service_running: bool,
    /// Déconnexion après N minutes en arrière-plan (ADR 0006).
    background: BackgroundTimer,
    /// La déconnexion en arrière-plan a eu lieu (minutes) : la notification le dit jusqu'au retour.
    bg_note: Option<u32>,
    /// La notification du service montre ce mot (elle n'est plus celle d'un service au premier plan).
    note_shown: bool,
    /// Le shell doit relire les marges.
    insets_dirty: bool,
}

/// Démarre le shell : réglages dans le dossier des données, heure locale et langue du système (lues avant le
/// démarrage des fils du réseau), réseau dont les événements reviennent par `proxy`.
pub fn build(host: Box<dyn Host>, proxy: EventLoopProxy<UserEvent>) -> MobileApp {
    let now = wall_ms();
    let utc_offset = host.utc_offset(now);
    let locale = host.locale();
    let data_dir = host.data_dir();
    let _ = std::fs::create_dir_all(&data_dir);
    let prefs = Prefs::load(&data_dir);
    let density = host.density().unwrap_or(2.625);
    let scale = if host.simulated_screen().is_some() {
        // Simulateur : 1 pixel logique = 1 dp par défaut (412×915), ou la taille demandée.
        std::env::var("ALPIRC_SIM_SCALE").ok().and_then(|s| s.parse::<f32>().ok()).filter(|s| (1.0..=8.0).contains(s)).or(prefs.scale).unwrap_or(density)
    } else {
        prefs.scale.unwrap_or_else(|| auto_scale(density))
    };
    let net = Net::new(Box::new(move |ev| {
        let _ = proxy.send_event(UserEvent::Net(ev));
    }));
    let cfg = ShellConfig { settings_path: data_dir.join("settings.toml"), data_dir: data_dir.clone(), layout: LayoutKind::Mobile, utc_offset, locale, now_ms: now };
    let mut shell = Shell::new(cfg, net);
    let fallbacks = host.font_fallbacks();
    if !fallbacks.is_empty() {
        let n = shell.res.fonts.load_font_files(&fallbacks);
        log::info!("{n} polices de repli du système");
    }
    shell.set_native_input(host.native_input());
    shell.set_phone_prefs(prefs.background_disconnect_min, prefs.vibrate);
    let background = BackgroundTimer::new(prefs.background_disconnect_min);
    let plat = Plat {
        host,
        repaint: true,
        tick_at: None,
        exit: false,
        prefs,
        scale,
        scale_changed: false,
        data_dir,
        picks: HashMap::new(),
        next_pick: 0,
        answers: Vec::new(),
    };
    MobileApp {
        shell,
        plat,
        window: None,
        surface: None,
        canvas: None,
        mods: ModifiersState::default(),
        pointer: Point::new(0, 0),
        finger: None,
        started: false,
        next_core_tick: 0,
        pending: Vec::new(),
        input_state: None,
        keyboard: false,
        service: None,
        foreground: true,
        service_running: false,
        background,
        bg_note: None,
        note_shown: false,
        insets_dirty: true,
    }
}

impl MobileApp {
    /// Échelle du canevas : sur le téléphone, la taille de l'interface ; dans le simulateur, celle de l'écran de
    /// l'ordinateur (la fenêtre a déjà la taille de l'écran du téléphone divisée par la taille de l'interface).
    fn canvas_scale(&self) -> f32 {
        if self.plat.host.simulated_screen().is_some() && !sim_physical() {
            self.window.as_ref().map(|w| w.scale_factor() as f32).unwrap_or(1.0)
        } else {
            self.plat.scale
        }
    }

    /// Taille logique de la fenêtre du simulateur pour la taille d'interface en vigueur.
    fn sim_logical_size(&self) -> Option<winit::dpi::Size> {
        let (w, h) = self.plat.host.simulated_screen()?;
        if sim_physical() {
            return Some(winit::dpi::PhysicalSize::new(w, h).into());
        }
        let s = self.plat.scale as f64;
        Some(winit::dpi::LogicalSize::new((w as f64 / s).round(), (h as f64 / s).round()).into())
    }

    fn create_window(&mut self, el: &ActiveEventLoop) {
        if self.window.is_none() {
            let mut attrs = Window::default_attributes().with_title("AlpIRC");
            if let Some(size) = self.sim_logical_size() {
                attrs = attrs.with_inner_size(size).with_resizable(false);
            }
            match el.create_window(attrs) {
                Ok(w) => {
                    if self.plat.host.simulated_screen().is_some() {
                        w.set_ime_allowed(true);
                    }
                    self.window = Some(Rc::new(w));
                }
                Err(e) => {
                    log::error!("fenêtre impossible : {e}");
                    return;
                }
            }
        }
        let Some(window) = self.window.clone() else { return };
        self.surface = softbuffer::Context::new(window.clone()).ok().and_then(|ctx| softbuffer::Surface::new(&ctx, window.clone()).ok());
        if self.surface.is_none() {
            log::error!("surface softbuffer impossible");
        }
        self.canvas = None;
        self.plat.repaint = true;
        window.request_redraw();
    }

    fn update_insets(&mut self) {
        let [_, top, _, bottom] = self.plat.host.insets();
        let s = self.plat.scale.max(0.5);
        self.shell.set_insets((top as f32 / s).ceil() as i32, (bottom as f32 / s).ceil() as i32);
        self.insets_dirty = false;
    }

    fn redraw(&mut self) {
        let Some(window) = self.window.clone() else { return };
        if self.surface.is_none() {
            return;
        }
        let size = window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else { return };
        if self.insets_dirty {
            self.update_insets();
        }
        let scale = self.canvas_scale();
        let logical = Size::new((size.width as f32 / scale).floor() as i32, (size.height as f32 / scale).floor() as i32);
        self.shell.set_viewport(logical);
        let reuse = self.canvas.as_ref().is_some_and(|c| c.width() == size.width && c.height() == size.height && (c.scale() - scale).abs() < 0.001);
        if !reuse {
            self.canvas = Some(Canvas::new(size.width, size.height, scale));
        }
        let Some(canvas) = self.canvas.as_mut() else { return };
        self.shell.paint(canvas);
        let Some(surface) = self.surface.as_mut() else { return };
        if surface.resize(w, h).is_err() {
            return;
        }
        if let Ok(mut buf) = surface.buffer_mut() {
            let n = buf.len().min(canvas.pixels.len());
            buf[..n].copy_from_slice(&canvas.pixels[..n]);
            if let Err(e) = buf.present() {
                log::warn!("affichage : {e}");
            }
        }
    }

    fn to_logical(&self, x: f64, y: f64) -> Point {
        let s = self.canvas_scale() as f64;
        Point::new((x / s).floor() as i32, (y / s).floor() as i32)
    }

    fn send(&mut self, input: Input) {
        self.shell.input(input, &mut self.plat);
    }

    /// Touches du simulateur pour ce qui n'a pas de bouton sur ordinateur : F2 = bouton Historique du champ natif,
    /// F9 / F10 = l'appli passe derrière / revient devant (déconnexion en arrière-plan).
    fn sim_key(&mut self, key: alpirc_ui::Key) -> bool {
        match key {
            alpirc_ui::Key::F(2) => self.system_event(SysEvent::HistoryMenu),
            alpirc_ui::Key::F(9) => self.system_event(SysEvent::Foreground(false)),
            alpirc_ui::Key::F(10) => self.system_event(SysEvent::Foreground(true)),
            _ => return false,
        }
        true
    }

    /// Bouton Retour : ferme ce qui est ouvert, sinon l'appli passe derrière (connexions gardées, ADR 0006).
    fn back(&mut self) {
        if !self.shell.mobile_back(&mut self.plat) && !self.plat.host.to_back() {
            log::info!("retour : rien à fermer");
        }
    }

    fn system_event(&mut self, e: SysEvent) {
        if !self.started && !matches!(e, SysEvent::Insets | SysEvent::Log(_)) {
            self.pending.push(e);
            return;
        }
        match e {
            SysEvent::Insets => {
                self.insets_dirty = true;
                self.plat.repaint = true;
            }
            SysEvent::Send(text) => self.shell.mobile_submit(&text, &mut self.plat),
            SysEvent::Tab(text) => {
                if let Some(n) = self.shell.mobile_complete(&text) {
                    self.plat.host.set_input_text(&n);
                }
            }
            SysEvent::Back => self.back(),
            SysEvent::Url(u) => self.shell.open_irc_url(&u, &mut self.plat),
            SysEvent::Foreground(fg) => {
                self.foreground = fg;
                self.shell.set_app_focused(fg, &mut self.plat);
                self.insets_dirty = true;
                if fg {
                    // Retour avant le délai : il est annulé ; après : reconnexion aux mêmes serveurs.
                    self.bg_note = None;
                    if self.background.foreground() {
                        let n = self.shell.mobile_background_reconnect(&mut self.plat);
                        log::info!("retour au premier plan : {n} reconnexion(s)");
                    }
                } else {
                    self.background.background(wall_ms());
                }
            }
            SysEvent::History { older, text } => {
                if let Some(t) = self.shell.mobile_history_step(older, &text) {
                    self.plat.host.set_input_text(&t);
                }
            }
            SysEvent::HistoryMenu => self.shell.mobile_history_menu(&mut self.plat),
            SysEvent::Quit => self.shell.exit_now(&mut self.plat),
            SysEvent::Destroyed { config } => {
                // La boucle doit rendre la main pour que l'activité se termine, et ne pourra pas être recréée dans
                // ce processus : on quitte proprement (QUIT, réglages enregistrés).
                log::info!("fin de l'activité (configuration : {config})");
                self.shell.exit_now(&mut self.plat);
            }
            SysEvent::Picked { token, path } => match self.plat.picks.remove(&token) {
                Some(request) => self.shell.file_picked(request, path, &mut self.plat),
                None => log::warn!("réponse du sélecteur sans demande : {token}"),
            },
            SysEvent::Log(m) => log::info!("java : {m}"),
        }
    }

    /// Champ de saisie natif, clavier du système pour les boîtes de dialogue, service de connexion.
    fn sync_system(&mut self) {
        if self.plat.host.native_input() {
            let want = (self.shell.mobile_native_input_visible(), self.shell.mobile_input_hint());
            if self.input_state.as_ref() != Some(&want) {
                let lang = self.shell.language();
                if self.input_state.is_none() {
                    self.plat.host.set_input_labels(&t(lang, "mobile.send"), &t(lang, "mobile.tab"), &t(lang, "mobile.history"));
                }
                self.plat.host.set_input(want.0, &want.1);
                self.input_state = Some(want);
                self.insets_dirty = true;
            }
            if let Some(text) = self.shell.take_native_input_text() {
                self.plat.host.set_input_text(&text);
            }
            let kb = self.shell.mobile_text_focus();
            if kb != self.keyboard {
                self.keyboard = kb;
                self.plat.host.show_keyboard(kb);
            }
        }
        // Service de connexion : démarré dès qu'une connexion s'ouvre ; arrêté quand il n'y en a plus, appli au
        // premier plan (en arrière-plan, il reste pour laisser les reconnexions se faire).
        let infos: Vec<_> = self.shell.client.conns().into_iter().filter_map(|c| self.shell.client.conn_info(c)).collect();
        let active = infos.iter().filter(|i| i.state != ConnState::Disconnected).count();
        let connected = infos.iter().filter(|i| i.state == ConnState::Connected).count();
        let lang = self.shell.language();
        let text = match connected {
            0 if active > 0 => t(lang, "mobile.service.connecting"),
            0 => t(lang, "mobile.service.waiting"),
            1 => t(lang, "mobile.service.one"),
            n => alpirc_shell::tr::tf(lang, "mobile.service.many", &[("n", &n.to_string())]),
        };
        // Après la déconnexion en arrière-plan, le service s'arrête et sa notification le dit.
        let keep = active > 0 || (self.service_running && !self.foreground && self.bg_note.is_none());
        let (text, note) = match self.bg_note {
            Some(n) if active == 0 => (alpirc_shell::tr::tf(lang, "mobile.bg.done", &[("n", &n.to_string())]), true),
            _ => (text, false),
        };
        let st = ServiceState {
            active: if keep { active.max(1) } else { 0 },
            title: "AlpIRC".to_string(),
            text,
            quit: t(lang, "mobile.service.quit"),
            channel: t(lang, "mobile.service.channel"),
            notify_channel: t(lang, "mobile.notify.channel"),
            note,
        };
        if self.service.as_ref() != Some(&st) {
            if st.active > 0 || self.service_running || self.note_shown || st.note {
                self.plat.host.service(&st);
            }
            self.service_running = st.active > 0;
            self.note_shown = st.note;
            self.service = Some(st);
        }
    }

    fn after_events(&mut self, el: &ActiveEventLoop) {
        let now = wall_ms();
        if self.started && now >= self.next_core_tick {
            let next = self.shell.tick(&mut self.plat);
            self.next_core_tick = next.unwrap_or(now + CORE_TICK_MS).min(now + CORE_TICK_MS).max(now + 10);
        }
        if let Some(t) = self.plat.tick_at {
            if now >= t {
                self.plat.tick_at = None;
                self.send(Input::Tick);
            }
        }
        if self.started {
            for (request, path) in std::mem::take(&mut self.plat.answers) {
                self.shell.file_picked(request, path, &mut self.plat);
            }
            if let Some((minutes, vibrate)) = self.shell.take_phone_prefs_change() {
                self.plat.prefs.background_disconnect_min = minutes;
                self.plat.prefs.vibrate = vibrate;
                self.plat.prefs.save(&self.plat.data_dir);
                self.background.set_minutes(minutes);
            }
            if self.background.poll(now) {
                let n = self.shell.mobile_background_disconnect(&mut self.plat);
                log::info!("{} min en arrière-plan : {n} connexion(s) fermée(s)", self.background.minutes());
                self.bg_note = Some(self.background.minutes());
            }
            self.sync_system();
        }
        if self.plat.exit {
            self.plat.host.service(&ServiceState::default());
            self.plat.host.exit_app();
            el.exit();
            return;
        }
        if std::mem::take(&mut self.plat.scale_changed) {
            self.insets_dirty = true;
            self.canvas = None;
            if let (Some(w), Some(size)) = (&self.window, self.sim_logical_size()) {
                let _ = w.request_inner_size(size);
            }
        }
        if self.insets_dirty {
            self.plat.repaint = true;
        }
        if std::mem::take(&mut self.plat.repaint) {
            if let Some(w) = &self.window {
                w.request_redraw();
            }
        }
        let wake = self.plat.tick_at.map_or(self.next_core_tick, |t| t.min(self.next_core_tick));
        let wait = (wake - wall_ms()).clamp(1, CORE_TICK_MS) as u64;
        el.set_control_flow(ControlFlow::WaitUntil(Instant::now() + std::time::Duration::from_millis(wait)));
    }
}

impl MobileApp {
    /// Une panique dans le traitement d'un événement est journalisée (crochet de panique) et l'appli continue,
    /// plutôt que de fermer l'activité.
    fn guarded(&mut self, el: &ActiveEventLoop, f: impl FnOnce(&mut MobileApp, &ActiveEventLoop)) {
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(self, el))).is_err() {
            log::error!("panique interceptée : AlpIRC continue");
            self.plat.repaint = true;
        }
    }
}

impl ApplicationHandler<UserEvent> for MobileApp {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        self.guarded(el, |s, el| s.on_resumed(el));
    }

    fn suspended(&mut self, _el: &ActiveEventLoop) {
        // La surface du système disparaît ; les connexions continuent (service de connexion).
        self.surface = None;
    }

    fn user_event(&mut self, el: &ActiveEventLoop, event: UserEvent) {
        self.guarded(el, |s, el| s.on_user_event(el, event));
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        self.guarded(el, |s, el| s.on_window_event(el, event));
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        self.guarded(el, |s, el| s.after_events(el));
    }

    fn exiting(&mut self, _el: &ActiveEventLoop) {
        log::info!("fin de la boucle");
    }
}

impl MobileApp {
    fn on_resumed(&mut self, el: &ActiveEventLoop) {
        self.create_window(el);
        self.insets_dirty = true;
        if !self.started {
            self.started = true;
            self.shell.start(&mut self.plat);
            for e in std::mem::take(&mut self.pending) {
                self.system_event(e);
            }
        }
        self.after_events(el);
    }

    fn on_user_event(&mut self, el: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Net(ev) => self.shell.on_net(ev, &mut self.plat),
            UserEvent::Sys(raw) => match parse_event(&raw) {
                Some(e) => self.system_event(e),
                None => log::warn!("événement du système inconnu : {raw:?}"),
            },
        }
        self.after_events(el);
    }

    fn on_window_event(&mut self, el: &ActiveEventLoop, event: WindowEvent) {
        match event {
            WindowEvent::RedrawRequested => self.redraw(),
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                self.insets_dirty = true;
                self.plat.repaint = true;
            }
            WindowEvent::CloseRequested => self.shell.request_exit(&mut self.plat),
            // Simulateur : la fenêtre de l'ordinateur prend ou perd le focus. Sur Android, le premier plan vient
            // de l'activité (« fg ») : le champ natif prend le focus de la fenêtre sans que l'appli soit derrière.
            WindowEvent::Focused(f) if self.plat.host.simulated_screen().is_some() => {
                self.shell.set_app_focused(f, &mut self.plat);
            }
            WindowEvent::ModifiersChanged(m) => self.mods = m.state(),
            // Le doigt.
            WindowEvent::Touch(tch) => {
                let pos = self.to_logical(tch.location.x, tch.location.y);
                match tch.phase {
                    TouchPhase::Started => {
                        if self.finger.is_none() {
                            self.finger = Some(Finger { id: tch.id, pos });
                            self.send(Input::MouseDown { pos, button: MouseButton::Left, mods: Default::default(), clicks: 1 });
                        }
                    }
                    TouchPhase::Moved => {
                        if let Some(f) = self.finger.as_mut().filter(|f| f.id == tch.id) {
                            if f.pos != pos {
                                f.pos = pos;
                                self.send(Input::MouseMove { pos });
                            }
                        }
                    }
                    TouchPhase::Ended => {
                        if self.finger.is_some_and(|f| f.id == tch.id) {
                            self.finger = None;
                            self.send(Input::MouseUp { pos, button: MouseButton::Left, mods: Default::default() });
                        }
                    }
                    TouchPhase::Cancelled => {
                        if self.finger.is_some_and(|f| f.id == tch.id) {
                            self.finger = None;
                            self.send(Input::MouseLeave);
                        }
                    }
                }
            }
            // La souris du simulateur fait office de doigt (bouton droit : appui long).
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer = self.to_logical(position.x, position.y);
                self.send(Input::MouseMove { pos: self.pointer });
            }
            WindowEvent::CursorLeft { .. } => self.send(Input::MouseLeave),
            WindowEvent::MouseInput { state, button, .. } => {
                let b = match button {
                    winit::event::MouseButton::Left => MouseButton::Left,
                    winit::event::MouseButton::Right => MouseButton::Right,
                    _ => return,
                };
                let pos = self.pointer;
                let mods = keys::mods(self.mods);
                match state {
                    ElementState::Pressed => self.send(Input::MouseDown { pos, button: b, mods, clicks: 1 }),
                    ElementState::Released => self.send(Input::MouseUp { pos, button: b, mods }),
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x, y),
                    MouseScrollDelta::PixelDelta(p) => ((p.x / 16.0) as f32, (p.y / 16.0) as f32),
                };
                let mods = keys::mods(self.mods);
                self.send(Input::Wheel { pos: self.pointer, dx, dy, mods });
            }
            WindowEvent::KeyboardInput { event, is_synthetic: false, .. } => {
                if event.state == ElementState::Pressed {
                    if keys::is_back(&event) {
                        self.back();
                    } else {
                        let mods = keys::mods(self.mods);
                        if let Some((key, text)) = keys::map(&event, mods) {
                            if self.plat.host.simulated_screen().is_some() && self.sim_key(key) {
                                // Touche du simulateur (F2, F9, F10).
                            } else if key == alpirc_ui::Key::Escape && self.shell.mobile_back(&mut self.plat) {
                                // Échap du simulateur : comme Retour.
                            } else {
                                self.send(Input::Key { key, mods, text, repeat: event.repeat });
                            }
                        }
                    }
                }
            }
            WindowEvent::Ime(ime) => match ime {
                Ime::Preedit(text, cursor) => self.send(Input::ImePreedit { text, cursor }),
                Ime::Commit(text) => self.send(Input::Text(text)),
                Ime::Enabled | Ime::Disabled => {}
            },
            _ => {}
        }
        self.after_events(el);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    /// Un hôte qui note les sons demandés.
    struct Rec(Rc<RefCell<Vec<String>>>, PathBuf);

    impl Host for Rec {
        fn clipboard_get(&mut self) -> Option<String> {
            None
        }
        fn clipboard_set(&mut self, _text: &str) {}
        fn notify(&mut self, _title: &str, _body: &str) {}
        fn vibrate(&mut self, _ms: i32) {}
        fn play_sound(&mut self, path: &Path) {
            self.0.borrow_mut().push(format!("play {}", path.display()));
        }
        fn stop_sound(&mut self) {
            self.0.borrow_mut().push("stop".into());
        }
        fn open_url(&mut self, _url: &str) {}
        fn data_dir(&self) -> PathBuf {
            self.1.clone()
        }
        fn utc_offset(&self, _now_ms: i64) -> i32 {
            0
        }
        fn locale(&self) -> Option<String> {
            None
        }
    }

    #[test]
    fn splay_stop_reaches_the_phone() {
        let dir = std::env::temp_dir().join(format!("alpirc-android-son-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let wav = dir.join("ding.wav");
        std::fs::write(&wav, b"RIFF").unwrap();
        let log = Rc::new(RefCell::new(Vec::new()));
        let mut plat = Plat {
            host: Box::new(Rec(log.clone(), dir.clone())),
            repaint: false,
            tick_at: None,
            exit: false,
            prefs: Prefs::default(),
            scale: 3.0,
            scale_changed: false,
            data_dir: dir.clone(),
            picks: HashMap::new(),
            next_pick: 0,
            answers: Vec::new(),
        };
        ShellPlatform::play_sound(&mut plat, &wav);
        ShellPlatform::stop_sound(&mut plat);
        assert_eq!(*log.borrow(), vec![format!("play {}", wav.display()), "stop".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
