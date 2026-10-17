//! Ce que l'appli du téléphone demande au système (Android par JNI, ou le simulateur), et les événements qui en
//! viennent.
//!
//! Le code Java d'AlpIRC (`app/android/java`) pose des événements texte dans une file ; ils sont décodés ici
//! ([`parse_event`]), ce qui se teste sans téléphone.

use std::path::{Path, PathBuf};

/// Un événement du système (code Java d'AlpIRC).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SysEvent {
    /// Les marges de l'écran ont changé (barres du système, clavier, champ de saisie natif).
    Insets,
    /// Texte validé dans le champ de saisie natif (Entrée, bouton Envoyer).
    Send(String),
    /// Bouton Tab du champ natif : compléter le dernier mot de ce texte.
    Tab(String),
    /// Bouton Retour.
    Back,
    /// Lien `irc://` ou `ircs://` ouvert avec AlpIRC.
    Url(String),
    /// L'appli passe au premier plan (`true`) ou derrière.
    Foreground(bool),
    /// Quitter (notification du service, appli balayée des applis récentes).
    Quit,
    /// L'activité se termine ; `config` : Android la recrée (changement de configuration).
    Destroyed { config: bool },
    /// Réponse du sélecteur de fichiers (jeton de la demande ; copie locale du document, ou rien si annulé).
    Picked { token: u32, path: Option<PathBuf> },
    Log(String),
}

/// Décode un événement de la file du code Java : `nom` puis, après une tabulation, le texte.
pub fn parse_event(raw: &str) -> Option<SysEvent> {
    let (head, rest) = raw.split_once('\t').unwrap_or((raw, ""));
    Some(match head {
        "insets" => SysEvent::Insets,
        "send" => SysEvent::Send(rest.to_string()),
        "tab" => SysEvent::Tab(rest.to_string()),
        "back" => SysEvent::Back,
        "url" if !rest.is_empty() => SysEvent::Url(rest.trim().to_string()),
        "fg" => SysEvent::Foreground(rest == "1"),
        "quit" => SysEvent::Quit,
        "activity" => match rest.split('\t').collect::<Vec<_>>().as_slice() {
            ["destroyed", how] => SysEvent::Destroyed { config: *how == "config" },
            _ => return None,
        },
        "picked" => {
            let (token, path) = rest.split_once('\t').unwrap_or((rest, ""));
            SysEvent::Picked { token: token.trim().parse().ok()?, path: (!path.is_empty()).then(|| PathBuf::from(path)) }
        }
        "log" => SysEvent::Log(rest.to_string()),
        _ => return None,
    })
}

/// Le service de connexion : nombre de connexions ouvertes (0 : arrêt) et textes de sa notification.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServiceState {
    pub active: usize,
    pub title: String,
    pub text: String,
    /// Bouton « Quitter », nom du canal de la notification du service, nom du canal des mentions.
    pub quit: String,
    pub channel: String,
    pub notify_channel: String,
}

