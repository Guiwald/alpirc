//! Simulateur de téléphone : l'appli Android dans une fenêtre de la taille de l'écran d'un Pixel 8 divisée par
//! la taille de l'interface (412×915 par défaut, `ALPIRC_SIM_SCALE=3` pour 360×800, comme le téléphone), la
//! souris pour doigt (bouton droit : appui long), la ligne de saisie de Rust à la place du champ natif.
//!
//! Variables : `ALPIRC_HOME` (dossier des réglages), `ALPIRC_SIM_SCALE` (taille de l'interface),
//! `ALPIRC_SIM_LOCALE` (langue), `ALPIRC_SIM_PHYSICAL=1` (fenêtre de 1080×2400 dessinée comme sur le Pixel 8 :
//! il faut un écran assez haut), `RUST_LOG`.

#[cfg(not(target_os = "android"))]
fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    alpirc::run_simulator();
}

#[cfg(target_os = "android")]
fn main() {}
