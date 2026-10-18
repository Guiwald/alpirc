//! AlpIRC — le client IRC des années 2000, refait en Rust : application de bureau (macOS, Linux).
//!
//! winit pour la fenêtre et les événements, softbuffer pour l'affichage ; tout le reste (moteur, réseau,
//! interface) est dans `alpirc-shell`.

mod app;
#[cfg(target_os = "linux")]
mod instance;
mod keys;
#[cfg(target_os = "macos")]
mod macevents;
#[cfg(target_os = "macos")]
mod macmenu;
#[cfg(target_os = "macos")]
mod macnotify;
mod platform;
mod sound;

use std::path::PathBuf;

use alpirc_net::{Net, NetEvent};
use alpirc_shell::{LayoutKind, Shell, ShellConfig};
use winit::event_loop::EventLoop;

/// Événements envoyés à la boucle depuis d'autres fils.
#[derive(Debug)]
pub enum UserEvent {
    Net(NetEvent),
    /// Un menu natif a été choisi (macOS).
    Menu,
    /// Lien `irc://` ou `ircs://` venu du système (Apple Event sur Mac, seconde instance sous Linux).
    OpenUrl(String),
    /// Ramener la fenêtre devant (seconde instance lancée sans lien, clic sur une notification).
    Raise,
}

/// Vrai pour un lien que `Shell::open_irc_url` sait ouvrir.
pub fn is_irc_url(s: &str) -> bool {
    let l = s.get(..7).unwrap_or(s).to_ascii_lowercase();
    l.starts_with("irc://") || l.starts_with("ircs://")
}

/// Heure réelle en millisecondes depuis 1970.
pub fn wall_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Dossiers de l'application : (réglages, données).
fn dirs() -> (PathBuf, PathBuf) {
    if let Some(d) = std::env::var_os("ALPIRC_HOME").map(PathBuf::from) {
        return (d.join("settings.toml"), d);
    }
    match directories::ProjectDirs::from("fr", "guiwald", "AlpIRC") {
        Some(p) => (p.config_dir().join("settings.toml"), p.data_dir().to_path_buf()),
        None => {
            let d = std::env::temp_dir().join("alpirc");
            (d.join("settings.toml"), d)
        }
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let args: Vec<String> = std::env::args().skip(1).filter(|a| !a.starts_with("-psn_")).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("AlpIRC {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    // L'heure locale se lit avant de lancer des fils (le crate `time` le refuse ensuite sous Unix).
    let now = wall_ms();
    let utc_offset = alpirc_core::timeutil::local_offset(now);
    let (settings_path, data_dir) = dirs();
    let _ = std::fs::create_dir_all(&data_dir);
    if let Some(parent) = settings_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // Lien irc:// passé en argument (ouverture depuis le bureau sous Linux, ou `open --args` sur Mac).
    let open_url = args.into_iter().find(|a| is_irc_url(a));
    // Linux : si AlpIRC tourne déjà (même dossier de données), il prend le lien et ce processus s'arrête.
    #[cfg(target_os = "linux")]
    let instance = match instance::claim(&data_dir, open_url.as_deref()) {
        instance::Claim::Primary(p) => Some(p),
        instance::Claim::Handed => return,
        instance::Claim::Alone => None,
    };
    let event_loop = match EventLoop::<UserEvent>::with_user_event().build() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("AlpIRC : impossible d'ouvrir l'affichage ({e})");
            std::process::exit(1);
        }
    };
    #[cfg(target_os = "linux")]
    if let Some(p) = instance {
        p.serve(event_loop.create_proxy());
    }
    // macOS : les liens arrivent par Apple Event, avant ou après le lancement.
    #[cfg(target_os = "macos")]
    macevents::install(event_loop.create_proxy());
    let proxy = event_loop.create_proxy();
    let net = Net::new(Box::new(move |ev| {
        let _ = proxy.send_event(UserEvent::Net(ev));
    }));
    let cfg = ShellConfig {
        settings_path,
        data_dir,
        layout: LayoutKind::Desktop,
        utc_offset,
        locale: sys_locale::get_locale(),
        now_ms: now,
    };
    let shell = Shell::new(cfg, net);
    let mut app = app::App::new(shell, event_loop.create_proxy(), open_url);
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("AlpIRC : {e}");
    }
}