/// Les services du système pour l'appli du téléphone.
pub trait Host {
    fn clipboard_get(&mut self) -> Option<String>;
    fn clipboard_set(&mut self, text: &str);
    /// Notification (mention, message privé) ; la toucher ouvre AlpIRC.
    fn notify(&mut self, title: &str, body: &str);
    fn vibrate(&mut self, ms: i32);
    fn open_url(&mut self, url: &str);
    /// Dossier des réglages, des scripts et des journaux.
    fn data_dir(&self) -> PathBuf;
    /// Décalage de l'heure locale (secondes) à l'instant `now_ms`.
    fn utc_offset(&self, now_ms: i64) -> i32;
    /// Langue du système (`fr-FR`).
    fn locale(&self) -> Option<String>;
    /// Densité de l'écran (pixels par dp), si le système la connaît mieux que la fenêtre.
    fn density(&self) -> Option<f32> {
        None
    }
    /// Polices de repli du système (emoji, CJK…), à lire à la demande.
    fn font_fallbacks(&self) -> Vec<PathBuf> {
        Vec::new()
    }
    /// Marges de l'écran en pixels du téléphone : gauche, haut, droite, bas (barre de navigation, clavier et
    /// champ de saisie natif compris).
    fn insets(&self) -> [i32; 4] {
        [0; 4]
    }
    /// Le champ de saisie est natif (Android) ; sinon la ligne de saisie de Rust sert.
    fn native_input(&self) -> bool {
        false
    }
    /// Affiche ou cache le champ de saisie natif, avec son texte d'aide.
    fn set_input(&mut self, visible: bool, hint: &str) {
        let _ = (visible, hint);
    }
    /// Remplace le texte du champ natif (complétion).
    fn set_input_text(&mut self, text: &str) {
        let _ = text;
    }
    /// Libellés des boutons du champ natif.
    fn set_input_labels(&mut self, send: &str, tab: &str) {
        let _ = (send, tab);
    }
    /// Ouvre ou ferme le clavier du système pour un champ dessiné par Rust (boîtes de dialogue).
    fn show_keyboard(&mut self, show: bool) {
        let _ = show;
    }
    /// Service de connexion (notification permanente) ; `active == 0` : arrêt.
    fn service(&mut self, s: &ServiceState) {
        let _ = s;
    }
    /// L'appli passe derrière (bouton Retour) ; `false` si ce n'est pas possible.
    fn to_back(&mut self) -> bool {
        false
    }
    /// Quitte pour de bon : service arrêté, activité et processus fermés.
    fn exit_app(&mut self) {}
    /// Dossier des fichiers reçus.
    fn downloads_dir(&self) -> Option<PathBuf> {
        None
    }
    /// Ouvre le sélecteur de documents du système ; la réponse arrive en [`SysEvent::Picked`] avec le même
    /// jeton. `false` : pas de sélecteur (la demande est aussitôt annulée).
    fn pick_file(&mut self, token: u32) -> bool {
        let _ = token;
        false
    }
    /// Ouvre un fichier reçu dans l'appli qui convient (un dossier : son emplacement est affiché).
    fn open_path(&mut self, path: &Path) {
        let _ = path;
    }
    /// Le simulateur : la fenêtre suit la taille de l'interface (écran du téléphone ÷ échelle).
    fn simulated_screen(&self) -> Option<(u32, u32)> {
        None
    }
}

/// Réglages propres au téléphone (`android.toml` dans le dossier des données) : la taille de l'interface.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prefs {
    /// Pixels du téléphone par pixel logique ; `None` : automatique (selon la densité de l'écran).
    pub scale: Option<f32>,
}

impl Prefs {
    pub fn path(data_dir: &Path) -> PathBuf {
        data_dir.join("android.toml")
    }

    pub fn load(data_dir: &Path) -> Prefs {
        let text = std::fs::read_to_string(Self::path(data_dir)).unwrap_or_default();
        Prefs::parse(&text)
    }

    pub fn parse(text: &str) -> Prefs {
        let mut p = Prefs::default();
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == "scale" {
                    p.scale = v.trim().parse::<f32>().ok().filter(|s| (1.0..=8.0).contains(s));
                }
            }
        }
        p
    }

    pub fn to_text(&self) -> String {
        let mut s = String::from("# Réglages d'AlpIRC propres au téléphone.\n# Taille de l'interface (pixels de l'écran par pixel d'AlpIRC) ; absent : automatique.\n");
        if let Some(v) = self.scale {
            s.push_str(&format!("scale = {v}\n"));
        }
        s
    }

    pub fn save(&self, data_dir: &Path) {
        if let Err(e) = std::fs::write(Self::path(data_dir), self.to_text()) {
            log::warn!("android.toml non enregistré : {e}");
        }
    }
}

/// Taille d'interface automatique pour une densité d'écran (pixels par dp) : environ 8/7 de la densité,
/// arrondi pour que la police en pixels reste nette (Pixel 8, 2,625 → 3, soit 360 pixels de large).
pub fn auto_scale(density: f32) -> f32 {
    (density * 8.0 / 7.0).round().clamp(1.0, 6.0)
}

// ---------------------------------------------------------------------------------------------------------------
// Simulateur.

/// Écran du Pixel 8 (pixels) et sa densité.
pub const PIXEL8: (u32, u32) = (1080, 2400);
pub const PIXEL8_DENSITY: f32 = 2.625;

/// Le simulateur de téléphone sur ordinateur : presse-papiers en mémoire, notifications et vibrations dans le
/// journal, marges d'un Pixel 8 (barre d'état de 24 dp, barre de navigation de 16 dp).
#[cfg(not(target_os = "android"))]
pub struct SimHost {
    clipboard: Option<String>,
    data: PathBuf,
}

#[cfg(not(target_os = "android"))]
impl SimHost {
    pub fn new() -> SimHost {
        let data = std::env::var_os("ALPIRC_HOME").map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join("alpirc-phone-sim"));
        SimHost { clipboard: None, data }
    }
}

#[cfg(not(target_os = "android"))]
impl Default for SimHost {
    fn default() -> SimHost {
        SimHost::new()
    }
}

