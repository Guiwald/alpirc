//! AlpIRC pour Android (ADR 0006) : la disposition du téléphone du shell (une fenêtre à la fois, tiroirs, menus
//! au toucher), dessinée par Rust dans une `NativeActivity` (winit, softbuffer), avec un champ de saisie natif,
//! un service de connexion au premier plan et les notifications, tenus par quelques classes Java
//! (`app/android/java`).
//!
//! Sur ordinateur, la même appli tourne dans un simulateur de téléphone (`cargo run -p alpirc-android --bin
//! phone-sim`), la souris pour doigt et la ligne de saisie de Rust à la place du champ natif.

// Style du projet : on garde parfois des `if` imbriqués pour la lisibilité.
#![allow(clippy::collapsible_if)]

pub mod app;
pub mod host;
pub mod keys;

#[cfg(target_os = "android")]
mod android;

/// Lance l'appli du simulateur de téléphone (ordinateur).
#[cfg(not(target_os = "android"))]
pub fn run_simulator() {
    use winit::event_loop::EventLoop;
    let event_loop = match EventLoop::<app::UserEvent>::with_user_event().build() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("AlpIRC : impossible d'ouvrir l'affichage ({e})");
            std::process::exit(1);
        }
    };
    let host = host::SimHost::new();
    let mut m = app::build(Box::new(host), event_loop.create_proxy());
    if let Err(e) = event_loop.run_app(&mut m) {
        eprintln!("AlpIRC : {e}");
    }
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    use std::panic::AssertUnwindSafe;
    use winit::platform::android::EventLoopBuilderExtAndroid;

    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("AlpIRC"));
    std::panic::set_hook(Box::new(|info| log::error!("panique : {info}")));
    log::info!("AlpIRC {} démarre", env!("CARGO_PKG_VERSION"));
    android::init_bridge(&app);
    let event_loop = match winit::event_loop::EventLoop::<app::UserEvent>::with_user_event().with_android_app(app.clone()).build() {
        Ok(e) => e,
        Err(winit::error::EventLoopError::RecreationAttempt) => {
            // La boucle de winit ne se crée qu'une fois par processus : relance dans un processus neuf.
            log::warn!("activité recréée dans un processus qui a déjà servi : relance");
            if !android::restart() {
                android::exit_app();
            }
            return;
        }
        Err(e) => {
            log::error!("boucle d'événements : {e}");
            return;
        }
    };
    let proxy = event_loop.create_proxy();
    android::start_event_thread(proxy.clone());
    let host = android::AndroidHost::new(app);
    let built = std::panic::catch_unwind(AssertUnwindSafe(|| app::build(Box::new(host), proxy)));
    let Ok(mut m) = built else {
        log::error!("démarrage impossible");
        android::exit_app();
        return;
    };
    if let Err(e) = event_loop.run_app(&mut m) {
        log::error!("{e}");
    }
    android::stop_event_thread();
    log::info!("AlpIRC s'arrête");
}