#[cfg(not(target_os = "android"))]
impl Host for SimHost {
    fn clipboard_get(&mut self) -> Option<String> {
        self.clipboard.clone()
    }
    fn clipboard_set(&mut self, text: &str) {
        self.clipboard = Some(text.to_string());
    }
    fn notify(&mut self, title: &str, body: &str) {
        log::info!("notification : {title} — {body}");
    }
    fn vibrate(&mut self, ms: i32) {
        log::info!("vibration : {ms} ms");
    }
    fn open_url(&mut self, url: &str) {
        log::info!("ouvrir : {url}");
    }
    fn data_dir(&self) -> PathBuf {
        self.data.clone()
    }
    fn utc_offset(&self, now_ms: i64) -> i32 {
        alpirc_core::timeutil::local_offset(now_ms)
    }
    fn locale(&self) -> Option<String> {
        std::env::var("ALPIRC_SIM_LOCALE").ok().or_else(sys_locale::get_locale)
    }
    fn density(&self) -> Option<f32> {
        Some(PIXEL8_DENSITY)
    }
    fn insets(&self) -> [i32; 4] {
        let d = PIXEL8_DENSITY;
        [0, (24.0 * d) as i32, 0, (16.0 * d) as i32]
    }
    fn simulated_screen(&self) -> Option<(u32, u32)> {
        Some(PIXEL8)
    }
    fn downloads_dir(&self) -> Option<PathBuf> {
        Some(self.data.join("downloads"))
    }
    fn open_path(&mut self, path: &Path) {
        log::info!("ouvrir : {}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evenements_decodes() {
        assert_eq!(parse_event("insets"), Some(SysEvent::Insets));
        assert_eq!(parse_event("send\tsalut\ttout le monde"), Some(SysEvent::Send("salut\ttout le monde".into())));
        assert_eq!(parse_event("send\tdeux\nlignes"), Some(SysEvent::Send("deux\nlignes".into())));
        assert_eq!(parse_event("tab\tsalut gu"), Some(SysEvent::Tab("salut gu".into())));
        assert_eq!(parse_event("back"), Some(SysEvent::Back));
        assert_eq!(parse_event("url\tirc://irc.libera.chat/#alpirc"), Some(SysEvent::Url("irc://irc.libera.chat/#alpirc".into())));
        assert_eq!(parse_event("url\t"), None);
        assert_eq!(parse_event("fg\t1"), Some(SysEvent::Foreground(true)));
        assert_eq!(parse_event("fg\t0"), Some(SysEvent::Foreground(false)));
        assert_eq!(parse_event("quit"), Some(SysEvent::Quit));
        assert_eq!(parse_event("activity\tdestroyed\tconfig"), Some(SysEvent::Destroyed { config: true }));
        assert_eq!(parse_event("activity\tdestroyed\tfinish"), Some(SysEvent::Destroyed { config: false }));
        assert_eq!(parse_event("activity\tautre"), None);
        assert_eq!(parse_event("log\tbonjour"), Some(SysEvent::Log("bonjour".into())));
        assert_eq!(parse_event("picked\t3\t/data/cache/picked/3/photo.jpg"), Some(SysEvent::Picked { token: 3, path: Some(PathBuf::from("/data/cache/picked/3/photo.jpg")) }));
        assert_eq!(parse_event("picked\t4"), Some(SysEvent::Picked { token: 4, path: None }));
        assert_eq!(parse_event("picked\tx"), None);
        assert_eq!(parse_event("inconnu"), None);
    }

    #[test]
    fn reglages_du_telephone() {
        assert_eq!(Prefs::parse(""), Prefs { scale: None });
        assert_eq!(Prefs::parse("scale = 2.5\n"), Prefs { scale: Some(2.5) });
        assert_eq!(Prefs::parse("# rien\nscale=3 # commentaire"), Prefs { scale: Some(3.0) });
        assert_eq!(Prefs::parse("scale = 99"), Prefs { scale: None }, "valeur absurde ignorée");
        let p = Prefs { scale: Some(2.75) };
        assert_eq!(Prefs::parse(&p.to_text()), p);
        assert_eq!(Prefs::parse(&Prefs::default().to_text()), Prefs::default());
    }

    #[test]
    fn taille_automatique() {
        assert_eq!(auto_scale(2.625), 3.0, "Pixel 8");
        assert_eq!(auto_scale(2.0), 2.0);
        assert_eq!(auto_scale(3.5), 4.0);
        assert_eq!(auto_scale(1.0), 1.0);
    }
}
