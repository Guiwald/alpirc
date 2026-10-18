//! Disposition du téléphone (ADR 0006) : une fenêtre à la fois en plein écran ; barre du haut façon barre de
//! titre de Windows 98 (☰ tiroir des fenêtres, nom de la fenêtre et sujet, bouton des pseudos, ⋮ menu) ; tiroir
//! des fenêtres à gauche (réseaux, canaux, privés, couleurs d'activité de la switchbar) ; liste des pseudos dans
//! un tiroir à droite ; menus au toucher (lignes hautes, sous-menus en descente) ; boîtes de dialogue du bureau
//! réduites à la largeur de l'écran, avec des zones tactiles élargies.
//!
//! Le toucher arrive comme la souris (`MouseDown` / `MouseMove` / `MouseUp`, bouton gauche) et devient ici des
//! gestes : toucher bref = clic, deux touchers rapprochés = double-clic, appui long (≈ 450 ms sans bouger) = clic
//! droit (menus des scripts), glisser vertical = défilement (molette, avec un élan au lâcher), glisser depuis un
//! bord = ouvrir un tiroir. Le simulateur de téléphone envoie la souris telle quelle : mêmes gestes.
//!
//! Sur Android, la saisie se fait dans un champ natif (`EditText`, ADR 0006) ; la ligne de saisie de Rust est
//! alors placée sous lui (invisible) et le texte arrive par [`Shell::mobile_submit`].

// Style du projet : on garde parfois des `if` imbriqués pour la lisibilité.
#![allow(clippy::collapsible_if)]

use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;

use alpirc_core::i18n::Lang;
use alpirc_core::{ConnState, WinId, WinKind};
use alpirc_script::PopupKind;
use alpirc_ui::widgets::{ArrowDir, CHECK_MARK, MENU_DOT, draw_arrow, draw_pattern};
use alpirc_ui::{
    Bevel, Canvas, ControlKind, DialogMsg, DialogSpec, DialogView, EditMsg, FontKind, Input, Key, MenuItem, Mods,
    MouseButton, Point, Rect, TextStyle, draw_mirc_line,
};

use crate::dialogs::{Cx, DialogCtl, Outcome};
use crate::menus::{self, ids};
use crate::tr::{plain, t, tf};
use crate::views::{self, WinLines, short_label};
use crate::{PopupSrc, Shell, ShellPlatform, win_menu};

/// Textes propres au téléphone (voir [`crate::tr`]).
pub const TR: crate::tr::Table = &[
    ("mobile.windows", ["Windows", "Fenêtres", "Ventanas", "Fenster", "Finestre", "Finestres", "Fuinneoga"]),
    (
        "mobile.users",
        ["{n} users", "{n} personnes", "{n} usuarios", "{n} Nutzer", "{n} utenti", "{n} usuaris", "{n} úsáideoir"],
    ),
    (
        "mobile.join",
        [
            "&Join a Channel...",
            "&Rejoindre un canal...",
            "&Entrar en un canal...",
            "Channel &betreten...",
            "&Entra in un canale...",
            "&Entrar en un canal...",
            "Téigh &isteach i gcainéal...",
        ],
    ),
    (
        "mobile.copy_line",
        [
            "&Copy the Line",
            "&Copier la ligne",
            "&Copiar la línea",
            "Zeile &kopieren",
            "&Copia la riga",
            "&Copiar la línia",
            "&Cóipeáil an líne",
        ],
    ),
    (
        "mobile.display_size",
        [
            "Display &Size",
            "&Taille de l'affichage",
            "&Tamaño de la pantalla",
            "&Anzeigegröße",
            "Dimensione dello &schermo",
            "&Mida de la pantalla",
            "&Méid na taispeána",
        ],
    ),
    (
        "mobile.no_window",
        [
            "No window",
            "Aucune fenêtre",
            "Ninguna ventana",
            "Kein Fenster",
            "Nessuna finestra",
            "Cap finestra",
            "Gan fuinneog",
        ],
    ),
    (
        "mobile.hint",
        [
            "Message {win}",
            "Message dans {win}",
            "Mensaje en {win}",
            "Nachricht in {win}",
            "Messaggio in {win}",
            "Missatge en {win}",
            "Teachtaireacht i {win}",
        ],
    ),
    (
        "mobile.hint.status",
        [
            "Command (/join #channel)",
            "Commande (/join #canal)",
            "Comando (/join #canal)",
            "Befehl (/join #channel)",
            "Comando (/join #canale)",
            "Ordre (/join #canal)",
            "Ordú (/join #cainéal)",
        ],
    ),
    ("mobile.send", ["Send", "Envoyer", "Enviar", "Senden", "Invia", "Enviar", "Seol"]),
    ("mobile.tab", ["Tab", "Tab", "Tab", "Tab", "Tab", "Tab", "Tab"]),
    (
        "mobile.service.one",
        [
            "Connected to 1 network",
            "Connecté à 1 réseau",
            "Conectado a 1 red",
            "Mit 1 Netzwerk verbunden",
            "Connesso a 1 rete",
            "Connectat a 1 xarxa",
            "Ceangailte le líonra amháin",
        ],
    ),
    (
        "mobile.service.many",
        [
            "Connected to {n} networks",
            "Connecté à {n} réseaux",
            "Conectado a {n} redes",
            "Mit {n} Netzwerken verbunden",
            "Connesso a {n} reti",
            "Connectat a {n} xarxes",
            "Ceangailte le {n} líonra",
        ],
    ),
    (
        "mobile.service.connecting",
        [
            "Connecting...",
            "Connexion en cours...",
            "Conectando...",
            "Verbinde...",
            "Connessione in corso...",
            "S'està connectant...",
            "Ag ceangal...",
        ],
    ),
    (
        "mobile.service.waiting",
        [
            "Disconnected, waiting to reconnect",
            "Déconnecté, reconnexion en attente",
            "Desconectado, esperando para reconectar",
            "Getrennt, warte auf Neuverbindung",
            "Disconnesso, in attesa di riconnessione",
            "Desconnectat, en espera de reconnectar",
            "Dícheangailte, ag fanacht le hathcheangal",
        ],
    ),
    ("mobile.service.quit", ["Quit", "Quitter", "Salir", "Beenden", "Esci", "Eixir", "Scoir"]),
    (
        "mobile.service.channel",
        ["Connection", "Connexion", "Conexión", "Verbindung", "Connessione", "Connexió", "Ceangal"],
    ),
    (
        "mobile.notify.channel",
        [
            "Highlights and private messages",
            "Mentions et messages privés",
            "Menciones y mensajes privados",
            "Erwähnungen und private Nachrichten",
            "Menzioni e messaggi privati",
            "Mencions i missatges privats",
            "Luanna agus teachtaireachtaí príobháideacha",
        ],
    ),
    ("mobile.phone", ["&Phone", "&Téléphone", "&Teléfono", "Tele&fon", "&Telefono", "&Telèfon", "&Fón"]),
    (
        "mobile.bg",
        [
            "&Disconnect in Background",
            "&Déconnexion en arrière-plan",
            "&Desconexión en segundo plano",
            "&Trennen im Hintergrund",
            "&Disconnessione in background",
            "&Desconnexió en segon pla",
            "&Dícheangal sa chúlra",
        ],
    ),
    ("mobile.bg.never", ["&Never", "&Jamais", "&Nunca", "&Nie", "&Mai", "&Mai", "&Choíche"]),
    (
        "mobile.bg.after",
        [
            "After {n} minutes",
            "Après {n} minutes",
            "Tras {n} minutos",
            "Nach {n} Minuten",
            "Dopo {n} minuti",
            "Després de {n} minuts",
            "Tar éis {n} nóiméad",
        ],
    ),
    (
        "mobile.bg.other",
        [
            "&Other Duration...",
            "&Autre durée...",
            "&Otra duración...",
            "&Andere Dauer...",
            "&Altra durata...",
            "&Altra durada...",
            "Fad &eile...",
        ],
    ),
    (
        "mobile.bg.other_n",
        [
            "&Other Duration ({n} min)...",
            "&Autre durée ({n} min)...",
            "&Otra duración ({n} min)...",
            "&Andere Dauer ({n} Min.)...",
            "&Altra durata ({n} min)...",
            "&Altra durada ({n} min)...",
            "Fad &eile ({n} nóim.)...",
        ],
    ),
    (
        "mobile.bg.title",
        [
            "Disconnect in Background",
            "Déconnexion en arrière-plan",
            "Desconexión en segundo plano",
            "Trennen im Hintergrund",
            "Disconnessione in background",
            "Desconnexió en segon pla",
            "Dícheangal sa chúlra",
        ],
    ),
    (
        "mobile.bg.prompt",
        [
            "Disconnect from IRC after this many minutes in the background (0: never):",
            "Se déconnecter d'IRC après ce nombre de minutes en arrière-plan (0 : jamais) :",
            "Desconectar de IRC tras estos minutos en segundo plano (0: nunca):",
            "Nach so vielen Minuten im Hintergrund vom IRC trennen (0: nie):",
            "Disconnetti da IRC dopo questi minuti in background (0: mai):",
            "Desconnectar d'IRC després d'estos minuts en segon pla (0: mai):",
            "Dícheangail ó IRC tar éis an oiread seo nóiméad sa chúlra (0: choíche):",
        ],
    ),
    (
        "mobile.bg.minutes",
        [
            "minutes (at most 1440)",
            "minutes (1440 au plus)",
            "minutos (1440 como máximo)",
            "Minuten (höchstens 1440)",
            "minuti (al massimo 1440)",
            "minuts (1440 com a màxim)",
            "nóiméad (1440 ar a mhéad)",
        ],
    ),
    (
        "mobile.bg.done",
        [
            "Disconnected after {n} min in the background",
            "Déconnecté après {n} min en arrière-plan",
            "Desconectado tras {n} min en segundo plano",
            "Nach {n} Min. im Hintergrund getrennt",
            "Disconnesso dopo {n} min in background",
            "Desconnectat després de {n} min en segon pla",
            "Dícheangailte tar éis {n} nóim. sa chúlra",
        ],
    ),
    (
        "mobile.vibrate",
        [
            "&Vibrate with Beeps",
            "&Vibrer avec les bips",
            "&Vibrar con los pitidos",
            "Bei Pieptönen &vibrieren",
            "&Vibra con i bip",
            "&Vibrar amb els bips",
            "&Crith le bípeanna",
        ],
    ),
    ("mobile.history", ["History", "Historique", "Historial", "Verlauf", "Cronologia", "Historial", "Stair"]),
];

/// Hauteur de la barre du haut (pixels logiques) : deux lignes, nom et sujet.
pub const TOP_BAR_H: i32 = 44;
/// Largeur des boutons de la barre du haut.
const BTN_W: i32 = 40;
/// Lignes du tiroir des fenêtres (zone tactile d'au moins 44 px, ADR 0006).
const ROW_H: i32 = 44;
/// Lignes du tiroir des pseudos.
const NICK_ROW_H: i32 = 40;
/// En-tête des tiroirs.
const HEADER_H: i32 = 44;
/// Lignes des menus au toucher.
const MENU_ROW_H: i32 = 40;
const MENU_SEP_H: i32 = 9;
const MENU_BORDER: i32 = 3;
/// Appui long : clic droit.
const LONG_PRESS_MS: i64 = 450;
/// Deux touchers plus rapprochés : double-clic.
const DOUBLE_TAP_MS: i64 = 350;
const DOUBLE_TAP_DIST: i32 = 24;
/// Au-delà (pixels logiques), un toucher devient un glissement.
const SLOP: i32 = 8;
/// Bande des bords d'où un glissement horizontal ouvre un tiroir.
const EDGE: i32 = 20;
/// Distance horizontale qui ouvre un tiroir depuis un bord.
const EDGE_OPEN: i32 = 36;
/// Élan : intervalle des pas, freinage par pas de 16 ms, vitesse minimale (px/ms).
const FLING_STEP_MS: i64 = 16;
const FLING_DECAY: f32 = 0.95;
const FLING_MIN: f32 = 0.03;
/// Rayon d'attraction des contrôles des boîtes de dialogue (pixels de la boîte).
const SNAP: i32 = 12;
/// Plus petite réduction d'une boîte de dialogue, et plus grand agrandissement (les boîtes sont mises à la
/// largeur de l'écran : zones tactiles plus grandes).
const MIN_DIALOG_ZOOM: f32 = 0.5;
const MAX_DIALOG_ZOOM: f32 = 1.5;
/// Identifiants des entrées propres au menu ⋮ du téléphone (hors des plages de [`menus::ids`]).
const M_JOIN: u32 = 990_001;
/// Copier la ligne de discussion sous le doigt (appui long).
const M_COPY_LINE: u32 = 990_002;
/// Taille de l'affichage : `M_SCALE + centièmes` (200 = 2×).
const M_SCALE: u32 = 990_000;
const SCALES: [f32; 7] = [2.0, 2.25, 2.5, 2.75, 3.0, 3.5, 4.0];
/// Déconnexion en arrière-plan : `M_BG_BASE + minutes` (0 : jamais), « Autre durée… ».
const M_BG_BASE: u32 = 992_000;
const M_BG_OTHER: u32 = 990_003;
/// Vibrer avec les bips.
const M_VIBRATE: u32 = 990_004;
/// Durées proposées (minutes ; 0 : jamais) et durée la plus longue acceptée.
const BG_CHOICES: [u32; 5] = [0, 5, 15, 30, 60];
const BG_MAX_MIN: u32 = 1440;
/// Lignes de l'historique montrées par le bouton du champ natif.
const HISTORY_MENU_MAX: usize = 30;
/// Hauteur minimale des lignes des listes au toucher (boîtes de dialogue, fenêtres à liste).
const TOUCH_ROW_H: i32 = 24;

/// Réglages propres au téléphone, gardés par la plateforme (`android.toml`) et réglés depuis le menu ⋮
/// (Téléphone).
#[derive(Clone, Debug, PartialEq, Eq)]
struct PhonePrefs {
    /// Se déconnecter d'IRC après ce nombre de minutes en arrière-plan ; 0 : jamais (ADR 0006).
    pub background_disconnect_min: u32,
    /// Vibrer avec les bips.
    pub vibrate: bool,
}

impl Default for PhonePrefs {
    fn default() -> PhonePrefs {
        PhonePrefs { background_disconnect_min: 0, vibrate: true }
    }
}

/// Tiroir ouvert.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Drawer {
    /// Fenêtres, à gauche.
    Windows,
    /// Pseudos du canal, à droite.
    Nicks,
}

/// Bouton de la barre du haut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopButton {
    Windows,
    Nicks,
    Menu,
}

/// Ce qui est sous le doigt quand il se pose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Button(TopButton),
    /// Barre du haut hors des boutons.
    Title,
    /// Contenu de la fenêtre : toucher, appui long, glisser = défiler.
    Content,
    /// Ligne de saisie de Rust (simulateur) : événements transmis tels quels.
    Editbox,
    Drawer,
    /// Hors du tiroir ouvert : le ferme.
    Scrim,
    Menu,
    /// Hors du menu ouvert : le ferme.
    OutsideMenu,
    /// Boîte de dialogue (modale) : événements transmis, en coordonnées de la boîte.
    Dialog,
}

/// Un doigt posé.
struct Touch {
    target: Target,
    start: Point,
    last: Point,
    t0: i64,
    moved: bool,
    /// Appui long déjà traité.
    long_done: bool,
    /// Glisser de défilement en cours.
    scrolling: bool,
    /// Derniers points (heure, x, y), pour la vitesse de l'élan.
    samples: VecDeque<(i64, i32, i32)>,
    /// Glissé depuis un bord : le tiroir qu'il ouvrira.
    edge: Option<Drawer>,
    /// Décalage appliqué au doigt dans une boîte de dialogue (attraction vers le contrôle le plus proche).
    snap: (i32, i32),
    /// Posé sur une liste d'une boîte de dialogue : glisser la fait défiler.
    dialog_list: bool,
}

/// Ce que fait défiler un élan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scroller {
    Content,
    Drawer,
    Menu,
}

/// Élan après un glisser : vitesse en pixels logiques par milliseconde.
struct Fling {
    vx: f32,
    vy: f32,
    last: i64,
    at: Point,
    what: Scroller,
}

/// Origine d'un menu au toucher.
enum MenuOrigin {
    /// Menu ⋮ : commandes de [`menus::ids`] et entrées du téléphone.
    Main,
    /// Menu surgissant du shell (scripts, fenêtre).
    Popup(PopupSrc),
    /// Historique de la ligne de saisie d'une fenêtre (bouton du champ natif) : entrée `n` = ligne `n - 1`.
    History(WinId),
}

struct MenuLevel {
    /// Titre (sous-menus : l'entrée parente, affichée avec une flèche de retour).
    title: Option<String>,
    items: Vec<MenuItem>,
    scroll: i32,
}

/// Menu au toucher : un niveau à la fois, lignes hautes, sous-menus en descente.
struct TouchMenu {
    origin: MenuOrigin,
    /// Ce dont parle le menu (pseudo, fenêtre), en tête du premier niveau.
    caption: Option<String>,
    levels: Vec<MenuLevel>,
    anchor: Point,
    /// Ancré par son bord droit (menu ⋮).
    right_aligned: bool,
}

impl TouchMenu {
    fn new(origin: MenuOrigin, items: Vec<MenuItem>, anchor: Point, right_aligned: bool) -> TouchMenu {
        TouchMenu { origin, caption: None, levels: vec![MenuLevel { title: None, items: clean_items(items), scroll: 0 }], anchor, right_aligned }
    }

    fn with_caption(mut self, caption: Option<String>) -> TouchMenu {
        self.caption = caption.filter(|c| !c.is_empty());
        self
    }

    fn level(&self) -> &MenuLevel {
        self.levels.last().expect("niveau de menu")
    }

    /// Ligne de tête du niveau affiché : titre du sous-menu (avec retour), ou ce dont parle le menu.
    fn header(&self) -> Option<&str> {
        if self.levels.len() > 1 { self.level().title.as_deref() } else { self.caption.as_deref() }
    }
}

/// État de la disposition du téléphone.
#[derive(Default)]
pub struct MobileState {
    /// Fenêtre affichée.
    pub active: Option<WinId>,
    pub drawer: Option<Drawer>,
    /// Marges imposées par le système (barre d'état ; barre de navigation, clavier et champ de saisie natif),
    /// en pixels logiques : haut, bas.
    pub insets: (i32, i32),
    /// Champ de saisie natif (Android) : la ligne de saisie de Rust est cachée.
    pub native_input: bool,
    touch: Option<Touch>,
    fling: Option<Fling>,
    last_tap: Option<(i64, Point)>,
    menu: Option<TouchMenu>,
    drawer_scroll: i32,
    pressed_button: Option<TopButton>,
    /// Ligne enfoncée (tiroir ou menu), pour le retour visuel.
    pressed_row: Option<usize>,
    /// Pixels de défilement pas encore convertis en rangées.
    scroll_acc: f32,
    /// Complétion en cours du champ natif : fenêtre, mot d'origine, début du mot, dernier texte rendu, rang.
    tab: Option<(WinId, String, usize, String, usize)>,
    /// Texte de la ligne sous le doigt lors du dernier appui long (entrée « Copier la ligne »).
    copy_text: Option<String>,
    /// Réglages du téléphone (`None` : pas de section Téléphone), et s'ils ont changé depuis le menu.
    phone: Option<PhonePrefs>,
    phone_changed: bool,
    /// Réponse de la boîte « Autre durée » (minutes).
    bg_answer: Rc<Cell<Option<u32>>>,
    /// Connexions fermées par la déconnexion en arrière-plan (fenêtres Status), à rouvrir au retour.
    bg_disconnected: Vec<WinId>,
    /// Parcours de l'historique dans le champ natif : fenêtre, rang de la ligne affichée, brouillon gardé.
    history: Option<(WinId, usize, String)>,
    /// Texte à mettre dans le champ natif (historique choisi).
    native_text: Option<String>,
}

/// Ligne du tiroir des fenêtres.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DrawerRow {
    Window { id: WinId, child: bool },
    NewServer,
}

/// Retire les séparateurs en tête, en fin et doublés.
fn clean_items(items: Vec<MenuItem>) -> Vec<MenuItem> {
    let mut out: Vec<MenuItem> = Vec::with_capacity(items.len());
    for it in items {
        if it.separator && out.last().is_none_or(|l| l.separator) {
            continue;
        }
        out.push(it);
    }
    while out.last().is_some_and(|l| l.separator) {
        out.pop();
    }
    out
}

/// Les menus de la barre de menus du bureau, sans ce qui n'a pas de sens sur un téléphone (barres, mosaïque,
/// liste des fenêtres : le tiroir la remplace).
fn mobile_menubar(model: Vec<(String, Vec<MenuItem>)>) -> Vec<MenuItem> {
    let hidden = [ids::TOOLBAR, ids::SWITCHBAR, ids::TREEBAR, ids::NICKLIST, ids::CASCADE, ids::TILE_H, ids::TILE_V, ids::ARRANGE, ids::NEXT];
    model
        .into_iter()
        .filter_map(|(title, items)| {
            let items: Vec<MenuItem> =
                items.into_iter().filter(|i| !(hidden.contains(&i.id) && !i.is_submenu()) && !(ids::WIN_BASE..ids::SCRIPT_BASE).contains(&i.id)).collect();
            let items = clean_items(items);
            (!items.is_empty()).then(|| MenuItem::submenu(&title, items))
        })
        .collect()
}

/// Un pas dans l'historique de saisie (`len` lignes, la plus récente à la fin) depuis `pos` (`None` : le
/// brouillon) ; `older` : vers les plus anciennes (↑). Renvoie la nouvelle position (`Some(None)` : retour au
/// brouillon), ou `None` s'il n'y a rien à faire.
fn step_history(len: usize, pos: Option<usize>, older: bool) -> Option<Option<usize>> {
    match (pos, older) {
        (None, true) => len.checked_sub(1).map(Some),
        (Some(i), true) => (i > 0 && i <= len).then(|| Some(i - 1)),
        (None, false) => None,
        (Some(i), false) => Some((i + 1 < len).then_some(i + 1)),
    }
}

/// Entrées du sous-menu « Déconnexion en arrière-plan » (durée en vigueur cochée).
fn background_items(lang: Lang, minutes: u32) -> Vec<MenuItem> {
    let mut v: Vec<MenuItem> = BG_CHOICES
        .iter()
        .map(|&m| {
            let label = if m == 0 { t(lang, "mobile.bg.never") } else { tf(lang, "mobile.bg.after", &[("n", &m.to_string())]) };
            MenuItem::new(M_BG_BASE + m, &label).radio(minutes == m)
        })
        .collect();
    let custom = !BG_CHOICES.contains(&minutes);
    let other = if custom { tf(lang, "mobile.bg.other_n", &[("n", &minutes.to_string())]) } else { t(lang, "mobile.bg.other") };
    v.push(MenuItem::separator());
    v.push(MenuItem::new(M_BG_OTHER, &other).radio(custom));
    v
}

/// Boîte « Autre durée » de la déconnexion en arrière-plan : un nombre de minutes (0 : jamais).
struct BackgroundDialog {
    minutes: u32,
    answer: Rc<Cell<Option<u32>>>,
}

impl DialogCtl for BackgroundDialog {
    fn kind(&self) -> Option<String> {
        Some("mobile.bg".into())
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        use crate::dialogs::{button, edit, spec, text};
        let w = 300;
        spec(
            &t(lang, "mobile.bg.title"),
            w,
            12 + 48 + 8 + 22 + 14 + 24 + 12,
            vec![
                text(1, (12, 12, w - 24, 48), &t(lang, "mobile.bg.prompt")),
                edit(2, (12, 68, 64, 22), &self.minutes.to_string()),
                text(5, (84, 71, w - 96, 16), &t(lang, "mobile.bg.minutes")),
                button(3, (w - 12 - 2 * 84 - 8, 104, 84, 24), &t(lang, "dlg.ok")).style("default ok"),
                button(4, (w - 12 - 84, 104, 84, 24), &t(lang, "dlg.cancel")).style("cancel"),
            ],
        )
    }

    fn init(&mut self, view: &mut DialogView, _cx: &mut Cx) {
        view.set_focus(2);
    }

    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, _cx: &mut Cx) -> Outcome {
        match msg {
            DialogMsg::Ok(_) | DialogMsg::Clicked(3) => match view.text(2).unwrap_or_default().trim().parse::<u32>() {
                Ok(m) if m <= BG_MAX_MIN => {
                    self.answer.set(Some(m));
                    Outcome::Close
                }
                // Pas un nombre (ou trop grand) : la boîte reste ouverte.
                _ => Outcome::Keep,
            },
            DialogMsg::Cancel | DialogMsg::Clicked(4) => Outcome::Close,
            _ => Outcome::Keep,
        }
    }
}

/// Début (octet) du dernier mot d'un texte.
fn last_word_start(text: &str) -> usize {
    text.char_indices().rev().find(|(_, c)| c.is_whitespace()).map_or(0, |(i, c)| i + c.len_utf8())
}

/// Réduction d'une boîte de taille `size` pour tenir dans `avail` (avec une petite marge).
fn dialog_zoom(size: (i32, i32), avail: (i32, i32)) -> f32 {
    let zw = (avail.0 - 8).max(1) as f32 / size.0.max(1) as f32;
    let zh = (avail.1 - 8).max(1) as f32 / size.1.max(1) as f32;
    zw.min(zh).clamp(MIN_DIALOG_ZOOM, MAX_DIALOG_ZOOM)
}

/// Rectangle `r` (pixels logiques de l'écran) en coordonnées d'une couche réduite de `z`.
fn unzoom_rect(r: Rect, z: f32) -> Rect {
    Rect::from_corners((r.x as f32 / z).ceil() as i32, (r.y as f32 / z).ceil() as i32, (r.right() as f32 / z).floor() as i32, (r.bottom() as f32 / z).floor() as i32)
}

fn unzoom(p: Point, z: f32) -> Point {
    Point::new((p.x as f32 / z).floor() as i32, (p.y as f32 / z).floor() as i32)
}

/// Point d'un toucher dans une boîte : s'il ne tombe sur aucun contrôle, le plus proche à moins de [`SNAP`]
/// pixels l'attire (zones tactiles élargies, ADR 0006). Renvoie le décalage à appliquer.
fn dialog_snap(d: &DialogView, p: Point) -> (i32, i32) {
    let client = d.client_rect();
    if !client.contains(p) {
        return (0, 0);
    }
    let mut best: Option<(i32, Point)> = None;
    for (i, c) in d.controls.iter().enumerate() {
        if !d.is_shown(i) {
            continue;
        }
        let r = c.rect.offset(client.x, client.y);
        let tappable = c.enabled
            && matches!(
                c.kind,
                ControlKind::Button | ControlKind::Check | ControlKind::Radio | ControlKind::Combo | ControlKind::Edit | ControlKind::Link
            );
        if r.contains(p) {
            if matches!(c.kind, ControlKind::Box | ControlKind::Text | ControlKind::Icon | ControlKind::Tab) {
                continue;
            }
            return (0, 0);
        }
        if !tappable || r.w < 4 || r.h < 4 {
            continue;
        }
        let inner = r.inset(1);
        let q = Point::new(p.x.clamp(inner.x, inner.right() - 1), p.y.clamp(inner.y, inner.bottom() - 1));
        let d2 = (q.x - p.x).pow(2) + (q.y - p.y).pow(2);
        if d2 <= SNAP * SNAP && best.is_none_or(|(b, _)| d2 < b) {
            best = Some((d2, q));
        }
    }
    best.map_or((0, 0), |(_, q)| (q.x - p.x, q.y - p.y))
}

impl Shell {
    // -----------------------------------------------------------------------------------------------------------
    // Géométrie.

    /// La fenêtre affichée.
    fn mobile_active(&self) -> Option<WinId> {
        self.mobile.active.filter(|w| self.views.contains_key(w)).or(self.client.active())
    }

    /// Zone utile de l'écran (hors barre d'état, barre de navigation, clavier, champ natif).
    fn mobile_safe(&self) -> Rect {
        let (top, bottom) = self.mobile.insets;
        Rect::from_corners(0, top, self.viewport.w, (self.viewport.h - bottom).max(top + 1))
    }

    fn top_bar(&self) -> Rect {
        Rect::new(0, self.mobile.insets.0, self.viewport.w, TOP_BAR_H)
    }

    fn top_button_rect(&self, b: TopButton) -> Rect {
        let bar = self.top_bar();
        let y = bar.y + 2;
        let h = TOP_BAR_H - 4;
        match b {
            TopButton::Windows => Rect::new(2, y, BTN_W, h),
            TopButton::Menu => Rect::new(bar.right() - 2 - 32, y, 32, h),
            TopButton::Nicks => Rect::new(bar.right() - 2 - 32 - 2 - BTN_W, y, BTN_W, h),
        }
    }

    fn has_nick_button(&self) -> bool {
        self.mobile_active().and_then(|a| self.client.window(a)).is_some_and(|w| w.kind == WinKind::Channel)
    }

    /// Hauteur de la ligne de saisie de Rust.
    fn editbox_h(&self) -> i32 {
        self.mobile_active().and_then(|a| self.views.get(&a)).map(|v| v.editbox.preferred_height(&self.res)).unwrap_or(22)
    }

    /// Zone du contenu de la fenêtre affichée (discussion ou liste, et ligne de saisie). Avec le champ natif,
    /// elle descend de la hauteur de la ligne de saisie de Rust, qui se trouve ainsi sous le champ natif.
    pub(crate) fn mobile_content_rect(&self) -> Rect {
        let safe = self.mobile_safe();
        let y0 = safe.y + TOP_BAR_H;
        let mut y1 = safe.bottom().max(y0 + 1);
        let has_editbox = self.mobile_active().and_then(|a| self.client.window(a)).is_some_and(views::has_editbox);
        if self.mobile.native_input && has_editbox {
            y1 += self.editbox_h();
        }
        Rect::from_corners(0, y0, self.viewport.w, y1)
    }

    fn drawer_rect(&self, d: Drawer) -> Rect {
        let safe = self.mobile_safe();
        let w = match d {
            Drawer::Windows => (self.viewport.w * 82 / 100).min(320),
            Drawer::Nicks => (self.viewport.w * 70 / 100).min(260),
        };
        match d {
            Drawer::Windows => Rect::new(0, safe.y, w, safe.h),
            Drawer::Nicks => Rect::new(self.viewport.w - w, safe.y, w, safe.h),
        }
    }

    /// Zone des lignes d'un tiroir (sous l'en-tête).
    fn drawer_rows_rect(&self, d: Drawer) -> Rect {
        let r = self.drawer_rect(d);
        Rect::from_corners(r.x, r.y + HEADER_H, r.right(), r.bottom())
    }

    fn drawer_rows(&self) -> Vec<DrawerRow> {
        let mut rows: Vec<DrawerRow> = self
            .ordered_windows()
            .into_iter()
            .filter_map(|id| {
                let w = self.client.window(id)?;
                let child = w.kind != WinKind::Status && w.conn.is_some();
                Some(DrawerRow::Window { id, child })
            })
            .collect();
        rows.push(DrawerRow::NewServer);
        rows
    }

    fn drawer_row_h(d: Drawer) -> i32 {
        match d {
            Drawer::Windows => ROW_H,
            Drawer::Nicks => NICK_ROW_H,
        }
    }

    fn drawer_len(&self, d: Drawer) -> usize {
        match d {
            Drawer::Windows => self.drawer_rows().len(),
            Drawer::Nicks => self.mobile_active().and_then(|a| self.views.get(&a)).map_or(0, |v| v.nicks.entries().len()),
        }
    }

    fn drawer_max_scroll(&self, d: Drawer) -> i32 {
        let content = self.drawer_len(d) as i32 * Self::drawer_row_h(d);
        (content - self.drawer_rows_rect(d).h).max(0)
    }

    /// Ligne du tiroir sous un point.
    fn drawer_row_at(&self, d: Drawer, p: Point) -> Option<usize> {
        let r = self.drawer_rows_rect(d);
        if !r.contains(p) {
            return None;
        }
        let i = ((p.y - r.y + self.mobile.drawer_scroll) / Self::drawer_row_h(d)) as usize;
        (i < self.drawer_len(d)).then_some(i)
    }

    // --- Menus au toucher -------------------------------------------------------------------------------------

    fn menu_rect(&self) -> Option<Rect> {
        let m = self.mobile.menu.as_ref()?;
        let l = m.level();
        let safe = self.mobile_safe().inset(4);
        let mut w = 0;
        let mut h = 2 * MENU_BORDER;
        if let Some(title) = m.header() {
            w = w.max(self.res.fonts.measure(&plain(title), TextStyle::UI_BOLD) + 40);
            h += MENU_ROW_H;
        }
        for it in &l.items {
            if it.separator {
                h += MENU_SEP_H;
            } else {
                h += MENU_ROW_H;
                w = w.max(self.res.fonts.measure(&plain(&it.label), TextStyle::UI) + 28 + 28);
            }
        }
        let w = (w + 2 * MENU_BORDER).clamp(200.min(safe.w), safe.w);
        let h = h.min(safe.h);
        let mut x = if m.right_aligned { m.anchor.x - w } else { m.anchor.x };
        let mut y = m.anchor.y;
        if y + h > safe.bottom() {
            y = if m.anchor.y - h >= safe.y { m.anchor.y - h } else { safe.bottom() - h };
        }
        x = x.clamp(safe.x, (safe.right() - w).max(safe.x));
        y = y.clamp(safe.y, (safe.bottom() - h).max(safe.y));
        Some(Rect::new(x, y, w, h))
    }

    /// Lignes du niveau affiché : (index de l'entrée, ou `None` pour la ligne de titre, rectangle), défilement
    /// compris.
    fn menu_rows(&self) -> Vec<(Option<usize>, Rect)> {
        let (Some(m), Some(r)) = (self.mobile.menu.as_ref(), self.menu_rect()) else { return Vec::new() };
        let l = m.level();
        let mut y = r.y + MENU_BORDER - l.scroll;
        let x = r.x + MENU_BORDER;
        let w = r.w - 2 * MENU_BORDER;
        let mut out = Vec::new();
        if m.header().is_some() {
            out.push((None, Rect::new(x, y, w, MENU_ROW_H)));
            y += MENU_ROW_H;
        }
        for (i, it) in l.items.iter().enumerate() {
            let h = if it.separator { MENU_SEP_H } else { MENU_ROW_H };
            out.push((Some(i), Rect::new(x, y, w, h)));
            y += h;
        }
        out
    }

    fn menu_max_scroll(&self) -> i32 {
        let (Some(m), Some(r)) = (self.mobile.menu.as_ref(), self.menu_rect()) else { return 0 };
        let l = m.level();
        let content: i32 = m.header().map_or(0, |_| MENU_ROW_H) + l.items.iter().map(|i| if i.separator { MENU_SEP_H } else { MENU_ROW_H }).sum::<i32>();
        (content - (r.h - 2 * MENU_BORDER)).max(0)
    }

    /// Ligne du menu sous un point : `Some(None)` = ligne de titre (retour).
    fn menu_row_at(&self, p: Point) -> Option<Option<usize>> {
        let r = self.menu_rect()?.inset(MENU_BORDER);
        if !r.contains(p) {
            return None;
        }
        self.menu_rows().into_iter().find(|(_, rr)| rr.contains(p)).map(|(i, _)| i)
    }

    /// Le menu ⋮ : connexion, rejoindre un canal, puis les menus du bureau en sous-menus.
    fn open_main_menu(&mut self, p: &dyn ShellPlatform) {
        let lang = self.language();
        let model = self.menu_model();
        let connected = self
            .mobile_active()
            .and_then(|a| self.client.window(a))
            .and_then(|w| w.conn)
            .and_then(|c| self.client.conn_info(c))
            .is_some_and(|c| c.state != ConnState::Disconnected);
        let mut items = vec![
            if connected { MenuItem::new(ids::DISCONNECT, &t(lang, "file.disconnect")) } else { MenuItem::new(ids::CONNECT, &t(lang, "file.connect")) },
            MenuItem::new(M_JOIN, &t(lang, "mobile.join")),
            MenuItem::separator(),
        ];
        items.extend(mobile_menubar(model));
        // Téléphone : taille de l'affichage, déconnexion en arrière-plan, vibration.
        let mut phone = Vec::new();
        if let Some(cur) = p.ui_scale() {
            let sizes = SCALES.iter().map(|s| MenuItem::new(M_SCALE + (s * 100.0).round() as u32, &format!("{s}×")).radio((cur - s).abs() < 0.01)).collect();
            phone.push(MenuItem::submenu(&t(lang, "mobile.display_size"), sizes));
        }
        if let Some(pp) = &self.mobile.phone {
            phone.push(MenuItem::submenu(&t(lang, "mobile.bg"), background_items(lang, pp.background_disconnect_min)));
            phone.push(MenuItem::new(M_VIBRATE, &t(lang, "mobile.vibrate")).checked(pp.vibrate));
        }
        if !phone.is_empty() {
            items.push(MenuItem::separator());
            items.push(MenuItem::submenu(&t(lang, "mobile.phone"), phone));
        }
        let bar = self.top_bar();
        self.mobile.menu = Some(TouchMenu::new(MenuOrigin::Main, items, Point::new(bar.right() - 2, bar.bottom()), true));
    }

    /// Menu d'une fenêtre au toucher (le menu du bureau sans réduire / agrandir).
    fn window_menu_items(&self, win: WinId) -> Vec<MenuItem> {
        let lang = self.language();
        let _ = win;
        vec![
            MenuItem::new(win_menu::CLEAR, &t(lang, "swb.clear")),
            MenuItem::new(win_menu::LOG, &t(lang, "swb.log")),
            MenuItem::separator(),
            MenuItem::new(win_menu::CLOSE, &t(lang, "swb.close")),
        ]
    }

    /// Un menu surgissant ouvert par le shell (scripts, fenêtre) devient un menu au toucher.
    fn adopt_popup(&mut self) {
        let Some((menu, src)) = self.popup.take() else { return };
        let at = menu.rect().origin();
        let label = |w: WinId| self.client.window(w).map(|x| short_label(x, &self.client));
        let (items, caption) = match &src {
            PopupSrc::Script { kind: PopupKind::Nicklist, tree, selected, .. } => (menus::from_script(tree, 0), Some(selected.join(", "))),
            PopupSrc::Script { tree, win, .. } => (menus::from_script(tree, 0), label(*win)),
            PopupSrc::Window(w) => (self.window_menu_items(*w), label(*w)),
            PopupSrc::Notify { nick, .. } => (self.notify_menu_items(), Some(nick.clone())),
        };
        if !items.is_empty() {
            self.mobile.menu = Some(TouchMenu::new(MenuOrigin::Popup(src), items, at, false).with_caption(caption));
            self.mobile.pressed_row = None;
        }
    }

    fn menu_tap(&mut self, row: Option<usize>, p: &mut dyn ShellPlatform) {
        let Some(m) = self.mobile.menu.as_mut() else { return };
        let Some(i) = row else {
            // Ligne de titre d'un sous-menu : retour au niveau précédent.
            if m.levels.len() > 1 {
                m.levels.pop();
            }
            return;
        };
        let Some(it) = m.level().items.get(i).cloned() else { return };
        if it.separator || !it.enabled {
            return;
        }
        if it.id == M_COPY_LINE && !it.is_submenu() {
            self.mobile.menu = None;
            if let Some(text) = self.mobile.copy_text.take() {
                p.clipboard_set(&text);
            }
            return;
        }
        if it.is_submenu() {
            m.levels.push(MenuLevel { title: Some(it.label.clone()), items: clean_items(it.submenu), scroll: 0 });
            return;
        }
        let m = self.mobile.menu.take().expect("menu ouvert");
        match m.origin {
            MenuOrigin::Main => self.main_menu_action(it.id, p),
            MenuOrigin::History(win) => {
                let line = self.client.window(win).and_then(|w| w.history.get((it.id as usize).wrapping_sub(1))).cloned();
                if let Some(line) = line {
                    self.put_input_text(win, line);
                }
            }
            MenuOrigin::Popup(src) => {
                self.popup_activated(it.id, src, p);
                self.adopt_popup();
            }
        }
    }

    fn main_menu_action(&mut self, id: u32, p: &mut dyn ShellPlatform) {
        match id {
            M_JOIN => {
                if let Some(w) = self.mobile_active() {
                    self.open_dialog(Box::new(crate::dialogs::JoinPrompt { win: w }), p);
                }
            }
            id if (M_SCALE + 100..M_SCALE + 1000).contains(&id) => {
                p.set_ui_scale((id - M_SCALE) as f32 / 100.0);
                p.request_repaint();
            }
            id if (M_BG_BASE..=M_BG_BASE + BG_MAX_MIN).contains(&id) => self.set_background_minutes(id - M_BG_BASE),
            M_BG_OTHER => {
                let minutes = self.mobile.phone.as_ref().map_or(0, |pp| pp.background_disconnect_min);
                let answer = self.mobile.bg_answer.clone();
                self.open_dialog(Box::new(BackgroundDialog { minutes, answer }), p);
            }
            M_VIBRATE => {
                if let Some(pp) = self.mobile.phone.as_mut() {
                    pp.vibrate = !pp.vibrate;
                    self.mobile.phone_changed = true;
                }
            }
            id => self.menu_action(id, p),
        }
        self.adopt_popup();
    }

    // -----------------------------------------------------------------------------------------------------------
    // Boîtes de dialogue réduites.

    /// Réduction de chaque boîte ouverte, et place de chacune (centrée dans la zone utile).
    fn fit_dialogs(&mut self) -> Vec<f32> {
        let safe = self.mobile_safe();
        let mut zooms = Vec::with_capacity(self.modals.dialogs.len());
        for (_, d) in self.modals.dialogs.iter_mut() {
            let z = dialog_zoom((d.rect.w, d.rect.h), (safe.w, safe.h));
            d.center_in(unzoom_rect(safe, z));
            zooms.push(z);
        }
        zooms
    }

    fn top_dialog_zoom(&mut self) -> f32 {
        self.fit_dialogs().last().copied().unwrap_or(1.0)
    }

    /// Un événement pour la boîte du dessus, en coordonnées de l'écran.
    fn dialog_input(&mut self, input: &Input, snap: (i32, i32), p: &mut dyn ShellPlatform) {
        let z = self.top_dialog_zoom();
        let ev = match input {
            Input::MouseDown { pos, button, mods, clicks } => Input::MouseDown { pos: unzoom(*pos, z).offset(snap.0, snap.1), button: *button, mods: *mods, clicks: *clicks },
            Input::MouseUp { pos, button, mods } => Input::MouseUp { pos: unzoom(*pos, z).offset(snap.0, snap.1), button: *button, mods: *mods },
            Input::MouseMove { pos } => Input::MouseMove { pos: unzoom(*pos, z).offset(snap.0, snap.1) },
            Input::Wheel { pos, dx, dy, mods } => Input::Wheel { pos: unzoom(*pos, z), dx: *dx, dy: *dy, mods: *mods },
            other => other.clone(),
        };
        let bounds = unzoom_rect(self.mobile_safe(), z);
        if let Some((key, msg)) = self.modals.event(&ev, &self.res, p, bounds) {
            self.dialog_msg(key, msg, p);
            self.adopt_popup();
        }
        if let Some(m) = self.mobile.bg_answer.take() {
            self.set_background_minutes(m);
        }
        p.request_repaint();
    }

    /// Vrai si le contrôle qui a le focus dans la boîte du dessus attend du texte (le clavier du système doit
    /// s'ouvrir).
    pub fn mobile_text_focus(&self) -> bool {
        let Some((_, d)) = self.modals.dialogs.last() else { return false };
        let Some(id) = d.focus() else { return false };
        use alpirc_ui::dialog::Widget;
        d.controls.iter().find(|c| c.id == id).is_some_and(|c| match &c.widget {
            Widget::Edit(e) => e.enabled && !e.core.read_only,
            Widget::TextArea(a) => a.enabled && !a.core.read_only,
            Widget::Combo(cb) => cb.editable && cb.enabled,
            _ => false,
        })
    }

    // -----------------------------------------------------------------------------------------------------------
    // Interface avec la plateforme (Android, simulateur).

    /// Marges imposées par le système (Android : barres, clavier, champ de saisie natif), en pixels logiques.
    pub fn set_insets(&mut self, top: i32, bottom: i32) {
        self.mobile.insets = (top.max(0), bottom.max(0));
    }

    /// La plateforme a un champ de saisie natif (Android) : la ligne de saisie de Rust est cachée.
    pub fn set_native_input(&mut self, on: bool) {
        self.mobile.native_input = on;
    }

    /// Le champ de saisie natif doit-il être affiché ? (Fenêtre avec ligne de saisie, ni boîte ni tiroir.)
    pub fn mobile_native_input_visible(&self) -> bool {
        self.mobile.native_input
            && self.modals.is_empty()
            && self.mobile.drawer.is_none()
            && self.mobile_active().and_then(|a| self.client.window(a)).is_some_and(views::has_editbox)
    }

    /// Texte d'aide du champ de saisie natif (« Message dans #lyon »).
    pub fn mobile_input_hint(&self) -> String {
        let lang = self.language();
        match self.mobile_active().and_then(|a| self.client.window(a)) {
            Some(w) if w.kind == WinKind::Status => t(lang, "mobile.hint.status"),
            Some(w) => tf(lang, "mobile.hint", &[("win", &w.name)]),
            None => String::new(),
        }
    }

    /// Texte envoyé par le champ de saisie natif : comme Entrée dans la ligne de saisie du bureau (commande ou
    /// message dans la fenêtre affichée) ; plusieurs lignes passent par la confirmation du collage.
    pub fn mobile_submit(&mut self, text: &str, p: &mut dyn ShellPlatform) {
        let Some(win) = self.mobile_active() else { return };
        self.mobile.tab = None;
        self.mobile.history = None;
        let lines: Vec<String> = text.split('\n').map(|l| l.trim_end_matches('\r').to_string()).filter(|l| !l.trim().is_empty()).collect();
        if let Some(v) = self.views.get_mut(&win) {
            if lines.len() == 1 {
                v.editbox.history.push(lines[0].clone());
            }
            v.chat.scroll_to_bottom();
        }
        match lines.len() {
            0 => {}
            1 => self.edit_msg(win, EditMsg::Submit(lines.into_iter().next().unwrap_or_default()), p),
            _ => {
                self.edit_msg(win, EditMsg::PasteLines(lines), p);
                self.adopt_popup();
            }
        }
    }

    /// Complétion (bouton Tab du champ natif) : le dernier mot de `text` complété (pseudo, canal, commande), ou
    /// la complétion suivante si `text` est la dernière rendue. `None` : rien à compléter.
    pub fn mobile_complete(&mut self, text: &str) -> Option<String> {
        let win = self.mobile_active()?;
        let (word, start, idx) = match &self.mobile.tab {
            Some((w, word, start, last, i)) if *w == win && last == text && *start <= text.len() => (word.clone(), *start, i + 1),
            _ => {
                let s = last_word_start(text);
                (text[s..].to_string(), s, 0)
            }
        };
        if word.is_empty() {
            return None;
        }
        let rep = self.client.tab_complete(win, &word, idx)?;
        let new = format!("{}{}", &text[..start], rep);
        self.mobile.tab = Some((win, word, start, new.clone(), idx));
        Some(new)
    }

    // --- Réglages du téléphone, déconnexion en arrière-plan ----------------------------------------------------

    /// Durée la plus longue de la déconnexion en arrière-plan (minutes).
    pub const MOBILE_BG_MAX_MIN: u32 = BG_MAX_MIN;

    /// Les réglages du téléphone (lus par la plateforme) : déconnexion après `background_disconnect_min` minutes
    /// en arrière-plan (0 : jamais), vibration des bips. Le menu ⋮ montre alors la section Téléphone.
    pub fn set_phone_prefs(&mut self, background_disconnect_min: u32, vibrate: bool) {
        self.mobile.phone = Some(PhonePrefs { background_disconnect_min: background_disconnect_min.min(BG_MAX_MIN), vibrate });
        self.mobile.phone_changed = false;
    }

    /// Les réglages du téléphone (minutes, vibration), s'ils ont été changés depuis le menu (la plateforme les
    /// enregistre).
    pub fn take_phone_prefs_change(&mut self) -> Option<(u32, bool)> {
        if !std::mem::take(&mut self.mobile.phone_changed) {
            return None;
        }
        self.mobile.phone.as_ref().map(|pp| (pp.background_disconnect_min, pp.vibrate))
    }

    fn set_background_minutes(&mut self, minutes: u32) {
        if let Some(pp) = self.mobile.phone.as_mut() {
            pp.background_disconnect_min = minutes.min(BG_MAX_MIN);
            self.mobile.phone_changed = true;
        }
    }

    /// Déconnexion après N minutes en arrière-plan (ADR 0006) : QUIT avec le message habituel sur chaque
    /// connexion ouverte. Elles sont notées pour [`Shell::mobile_background_reconnect`]. Renvoie leur nombre.
    pub fn mobile_background_disconnect(&mut self, p: &mut dyn ShellPlatform) -> usize {
        let now = p.now_ms();
        let statuses: Vec<WinId> = self
            .client
            .conns()
            .into_iter()
            .filter_map(|c| self.client.conn_info(c))
            .filter(|i| i.state != ConnState::Disconnected)
            .map(|i| i.status_window)
            .collect();
        for st in &statuses {
            self.client.disconnect(*st, None, now);
        }
        log::info!("déconnexion en arrière-plan : {} connexion(s)", statuses.len());
        self.mobile.bg_disconnected = statuses.clone();
        self.after(p);
        statuses.len()
    }

    /// Retour au premier plan après une déconnexion en arrière-plan : reconnexion aux mêmes serveurs (`/server`
    /// dans chaque fenêtre Status restée fermée). Renvoie le nombre de reconnexions.
    pub fn mobile_background_reconnect(&mut self, p: &mut dyn ShellPlatform) -> usize {
        let now = p.now_ms();
        let mut n = 0;
        for st in std::mem::take(&mut self.mobile.bg_disconnected) {
            let closed = self.client.window(st).and_then(|w| w.conn).and_then(|c| self.client.conn_info(c)).is_some_and(|c| c.state == ConnState::Disconnected);
            if closed {
                self.client.command(st, "/server", now);
                n += 1;
            }
        }
        self.after(p);
        n
    }

    // --- Historique de la saisie ------------------------------------------------------------------------------

    /// Historique du champ natif, comme ↑ / ↓ dans la ligne de saisie du bureau : `older` remonte vers les lignes
    /// plus anciennes de la fenêtre affichée, sinon redescend jusqu'au brouillon. `current` : texte du champ.
    /// Renvoie le texte à y mettre (`None` : rien ne change).
    pub fn mobile_history_step(&mut self, older: bool, current: &str) -> Option<String> {
        let win = self.mobile_active()?;
        let hist = &self.client.window(win)?.history;
        let pos = self.mobile.history.as_ref().filter(|h| h.0 == win).map(|h| h.1);
        match step_history(hist.len(), pos, older)? {
            Some(i) => {
                let line = hist[i].clone();
                let draft = match self.mobile.history.take() {
                    Some((w, _, d)) if w == win => d,
                    _ => current.to_string(),
                };
                self.mobile.history = Some((win, i, draft));
                Some(line)
            }
            None => self.mobile.history.take().map(|(_, _, d)| d),
        }
    }

    /// Bouton Historique du champ natif : la liste des dernières lignes tapées dans la fenêtre affichée ; la
    /// ligne choisie revient par [`Shell::take_native_input_text`].
    pub fn mobile_history_menu(&mut self, p: &mut dyn ShellPlatform) {
        let lang = self.language();
        let Some(win) = self.mobile_active() else { return };
        let Some(w) = self.client.window(win) else { return };
        let mut items: Vec<MenuItem> =
            w.history.iter().enumerate().rev().take(HISTORY_MENU_MAX).map(|(i, l)| MenuItem::new(i as u32 + 1, &l.replace('&', "&&"))).collect();
        if items.is_empty() {
            items.push(MenuItem::new(0, &t(lang, "menu.empty")).disabled());
        }
        let safe = self.mobile_safe();
        self.mobile.drawer = None;
        let menu = TouchMenu::new(MenuOrigin::History(win), items, Point::new(safe.x + 4, safe.bottom()), false);
        self.mobile.menu = Some(menu.with_caption(Some(t(lang, "mobile.history"))));
        p.request_repaint();
    }

    /// Texte à mettre dans le champ natif (ligne choisie dans l'historique), une fois.
    pub fn take_native_input_text(&mut self) -> Option<String> {
        self.mobile.native_text.take()
    }

    fn put_input_text(&mut self, win: WinId, text: String) {
        self.mobile.history = None;
        if self.mobile.native_input {
            self.mobile.native_text = Some(text);
        } else if let Some(v) = self.views.get_mut(&win) {
            v.editbox.set_text(&text);
        }
    }

    /// Bouton Retour d'Android : ferme le menu, la boîte, le tiroir ouverts. `false` : rien à fermer (la
    /// plateforme passe alors l'appli en arrière-plan, connexions gardées).
    pub fn mobile_back(&mut self, p: &mut dyn ShellPlatform) -> bool {
        self.mobile.touch = None;
        self.mobile.fling = None;
        if let Some(m) = self.mobile.menu.as_mut() {
            m.levels.pop();
            if m.levels.is_empty() {
                self.mobile.menu = None;
            }
            p.request_repaint();
            return true;
        }
        if !self.modals.is_empty() {
            self.dialog_input(&Input::key(Key::Escape, Mods::NONE), (0, 0), p);
            return true;
        }
        if self.mobile.drawer.take().is_some() {
            p.request_repaint();
            return true;
        }
        false
    }

    /// Ouvre ou ferme un tiroir.
    pub(crate) fn mobile_toggle_drawer(&mut self, d: Drawer, p: &mut dyn ShellPlatform) {
        self.mobile.menu = None;
        if self.mobile.drawer == Some(d) {
            self.mobile.drawer = None;
        } else if d == Drawer::Windows || self.has_nick_button() {
            self.mobile.drawer = Some(d);
            self.mobile.drawer_scroll = 0;
            if d == Drawer::Windows {
                // La fenêtre affichée en vue.
                let rows = self.drawer_rows();
                let active = self.mobile_active();
                if let Some(i) = rows.iter().position(|r| matches!(r, DrawerRow::Window { id, .. } if Some(*id) == active)) {
                    let h = self.drawer_rows_rect(d).h;
                    let y = i as i32 * ROW_H;
                    if y + ROW_H > h {
                        self.mobile.drawer_scroll = (y + ROW_H - h + ROW_H).min(self.drawer_max_scroll(d));
                    }
                }
            }
        }
        p.request_repaint();
    }

    // -----------------------------------------------------------------------------------------------------------
    // Événements : gestes.

    fn hit(&self, pos: Point) -> Target {
        if !self.modals.is_empty() {
            return Target::Dialog;
        }
        if self.mobile.menu.is_some() {
            return if self.menu_rect().is_some_and(|r| r.contains(pos)) { Target::Menu } else { Target::OutsideMenu };
        }
        if let Some(d) = self.mobile.drawer {
            return if self.drawer_rect(d).contains(pos) { Target::Drawer } else { Target::Scrim };
        }
        let bar = self.top_bar();
        if pos.y < bar.bottom() {
            for b in [TopButton::Windows, TopButton::Nicks, TopButton::Menu] {
                if b == TopButton::Nicks && !self.has_nick_button() {
                    continue;
                }
                // Zone tactile un peu plus large que le bouton dessiné.
                if self.top_button_rect(b).inset(-2).contains(pos) || (b == TopButton::Windows && pos.x < BTN_W + 4) {
                    return Target::Button(b);
                }
            }
            return Target::Title;
        }
        if !self.mobile.native_input {
            if let Some(er) = self.mobile_active().and_then(|a| self.window_rects(a)).and_then(|r| r.editbox) {
                if er.contains(pos) {
                    return Target::Editbox;
                }
            }
        }
        Target::Content
    }

    pub(crate) fn mobile_input(&mut self, input: Input, p: &mut dyn ShellPlatform) {
        self.res.min_row_h = TOUCH_ROW_H;
        match input {
            Input::MouseDown { pos, button: MouseButton::Left, .. } => self.touch_down(pos, p),
            Input::MouseMove { pos } => self.touch_move(pos, p),
            Input::MouseUp { pos, button: MouseButton::Left, .. } => self.touch_up(pos, p),
            // Simulateur : le bouton droit fait l'appui long.
            Input::MouseDown { pos, button: MouseButton::Right, .. } => {
                self.mobile.touch = None;
                match self.hit(pos) {
                    Target::Dialog => self.dialog_input(&input, (0, 0), p),
                    Target::Content => self.content_long_press(pos, p),
                    Target::Drawer => self.drawer_long_press(pos, p),
                    _ => {}
                }
            }
            Input::MouseUp { .. } => {}
            Input::Wheel { pos, dx, dy, .. } => match self.hit(pos) {
                Target::Dialog => self.dialog_input(&input, (0, 0), p),
                Target::Content | Target::Editbox => self.content_wheel(pos, dx, dy, p),
                Target::Drawer => {
                    if let Some(d) = self.mobile.drawer {
                        let max = self.drawer_max_scroll(d);
                        self.mobile.drawer_scroll = (self.mobile.drawer_scroll - (dy * 3.0 * ROW_H as f32) as i32).clamp(0, max);
                        p.request_repaint();
                    }
                }
                Target::Menu => self.scroll_menu(dy * 3.0 * MENU_ROW_H as f32, p),
                _ => {}
            },
            Input::Tick => {
                self.touch_tick(p);
                if !self.modals.is_empty() {
                    self.dialog_input(&Input::Tick, (0, 0), p);
                } else if let Some(w) = self.mobile_active() {
                    self.content_event(w, &Input::Tick, p);
                }
            }
            Input::Key { key: Key::Escape, .. } if self.mobile.menu.is_some() || (self.modals.is_empty() && self.mobile.drawer.is_some()) => {
                self.mobile_back(p);
            }
            // Toucher annulé par le système (ou souris sortie de la fenêtre du simulateur).
            Input::MouseLeave => self.touch_cancel(p),
            other => {
                if self.mobile.menu.is_some() || (self.mobile.drawer.is_some() && self.modals.is_empty()) {
                    return;
                }
                if !self.modals.is_empty() {
                    self.dialog_input(&other, (0, 0), p);
                } else if let Some(w) = self.mobile_active() {
                    self.content_event(w, &other, p);
                    self.adopt_popup();
                }
            }
        }
    }

    fn touch_down(&mut self, pos: Point, p: &mut dyn ShellPlatform) {
        let now = p.now_ms();
        self.mobile.fling = None;
        self.mobile.scroll_acc = 0.0;
        let target = self.hit(pos);
        let mut snap = (0, 0);
        let mut edge = None;
        let mut dialog_list = false;
        match target {
            Target::Dialog => {
                let z = self.top_dialog_zoom();
                if let Some((_, d)) = self.modals.dialogs.last() {
                    let q = unzoom(pos, z);
                    snap = dialog_snap(d, q);
                    let client = d.client_rect();
                    dialog_list = d.controls.iter().enumerate().any(|(i, c)| {
                        d.is_shown(i)
                            && matches!(c.kind, ControlKind::List | ControlKind::ListView | ControlKind::Tree | ControlKind::TextArea)
                            && c.rect.offset(client.x, client.y).contains(q)
                    });
                }
                let clicks = self.tap_count(pos, now);
                let down = Input::MouseDown { pos, button: MouseButton::Left, mods: Mods::NONE, clicks };
                self.dialog_input(&down, snap, p);
            }
            Target::Editbox => {
                if let Some(w) = self.mobile_active() {
                    self.content_event(w, &Input::MouseDown { pos, button: MouseButton::Left, mods: Mods::NONE, clicks: 1 }, p);
                }
            }
            Target::Button(b) => self.mobile.pressed_button = Some(b),
            Target::Drawer => {
                if let Some(d) = self.mobile.drawer {
                    self.mobile.pressed_row = self.drawer_row_at(d, pos);
                }
            }
            Target::Menu => self.mobile.pressed_row = self.menu_row_at(pos).flatten(),
            Target::Content => {
                if pos.x < EDGE {
                    edge = Some(Drawer::Windows);
                } else if pos.x >= self.viewport.w - EDGE && self.has_nick_button() {
                    edge = Some(Drawer::Nicks);
                }
            }
            _ => {}
        }
        let mut samples = VecDeque::new();
        samples.push_back((now, pos.x, pos.y));
        self.mobile.touch = Some(Touch { target, start: pos, last: pos, t0: now, moved: false, long_done: false, scrolling: false, samples, edge, snap, dialog_list });
        if matches!(target, Target::Content | Target::Drawer) {
            p.schedule_tick(now + LONG_PRESS_MS);
        }
        p.request_repaint();
    }

    fn touch_move(&mut self, pos: Point, p: &mut dyn ShellPlatform) {
        let now = p.now_ms();
        let Some(t) = self.mobile.touch.as_mut() else { return };
        let (dx, dy) = (pos.x - t.start.x, pos.y - t.start.y);
        if !t.moved && (dx.abs() > SLOP || dy.abs() > SLOP) {
            t.moved = true;
        }
        let step = (pos.x - t.last.x, pos.y - t.last.y);
        t.last = pos;
        t.samples.push_back((now, pos.x, pos.y));
        while t.samples.len() > 2 && t.samples.front().is_some_and(|s| now - s.0 > 100) {
            t.samples.pop_front();
        }
        let (target, moved, long_done, edge, snap, dialog_list) = (t.target, t.moved, t.long_done, t.edge, t.snap, t.dialog_list);
        match target {
            // Liste d'une boîte : glisser fait défiler (molette), comme la discussion.
            Target::Dialog if dialog_list && moved => {
                if let Some(t) = self.mobile.touch.as_mut() {
                    t.scrolling = true;
                }
                let z = self.top_dialog_zoom();
                let row_h = (self.res.ui_line() as f32 * z).max(1.0);
                self.mobile.scroll_acc += step.1 as f32;
                let rows = (self.mobile.scroll_acc / row_h).trunc();
                if rows != 0.0 {
                    self.mobile.scroll_acc -= rows * row_h;
                    let start = self.mobile.touch.as_ref().map_or(pos, |t| t.start);
                    self.dialog_input(&Input::Wheel { pos: start, dx: 0.0, dy: rows / 3.0, mods: Mods::NONE }, snap, p);
                }
            }
            Target::Dialog => self.dialog_input(&Input::MouseMove { pos }, snap, p),
            Target::Editbox => {
                if let Some(w) = self.mobile_active() {
                    self.content_event(w, &Input::MouseMove { pos }, p);
                }
            }
            Target::Button(b) => {
                let inside = self.top_button_rect(b).inset(-8).contains(pos);
                let want = inside.then_some(b);
                if self.mobile.pressed_button != want {
                    self.mobile.pressed_button = want;
                    p.request_repaint();
                }
            }
            Target::Content if moved && !long_done => {
                if let Some(d) = edge {
                    let open = match d {
                        Drawer::Windows => dx >= EDGE_OPEN,
                        Drawer::Nicks => dx <= -EDGE_OPEN,
                    };
                    if open && dx.abs() > dy.abs() {
                        self.mobile.touch = None;
                        self.mobile_toggle_drawer(d, p);
                        return;
                    }
                    if dy.abs() <= SLOP {
                        return;
                    }
                }
                if let Some(t) = self.mobile.touch.as_mut() {
                    t.scrolling = true;
                }
                self.scroll_content(step.0 as f32, step.1 as f32, pos, p);
            }
            Target::Drawer if moved && !long_done => {
                self.mobile.pressed_row = None;
                if let Some(d) = self.mobile.drawer {
                    let max = self.drawer_max_scroll(d);
                    self.mobile.drawer_scroll = (self.mobile.drawer_scroll - step.1).clamp(0, max);
                    if let Some(t) = self.mobile.touch.as_mut() {
                        t.scrolling = true;
                    }
                    p.request_repaint();
                }
            }
            Target::Menu if moved => {
                self.mobile.pressed_row = None;
                if let Some(t) = self.mobile.touch.as_mut() {
                    t.scrolling = true;
                }
                self.scroll_menu(step.1 as f32, p);
            }
            _ => {}
        }
    }

    fn touch_up(&mut self, pos: Point, p: &mut dyn ShellPlatform) {
        let now = p.now_ms();
        let Some(t) = self.mobile.touch.take() else { return };
        let pressed_row = self.mobile.pressed_row.take();
        let pressed_button = self.mobile.pressed_button.take();
        p.request_repaint();
        match t.target {
            Target::Dialog => {
                // Après un défilement, le doigt est relâché là où il s'était posé (pas de sélection glissée).
                let at = if t.scrolling { t.start } else { pos };
                self.dialog_input(&Input::MouseUp { pos: at, button: MouseButton::Left, mods: Mods::NONE }, t.snap, p);
            }
            Target::Editbox => {
                if let Some(w) = self.mobile_active() {
                    self.content_event(w, &Input::MouseUp { pos, button: MouseButton::Left, mods: Mods::NONE }, p);
                }
            }
            Target::Button(b) => {
                if pressed_button == Some(b) {
                    match b {
                        TopButton::Windows => self.mobile_toggle_drawer(Drawer::Windows, p),
                        TopButton::Nicks => self.mobile_toggle_drawer(Drawer::Nicks, p),
                        TopButton::Menu => {
                            self.mobile.drawer = None;
                            self.open_main_menu(p);
                        }
                    }
                }
            }
            Target::Content if !t.long_done => {
                if t.scrolling {
                    self.start_fling(&t, Scroller::Content, now, p);
                } else if !t.moved {
                    self.content_tap(t.start, now, p);
                }
            }
            Target::Content => {}
            Target::Drawer => {
                if t.scrolling {
                    self.start_fling(&t, Scroller::Drawer, now, p);
                } else if !t.moved && !t.long_done {
                    self.drawer_tap(t.start, pressed_row, p);
                }
            }
            Target::Scrim => {
                if !t.moved {
                    self.mobile.drawer = None;
                }
            }
            Target::Menu => {
                if t.scrolling {
                    self.start_fling(&t, Scroller::Menu, now, p);
                } else if !t.moved {
                    if let Some(row) = self.menu_row_at(t.start) {
                        self.menu_tap(row, p);
                    }
                }
            }
            Target::OutsideMenu => {
                if !t.moved {
                    self.mobile.menu = None;
                }
            }
            Target::Title => {}
        }
    }

    /// Le toucher est annulé : rien n'est déclenché ; les boîtes et la ligne de saisie relâchent ce qu'elles
    /// tenaient.
    fn touch_cancel(&mut self, p: &mut dyn ShellPlatform) {
        self.mobile.pressed_row = None;
        self.mobile.pressed_button = None;
        let away = Point::new(-10_000, -10_000);
        match self.mobile.touch.take().map(|t| (t.target, t.snap)) {
            Some((Target::Dialog, snap)) => self.dialog_input(&Input::MouseUp { pos: away, button: MouseButton::Left, mods: Mods::NONE }, snap, p),
            Some((Target::Editbox, _)) => {
                if let Some(w) = self.mobile_active() {
                    self.content_event(w, &Input::MouseUp { pos: away, button: MouseButton::Left, mods: Mods::NONE }, p);
                }
            }
            _ => {}
        }
        if let Some(w) = self.mobile_active() {
            self.content_event(w, &Input::MouseLeave, p);
        }
        p.request_repaint();
    }

    fn touch_tick(&mut self, p: &mut dyn ShellPlatform) {
        let now = p.now_ms();
        if let Some(t) = self.mobile.touch.as_mut() {
            if !t.moved && !t.long_done && matches!(t.target, Target::Content | Target::Drawer) {
                if now - t.t0 >= LONG_PRESS_MS {
                    t.long_done = true;
                    let (target, at) = (t.target, t.start);
                    self.mobile.pressed_row = None;
                    match target {
                        Target::Content => self.content_long_press(at, p),
                        Target::Drawer => self.drawer_long_press(at, p),
                        _ => {}
                    }
                } else {
                    p.schedule_tick(t.t0 + LONG_PRESS_MS);
                }
            }
        }
        self.fling_step(now, p);
    }

    // --- Contenu ----------------------------------------------------------------------------------------------

    /// Hauteur d'une rangée du contenu de la fenêtre affichée (discussion ou liste).
    fn content_row_h(&self, win: WinId) -> i32 {
        let list = self.client.window(win).is_some_and(views::has_list);
        match self.views.get(&win) {
            Some(v) if list => v.list.row_height(&self.res),
            _ => self.res.fonts.line_height(FontKind::Chat),
        }
        .max(1)
    }

    /// Fait défiler le contenu d'un déplacement du doigt (pixels ; positif = vers le bas, on remonte dans
    /// l'historique), converti en crans de molette d'une rangée.
    fn scroll_content(&mut self, dx: f32, dy: f32, at: Point, p: &mut dyn ShellPlatform) {
        let Some(win) = self.mobile_active() else { return };
        let row_h = self.content_row_h(win) as f32;
        self.mobile.scroll_acc += dy;
        let rows = (self.mobile.scroll_acc / row_h).trunc();
        if rows != 0.0 {
            self.mobile.scroll_acc -= rows * row_h;
            self.content_event(win, &Input::Wheel { pos: at, dx: 0.0, dy: rows / 3.0, mods: Mods::NONE }, p);
        }
        // Listes larges (liste des canaux) : défilement horizontal aussi.
        if dx.abs() >= 1.0 && self.client.window(win).is_some_and(views::has_list) {
            self.content_event(win, &Input::Wheel { pos: at, dx: dx / 48.0, dy: 0.0, mods: Mods::NONE }, p);
        }
    }

    fn content_wheel(&mut self, at: Point, dx: f32, dy: f32, p: &mut dyn ShellPlatform) {
        if let Some(win) = self.mobile_active() {
            self.content_event(win, &Input::Wheel { pos: at, dx, dy, mods: Mods::NONE }, p);
        }
    }

    /// Nombre de touchers (1, ou 2 pour un double toucher rapproché dans le temps et l'espace).
    fn tap_count(&mut self, at: Point, now: i64) -> u8 {
        let double = self.mobile.last_tap.is_some_and(|(t0, q)| now - t0 <= DOUBLE_TAP_MS && (q.x - at.x).abs() <= DOUBLE_TAP_DIST && (q.y - at.y).abs() <= DOUBLE_TAP_DIST);
        self.mobile.last_tap = if double { None } else { Some((now, at)) };
        if double { 2 } else { 1 }
    }

    fn content_tap(&mut self, at: Point, now: i64, p: &mut dyn ShellPlatform) {
        let Some(win) = self.mobile_active() else { return };
        let clicks = self.tap_count(at, now);
        self.content_event(win, &Input::MouseDown { pos: at, button: MouseButton::Left, mods: Mods::NONE, clicks }, p);
        self.content_event(win, &Input::MouseUp { pos: at, button: MouseButton::Left, mods: Mods::NONE }, p);
        self.adopt_popup();
    }

    fn content_long_press(&mut self, at: Point, p: &mut dyn ShellPlatform) {
        let Some(win) = self.mobile_active() else { return };
        self.mobile.last_tap = None;
        let line = self.line_text_at(win, at);
        self.content_event(win, &Input::MouseDown { pos: at, button: MouseButton::Right, mods: Mods::NONE, clicks: 1 }, p);
        self.content_event(win, &Input::MouseUp { pos: at, button: MouseButton::Right, mods: Mods::NONE }, p);
        self.adopt_popup();
        // Pas de sélection au doigt (glisser fait défiler) : l'appui long propose de copier la ligne.
        if let Some(text) = line {
            let copy = MenuItem::new(M_COPY_LINE, &t(self.language(), "mobile.copy_line"));
            match self.mobile.menu.as_mut() {
                Some(m) if m.levels.len() == 1 => {
                    m.levels[0].items.insert(0, MenuItem::separator());
                    m.levels[0].items.insert(0, copy);
                }
                Some(_) => {}
                None => self.mobile.menu = Some(TouchMenu::new(MenuOrigin::Main, vec![copy], at, false)),
            }
            self.mobile.copy_text = Some(text);
        }
        p.request_repaint();
    }

    /// Texte (sans codes de couleur) de la ligne de discussion sous un point.
    fn line_text_at(&self, win: WinId, at: Point) -> Option<String> {
        let r = self.window_rects(win)?;
        if r.list.is_some() || !r.chat.contains(at) {
            return None;
        }
        let w = self.client.window(win)?;
        let v = self.views.get(&win)?;
        let src = WinLines { win: w, client: &self.client, nicks: v.nicks.entries() };
        let lh = self.res.fonts.line_height(FontKind::Chat);
        let row = v.chat.visible_rows(&self.res, r.chat, &src).into_iter().find(|vr| at.y >= vr.y && at.y < vr.y + lh)?;
        let line = w.lines.get(row.index).filter(|l| l.seq == row.seq).or_else(|| w.lines.iter().rev().find(|l| l.seq == row.seq))?;
        Some(alpirc_proto::format::strip(&line.text))
    }

    // --- Tiroirs ----------------------------------------------------------------------------------------------

    fn drawer_tap(&mut self, at: Point, pressed: Option<usize>, p: &mut dyn ShellPlatform) {
        let Some(d) = self.mobile.drawer else { return };
        let Some(i) = self.drawer_row_at(d, at).or(pressed) else { return };
        let now = p.now_ms();
        match d {
            Drawer::Windows => match self.drawer_rows().get(i).copied() {
                Some(DrawerRow::Window { id, .. }) => {
                    let r = self.drawer_rect(d);
                    if at.x >= r.right() - 40 {
                        // Bouton de fermeture.
                        self.client.close_window(id, now);
                        self.after(p);
                    } else {
                        self.activate(id, now);
                        self.mobile.drawer = None;
                        if let Some(v) = self.views.get_mut(&id) {
                            v.chat.scroll_to_bottom();
                        }
                        self.after(p);
                    }
                }
                Some(DrawerRow::NewServer) => {
                    self.mobile.drawer = None;
                    self.menu_action(ids::NEW_STATUS, p);
                }
                None => {}
            },
            Drawer::Nicks => self.nick_popup(i, at, p),
        }
    }

    fn drawer_long_press(&mut self, at: Point, p: &mut dyn ShellPlatform) {
        let Some(d) = self.mobile.drawer else { return };
        let Some(i) = self.drawer_row_at(d, at) else { return };
        match d {
            Drawer::Windows => {
                if let Some(DrawerRow::Window { id, .. }) = self.drawer_rows().get(i).copied() {
                    let items = self.window_menu_items(id);
                    let caption = self.client.window(id).map(|w| short_label(w, &self.client));
                    self.mobile.menu = Some(TouchMenu::new(MenuOrigin::Popup(PopupSrc::Window(id)), items, at, false).with_caption(caption));
                }
            }
            Drawer::Nicks => self.nick_popup(i, at, p),
        }
        p.request_repaint();
    }

    /// Choisit un pseudo du tiroir et ouvre son menu (les menus des scripts : Whois, Query, Slap!…).
    fn nick_popup(&mut self, i: usize, at: Point, p: &mut dyn ShellPlatform) {
        let Some(win) = self.mobile_active() else { return };
        let Some(nick) = self.views.get(&win).and_then(|v| v.nicks.entries().get(i)).map(|e| e.nick.clone()) else { return };
        if let Some(v) = self.views.get_mut(&win) {
            v.nicks.select_nick(&nick);
        }
        self.open_script_popup(PopupKind::Nicklist, win, vec![nick], at);
        self.adopt_popup();
        p.request_repaint();
    }

    // --- Défilement des menus et élan -------------------------------------------------------------------------

    fn scroll_menu(&mut self, dy: f32, p: &mut dyn ShellPlatform) {
        let max = self.menu_max_scroll();
        if let Some(m) = self.mobile.menu.as_mut() {
            if let Some(l) = m.levels.last_mut() {
                l.scroll = (l.scroll - dy as i32).clamp(0, max);
                p.request_repaint();
            }
        }
    }

    fn start_fling(&mut self, t: &Touch, what: Scroller, now: i64, p: &mut dyn ShellPlatform) {
        let (Some(first), Some(last)) = (t.samples.front(), t.samples.back()) else { return };
        let dt = (last.0 - first.0).max(1) as f32;
        if now - last.0 > 80 || dt < 8.0 {
            return;
        }
        let vx = (last.1 - first.1) as f32 / dt;
        let vy = (last.2 - first.2) as f32 / dt;
        if vy.abs() < 0.15 && vx.abs() < 0.15 {
            return;
        }
        self.mobile.fling = Some(Fling { vx, vy, last: now, at: t.last, what });
        p.schedule_tick(now + FLING_STEP_MS);
    }

    fn fling_step(&mut self, now: i64, p: &mut dyn ShellPlatform) {
        let Some(f) = self.mobile.fling.as_mut() else { return };
        let dt = (now - f.last).clamp(1, 100) as f32;
        f.last = now;
        let (dx, dy, at, what) = (f.vx * dt, f.vy * dt, f.at, f.what);
        let k = FLING_DECAY.powf(dt / FLING_STEP_MS as f32);
        f.vx *= k;
        f.vy *= k;
        let done = f.vx.abs() < FLING_MIN && f.vy.abs() < FLING_MIN;
        match what {
            Scroller::Content => self.scroll_content(dx, dy, at, p),
            Scroller::Drawer => {
                if let Some(d) = self.mobile.drawer {
                    let max = self.drawer_max_scroll(d);
                    self.mobile.drawer_scroll = (self.mobile.drawer_scroll - dy as i32).clamp(0, max);
                }
            }
            Scroller::Menu => self.scroll_menu(dy, p),
        }
        p.request_repaint();
        if done {
            self.mobile.fling = None;
        } else {
            p.schedule_tick(now + FLING_STEP_MS);
        }
    }

    // -----------------------------------------------------------------------------------------------------------
    // Dessin.

    pub(crate) fn mobile_paint(&mut self, canvas: &mut Canvas) {
        // Listes des boîtes et des fenêtres à liste : lignes plus hautes au toucher.
        self.res.min_row_h = TOUCH_ROW_H;
        let full = Rect::new(0, 0, self.viewport.w, self.viewport.h);
        canvas.fill(full, self.res.theme.face);
        let active = self.mobile_active();
        if let Some(a) = active {
            self.paint_mobile_content(canvas, a);
        } else {
            let lang = self.language();
            let r = self.mobile_content_rect();
            self.res.fonts.draw_label(canvas, r, &t(lang, "mobile.no_window"), TextStyle::UI, self.res.theme.text, alpirc_ui::Align::Center, false, None);
        }
        self.paint_top_bar(canvas);
        // Sous la barre d'état : la couleur de la barre de titre (icônes du système en clair).
        let top = self.mobile.insets.0;
        if top > 0 {
            canvas.fill(Rect::new(0, 0, self.viewport.w, top), self.res.theme.title_active_from);
        }
        if let Some(d) = self.mobile.drawer {
            canvas.blend_fill(full, 0x000000, 110);
            match d {
                Drawer::Windows => self.paint_windows_drawer(canvas),
                Drawer::Nicks => self.paint_nicks_drawer(canvas),
            }
        }
        if self.mobile.menu.is_some() {
            self.paint_touch_menu(canvas);
        }
        if !self.modals.is_empty() {
            let zooms = self.fit_dialogs();
            canvas.blend_fill(full, 0x000000, 90);
            let s = canvas.scale();
            for ((_, d), z) in self.modals.dialogs.iter().zip(zooms) {
                canvas.set_scale(s * z);
                d.paint(canvas, &self.res);
                canvas.set_scale(s);
            }
        }
    }

    fn paint_mobile_content(&self, canvas: &mut Canvas, win: WinId) {
        let (Some(r), Some(w), Some(v)) = (self.window_rects(win), self.client.window(win), self.views.get(&win)) else { return };
        let content = self.mobile_content_rect();
        canvas.with_clip(content, |c| {
            if let Some(lr) = r.list {
                v.list.paint(c, &self.res, lr);
            } else {
                let entries = v.nicks.entries();
                let src = WinLines { win: w, client: &self.client, nicks: entries };
                v.chat.paint(c, &self.res, r.chat, &src);
            }
            if !self.mobile.native_input {
                if let Some(er) = r.editbox {
                    v.editbox.paint(c, &self.res, er);
                }
            }
        });
        if !self.mobile.native_input {
            v.editbox.paint_popup(canvas, &self.res);
        }
    }

    /// Bouton façon Windows 98 (relief, enfoncé au toucher).
    fn paint_button(&self, canvas: &mut Canvas, r: Rect, pressed: bool) {
        let t = &self.res.theme;
        canvas.fill(r, t.face);
        canvas.bevel(r, if pressed { Bevel::ButtonPressed } else { Bevel::Button }, t);
    }

    fn paint_top_bar(&self, canvas: &mut Canvas) {
        let t = &self.res.theme;
        let bar = self.top_bar();
        canvas.gradient_h(bar, t.title_active_from, t.title_active_to);
        let fg = t.title_active_text;
        // ☰ : tiroir des fenêtres.
        let br = self.top_button_rect(TopButton::Windows);
        let pressed = self.mobile.pressed_button == Some(TopButton::Windows) || self.mobile.drawer == Some(Drawer::Windows);
        self.paint_button(canvas, br, pressed);
        let o = if pressed { 1 } else { 0 };
        for k in 0..3 {
            canvas.fill(Rect::new(br.x + (br.w - 18) / 2 + o, br.y + (br.h - 14) / 2 + k * 6 + o, 18, 2), t.text);
        }
        // Activité ailleurs : un point de couleur sur le bouton.
        if let Some(c) = self.other_activity_color() {
            canvas.fill(Rect::new(br.right() - 9, br.y + 4, 5, 5), c);
        }
        // ⋮ : menu.
        let mr = self.top_button_rect(TopButton::Menu);
        let pressed = self.mobile.pressed_button == Some(TopButton::Menu) || matches!(self.mobile.menu.as_ref().map(|m| &m.origin), Some(MenuOrigin::Main));
        self.paint_button(canvas, mr, pressed);
        let o = if pressed { 1 } else { 0 };
        for k in 0..3 {
            canvas.fill(Rect::new(mr.x + (mr.w - 4) / 2 + o, mr.y + (mr.h - 16) / 2 + k * 6 + o, 4, 4), t.text);
        }
        // Pseudos du canal.
        let mut right = mr.x - 4;
        let active = self.mobile_active();
        let win = active.and_then(|a| self.client.window(a));
        if self.has_nick_button() {
            let nr = self.top_button_rect(TopButton::Nicks);
            let pressed = self.mobile.pressed_button == Some(TopButton::Nicks) || self.mobile.drawer == Some(Drawer::Nicks);
            self.paint_button(canvas, nr, pressed);
            let o = if pressed { 1 } else { 0 };
            // Deux silhouettes (tête et épaules), puis le nombre de personnes.
            const PEOPLE: [&str; 11] = [
                "......kkk......",
                ".kkk.kkkkk.....",
                "kkkkkkkkkkk....",
                "kkkkkkkkkkk....",
                "kkkkk.kkk......",
                ".kkk...........",
                "....kkkkkkkk...",
                "kkkkkkkkkkkkkk.",
                "kkkkkkkkkkkkkkk",
                "kkkkkkkkkkkkkkk",
                "kkkkkkkkkkkkkkk",
            ];
            draw_pattern(canvas, nr.x + (nr.w - 15) / 2 + o, nr.y + 5 + o, &PEOPLE, |ch| (ch == b'k').then_some(t.text));
            let n = active.and_then(|a| self.views.get(&a)).map_or(0, |v| v.nicks.entries().len());
            let label = n.to_string();
            let lr = Rect::new(nr.x + o, nr.bottom() - 18 + o, nr.w, 16);
            self.res.fonts.draw_label(canvas, lr, &label, TextStyle::UI, t.text, alpirc_ui::Align::Center, false, None);
            right = nr.x - 4;
        }
        // Nom de la fenêtre et sujet.
        let x = br.right() + 6;
        let w = (right - x).max(10);
        let (name, sub) = match win {
            Some(w) => {
                let name = short_label(w, &self.client);
                let sub = match w.kind {
                    WinKind::Channel => self.client.channel_state(w.id).and_then(|c| c.topic.clone()).unwrap_or_else(|| {
                        self.client.channel_state(w.id).map(|c| c.mode_string(true)).filter(|m| !m.is_empty()).map(|m| format!("[{m}]")).unwrap_or_default()
                    }),
                    WinKind::Status => w.title.clone(),
                    _ => w.title.strip_prefix(&w.name).map(|s| s.trim().to_string()).unwrap_or_else(|| w.title.clone()),
                };
                (name, sub)
            }
            None => ("AlpIRC".to_string(), String::new()),
        };
        canvas.with_clip(Rect::new(x, bar.y, w, bar.h), |c| {
            let name = self.res.fonts.ellipsize(&name, TextStyle::UI_BOLD, w);
            if sub.is_empty() {
                self.res.fonts.draw_text(c, x, bar.y + (TOP_BAR_H - 16) / 2, &name, TextStyle::UI_BOLD, fg);
            } else {
                self.res.fonts.draw_text(c, x, bar.y + 4, &name, TextStyle::UI_BOLD, fg);
                draw_mirc_line(c, &self.res, Rect::new(x, bar.y + 22, w, 18), &sub, FontKind::Ui, fg);
            }
        });
    }

    /// Couleur de l'activité la plus forte des autres fenêtres (pour le bouton ☰).
    fn other_activity_color(&self) -> Option<u32> {
        let active = self.mobile_active();
        let a = self.client.windows().iter().filter(|w| Some(w.id) != active).map(|w| Self::activity(w.activity)).max()?;
        a.color(&self.res.theme)
    }

    fn paint_drawer_frame(&self, canvas: &mut Canvas, r: Rect, title: &str, sub: &str) {
        let t = &self.res.theme;
        canvas.fill(r, t.face);
        canvas.bevel(r, Bevel::Window, t);
        let head = Rect::new(r.x + 3, r.y + 3, r.w - 6, HEADER_H - 6);
        canvas.gradient_h(head, t.title_active_from, t.title_active_to);
        let mut x = head.x + 6;
        if let Some(img) = self.res.icons.by_name("app/alpirc-16") {
            canvas.blit(img, x, head.y + (head.h - 16) / 2);
            x += 22;
        }
        let w = head.right() - x - 4;
        if sub.is_empty() {
            let s = self.res.fonts.ellipsize(title, TextStyle::UI_BOLD, w);
            self.res.fonts.draw_text(canvas, x, head.y + (head.h - 16) / 2, &s, TextStyle::UI_BOLD, t.title_active_text);
        } else {
            let s = self.res.fonts.ellipsize(title, TextStyle::UI_BOLD, w);
            self.res.fonts.draw_text(canvas, x, head.y + 2, &s, TextStyle::UI_BOLD, t.title_active_text);
            let s = self.res.fonts.ellipsize(sub, TextStyle::UI, w);
            self.res.fonts.draw_text(canvas, x, head.y + 18, &s, TextStyle::UI, t.title_active_text);
        }
    }

    fn paint_windows_drawer(&self, canvas: &mut Canvas) {
        let lang = self.language();
        let t = &self.res.theme;
        let d = Drawer::Windows;
        let r = self.drawer_rect(d);
        self.paint_drawer_frame(canvas, r, &format!("AlpIRC — {}", t_plain(lang, "mobile.windows")), "");
        let rows_r = self.drawer_rows_rect(d).inset_sides(3, 0, 3, 3);
        let rows = self.drawer_rows();
        let active = self.mobile_active();
        canvas.fill(rows_r, t.color(t.nicklist_bg));
        canvas.with_clip(rows_r, |c| {
            for (i, row) in rows.iter().enumerate() {
                let y = rows_r.y + i as i32 * ROW_H - self.mobile.drawer_scroll;
                if y + ROW_H < rows_r.y || y > rows_r.bottom() {
                    continue;
                }
                let rr = Rect::new(rows_r.x, y, rows_r.w, ROW_H);
                let pressed = self.mobile.pressed_row == Some(i);
                match *row {
                    DrawerRow::Window { id, child } => {
                        let Some(w) = self.client.window(id) else { continue };
                        let selected = Some(id) == active;
                        let (bg, mut fg) = if selected || pressed { (t.selection, t.selection_text) } else { (t.color(t.nicklist_bg), t.color(t.nicklist_fg)) };
                        c.fill(rr, bg);
                        if !selected && !pressed {
                            if let Some(ac) = Self::activity(w.activity).color(t) {
                                fg = ac;
                            }
                        }
                        let mut x = rr.x + if child { 26 } else { 8 };
                        let connected = w.conn.and_then(|cn| self.client.conn_info(cn)).is_some_and(|ci| ci.state != ConnState::Disconnected);
                        if let Some(img) = views::icon(&self.res, views::icon_name(w, connected)).and_then(|i| self.res.icons.image(i)) {
                            c.blit(img, x, rr.y + (ROW_H - 16) / 2);
                            x += 24;
                        }
                        let style = if w.kind == WinKind::Status { TextStyle::UI_BOLD } else { TextStyle::UI };
                        let label = self.res.fonts.ellipsize(&short_label(w, &self.client), style, rr.right() - 44 - x);
                        self.res.fonts.draw_text(c, x, rr.y + (ROW_H - 16) / 2, &label, style, fg);
                        // Fermer.
                        let xr = Rect::new(rr.right() - 34, rr.y + (ROW_H - 24) / 2, 26, 24);
                        self.paint_button(c, xr, false);
                        if let Some(img) = self.res.icons.by_name("ui/close_x") {
                            c.blit(img, xr.x + (xr.w - img.w as i32) / 2, xr.y + (xr.h - img.h as i32) / 2);
                        }
                        c.hline(rr.x, rr.bottom() - 1, rr.w, t.light);
                    }
                    DrawerRow::NewServer => {
                        let (bg, fg) = if pressed { (t.selection, t.selection_text) } else { (t.face, t.text) };
                        c.fill(rr, bg);
                        let mut x = rr.x + 8;
                        if let Some(img) = self.res.icons.by_name("toolbar/connect") {
                            c.blit(img, x, rr.y + (ROW_H - 16) / 2);
                            x += 24;
                        }
                        let label = self.res.fonts.ellipsize(&t_plain(lang, "file.new_status"), TextStyle::UI, rr.right() - 8 - x);
                        self.res.fonts.draw_text(c, x, rr.y + (ROW_H - 16) / 2, &label, TextStyle::UI, fg);
                    }
                }
            }
        });
        canvas.bevel(rows_r.inset(-2).intersect(&r.inset(3)), Bevel::Field, t);
    }

    fn paint_nicks_drawer(&self, canvas: &mut Canvas) {
        let lang = self.language();
        let t = &self.res.theme;
        let d = Drawer::Nicks;
        let r = self.drawer_rect(d);
        let active = self.mobile_active();
        let name = active.and_then(|a| self.client.window(a)).map(|w| w.name.clone()).unwrap_or_default();
        let Some(v) = active.and_then(|a| self.views.get(&a)) else { return };
        let entries = v.nicks.entries();
        let selected = v.nicks.selected_nicks();
        self.paint_drawer_frame(canvas, r, &name, &tf(lang, "mobile.users", &[("n", &entries.len().to_string())]));
        let rows_r = self.drawer_rows_rect(d).inset_sides(3, 0, 3, 3);
        canvas.fill(rows_r, t.color(t.nicklist_bg));
        canvas.with_clip(rows_r, |c| {
            for (i, e) in entries.iter().enumerate() {
                let y = rows_r.y + i as i32 * NICK_ROW_H - self.mobile.drawer_scroll;
                if y + NICK_ROW_H < rows_r.y || y > rows_r.bottom() {
                    continue;
                }
                let rr = Rect::new(rows_r.x, y, rows_r.w, NICK_ROW_H);
                let sel = self.mobile.pressed_row == Some(i) || selected.contains(&e.nick);
                let (bg, fg) = if sel {
                    (t.selection, t.selection_text)
                } else if e.away {
                    (t.color(t.nicklist_bg), t.color(t.nicklist_away))
                } else {
                    (t.color(t.nicklist_bg), e.color.unwrap_or(t.color(t.nicklist_fg)))
                };
                c.fill(rr, bg);
                let ty = rr.y + (NICK_ROW_H - self.res.fonts.line_height(FontKind::Chat)) / 2;
                let x = rr.x + 8;
                let pw = self.res.fonts.measure("@", TextStyle::CHAT);
                if !e.prefix.is_empty() {
                    self.res.fonts.draw_text(c, x, ty, &e.prefix, TextStyle::CHAT, fg);
                }
                let label = self.res.fonts.ellipsize(&e.nick, TextStyle::CHAT, rr.right() - 8 - (x + pw + 2));
                self.res.fonts.draw_text(c, x + pw + 2, ty, &label, TextStyle::CHAT, fg);
            }
        });
        canvas.bevel(rows_r.inset(-2).intersect(&r.inset(3)), Bevel::Field, t);
    }

    fn paint_touch_menu(&self, canvas: &mut Canvas) {
        let (Some(m), Some(r)) = (self.mobile.menu.as_ref(), self.menu_rect()) else { return };
        let t = &self.res.theme;
        canvas.fill(r, t.menu);
        canvas.bevel(r, Bevel::Window, t);
        let l = m.level();
        let inner = r.inset(MENU_BORDER);
        canvas.with_clip(inner, |c| {
            for (row, rr) in self.menu_rows() {
                if rr.bottom() < inner.y || rr.y > inner.bottom() {
                    continue;
                }
                let Some(i) = row else {
                    // Titre du sous-menu, avec la flèche de retour ; au premier niveau, ce dont parle le menu.
                    c.fill(rr, t.face);
                    let x = if m.levels.len() > 1 {
                        draw_arrow(c, Rect::new(rr.x + 4, rr.y, 20, rr.h), ArrowDir::Left, t.text);
                        rr.x + 28
                    } else {
                        rr.x + 10
                    };
                    let title = m.header().map(plain).unwrap_or_default();
                    let s = self.res.fonts.ellipsize(&title, TextStyle::UI_BOLD, rr.right() - 8 - x);
                    self.res.fonts.draw_text(c, x, rr.y + (rr.h - 16) / 2, &s, TextStyle::UI_BOLD, t.text);
                    c.etched_hline(rr.x, rr.bottom() - 2, rr.w, t);
                    continue;
                };
                let it = &l.items[i];
                if it.separator {
                    c.etched_hline(rr.x + 2, rr.y + rr.h / 2 - 1, rr.w - 4, t);
                    continue;
                }
                let pressed = self.mobile.pressed_row == Some(i) && it.enabled;
                let fg = if pressed { t.menu_highlight_text } else { t.menu_text };
                if pressed {
                    c.fill(rr, t.menu_highlight);
                }
                let ty = rr.y + (rr.h - 16) / 2;
                if it.checked {
                    let (pat, w, h): (&[&str], i32, i32) = if it.radio { (&MENU_DOT, 5, 5) } else { (&CHECK_MARK, 7, 7) };
                    draw_pattern(c, rr.x + 10 - w / 2 + 4, rr.y + (rr.h - h) / 2, pat, |ch| (ch == b'k').then_some(fg));
                }
                let label = self.res.fonts.ellipsize(&plain(&it.label), TextStyle::UI, rr.w - 28 - 24);
                if it.enabled {
                    self.res.fonts.draw_text(c, rr.x + 24, ty, &label, TextStyle::UI, fg);
                } else {
                    self.res.fonts.draw_text_disabled(c, rr.x + 24, ty, &label, TextStyle::UI, t);
                }
                if it.is_submenu() {
                    draw_arrow(c, Rect::new(rr.right() - 22, rr.y, 16, rr.h), ArrowDir::Right, fg);
                }
            }
        });
        // Indication de défilement.
        if self.menu_max_scroll() > 0 {
            let l = m.level();
            if l.scroll > 0 {
                draw_arrow(canvas, Rect::new(inner.right() - 14, inner.y, 12, 10), ArrowDir::Up, t.shadow);
            }
            if l.scroll < self.menu_max_scroll() {
                draw_arrow(canvas, Rect::new(inner.right() - 14, inner.bottom() - 10, 12, 10), ArrowDir::Down, t.shadow);
            }
        }
    }
}

/// Texte traduit sans `&` de raccourci.
fn t_plain(lang: alpirc_core::i18n::Lang, key: &str) -> String {
    plain(&t(lang, key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separateurs_nettoyes() {
        let items = vec![MenuItem::separator(), MenuItem::new(1, "a"), MenuItem::separator(), MenuItem::separator(), MenuItem::new(2, "b"), MenuItem::separator()];
        let c = clean_items(items);
        assert_eq!(c.len(), 3);
        assert!(!c[0].separator && c[1].separator && !c[2].separator);
    }

    #[test]
    fn barre_de_menus_du_telephone() {
        let model = vec![
            ("&Fichier".to_string(), vec![MenuItem::new(ids::CONNECT, "c"), MenuItem::separator(), MenuItem::new(ids::EXIT, "q")]),
            ("&Affichage".to_string(), vec![MenuItem::new(ids::TOOLBAR, "t"), MenuItem::new(ids::SWITCHBAR, "s"), MenuItem::separator(), MenuItem::new(ids::FIND, "f")]),
            ("Fe&nêtre".to_string(), vec![MenuItem::new(ids::CASCADE, "x"), MenuItem::separator(), MenuItem::new(ids::CLOSE, "w"), MenuItem::new(ids::WIN_BASE + 3, "#lyon")]),
            ("Vide".to_string(), vec![MenuItem::new(ids::TILE_H, "h")]),
        ];
        let m = mobile_menubar(model);
        assert_eq!(m.len(), 3, "le menu vidé disparaît");
        assert_eq!(m[1].submenu.len(), 1, "barres d'outils retirées, séparateur de tête retiré");
        assert_eq!(m[1].submenu[0].id, ids::FIND);
        assert_eq!(m[2].submenu.iter().map(|i| i.id).collect::<Vec<_>>(), vec![ids::CLOSE], "mosaïque et liste des fenêtres retirées");
    }

    #[test]
    fn pas_dans_l_historique() {
        // Trois lignes : ↑ depuis le brouillon donne la plus récente, puis les plus anciennes, puis s'arrête.
        assert_eq!(step_history(3, None, true), Some(Some(2)));
        assert_eq!(step_history(3, Some(2), true), Some(Some(1)));
        assert_eq!(step_history(3, Some(0), true), None);
        // ↓ redescend, puis revient au brouillon ; depuis le brouillon, rien.
        assert_eq!(step_history(3, Some(1), false), Some(Some(2)));
        assert_eq!(step_history(3, Some(2), false), Some(None));
        assert_eq!(step_history(3, None, false), None);
        // Historique vide.
        assert_eq!(step_history(0, None, true), None);
        // Position périmée (historique raccourci) : rien.
        assert_eq!(step_history(1, Some(5), true), None);
    }

    #[test]
    fn menu_de_deconnexion_en_arriere_plan() {
        let lang = Lang::Fr;
        let v = background_items(lang, 15);
        let checked: Vec<u32> = v.iter().filter(|i| i.checked).map(|i| i.id).collect();
        assert_eq!(checked, vec![M_BG_BASE + 15]);
        assert_eq!(v[0].id, M_BG_BASE, "jamais");
        assert_eq!(plain(&v[0].label), "Jamais");
        assert_eq!(v[2].label, "Après 15 minutes");
        // Une durée hors de la liste : « Autre durée (45 min)… » cochée.
        let v = background_items(lang, 45);
        let other = v.iter().find(|i| i.id == M_BG_OTHER).unwrap();
        assert!(other.checked && other.label.contains("45 min"));
        assert_eq!(v.iter().filter(|i| i.checked).count(), 1);
        // Les identifiants restent hors des plages des autres entrées.
        assert!(!(M_SCALE + 100..M_SCALE + 1000).contains(&(M_BG_BASE + BG_MAX_MIN)));
        assert!(M_BG_BASE + BG_MAX_MIN < ids::SCRIPT_BASE && M_BG_BASE > ids::WIN_BASE + 100_000);
    }

    #[test]
    fn dernier_mot() {
        assert_eq!(last_word_start("salut gui"), 6);
        assert_eq!(last_word_start("gui"), 0);
        assert_eq!(last_word_start("é à"), 3);
        assert_eq!(last_word_start("fini "), 5);
    }

    #[test]
    fn reduction_des_boites() {
        let z = dialog_zoom((300, 200), (360, 700));
        assert!((z - 352.0 / 300.0).abs() < 1e-4, "agrandie à la largeur de l'écran");
        assert_eq!(dialog_zoom((100, 80), (360, 700)), MAX_DIALOG_ZOOM);
        let z = dialog_zoom((520, 400), (360, 700));
        assert!((z - 352.0 / 520.0).abs() < 1e-4);
        assert_eq!(dialog_zoom((2000, 2000), (360, 700)), MIN_DIALOG_ZOOM);
        let r = unzoom_rect(Rect::new(0, 20, 360, 700), 0.5);
        assert_eq!(r, Rect::new(0, 40, 720, 1400));
        assert_eq!(unzoom(Point::new(10, 11), 0.5), Point::new(20, 22));
    }
}

/// Essais de la disposition du téléphone avec l'application entière (sans écran ni réseau) : historique de la
/// saisie, réglages du téléphone dans le menu ⋮, déconnexion en arrière-plan et reconnexion.
#[cfg(test)]
mod phone_tests {
    use std::path::{Path, PathBuf};

    use alpirc_core::settings::{Network, Server};
    use alpirc_net::{Net, NetEvent};
    use alpirc_ui::{Cursor, Input, Key, Mods, Platform, Rect};

    use super::*;
    use crate::{LayoutKind, ShellConfig};

    #[derive(Default)]
    struct P {
        now: i64,
    }

    impl Platform for P {
        fn clipboard_get(&mut self) -> Option<String> {
            None
        }
        fn clipboard_set(&mut self, _text: &str) {}
        fn now_ms(&self) -> i64 {
            self.now
        }
        fn set_cursor(&mut self, _c: Cursor) {}
        fn set_ime_area(&mut self, _rect: Rect) {}
        fn request_repaint(&mut self) {}
    }

    impl ShellPlatform for P {
        fn notify(&mut self, _title: &str, _body: &str) {}
        fn beep(&mut self) {}
        fn play_sound(&mut self, _path: &Path) {}
        fn flash(&mut self) {}
        fn open_url(&mut self, _url: &str) {}
        fn set_title(&mut self, _title: &str) {}
        fn exit(&mut self) {}
    }

    fn phone(name: &str, p: &mut P) -> Shell {
        let d: PathBuf = std::env::temp_dir().join(format!("alpirc-mobile-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        p.now = 1_792_260_000_000;
        let cfg = ShellConfig {
            settings_path: d.join("settings.toml"),
            data_dir: d.clone(),
            layout: LayoutKind::Mobile,
            utc_offset: 0,
            locale: Some("fr-FR".into()),
            now_ms: p.now,
        };
        let mut s = Shell::new(cfg, Net::new(Box::new(|_e: NetEvent| {})));
        s.set_viewport(alpirc_ui::Size::new(360, 800));
        s.set_native_input(true);
        s
    }

    fn status(s: &Shell) -> WinId {
        s.client.windows().iter().find(|w| w.kind == WinKind::Status).map(|w| w.id).unwrap()
    }

    #[test]
    fn historique_du_champ_natif() {
        let mut p = P::default();
        let mut s = phone("hist", &mut p);
        let st = status(&s);
        s.activate(st, p.now);
        for l in ["/echo un", "/echo deux", "/echo trois"] {
            s.mobile_submit(l, &mut p);
        }
        // ↑ depuis le brouillon : la plus récente, puis les plus anciennes ; la plus ancienne reste.
        assert_eq!(s.mobile_history_step(true, "brouil").as_deref(), Some("/echo trois"));
        assert_eq!(s.mobile_history_step(true, "/echo trois").as_deref(), Some("/echo deux"));
        assert_eq!(s.mobile_history_step(true, "/echo deux").as_deref(), Some("/echo un"));
        assert_eq!(s.mobile_history_step(true, "/echo un"), None);
        // ↓ redescend jusqu'au brouillon gardé.
        assert_eq!(s.mobile_history_step(false, "/echo un").as_deref(), Some("/echo deux"));
        assert_eq!(s.mobile_history_step(false, "").as_deref(), Some("/echo trois"));
        assert_eq!(s.mobile_history_step(false, "").as_deref(), Some("brouil"));
        assert_eq!(s.mobile_history_step(false, "brouil"), None);
        // La liste (bouton Historique) : la plus récente d'abord ; le choix revient pour le champ natif.
        s.mobile_history_menu(&mut p);
        let first = s.mobile.menu.as_ref().unwrap().level().items[0].label.clone();
        assert_eq!(first, "/echo trois");
        s.menu_tap(Some(1), &mut p);
        assert_eq!(s.take_native_input_text().as_deref(), Some("/echo deux"));
        assert_eq!(s.take_native_input_text(), None, "une seule fois");
        assert!(s.mobile.menu.is_none());
    }

    #[test]
    fn reglages_du_telephone_dans_le_menu() {
        let mut p = P::default();
        let mut s = phone("prefs", &mut p);
        assert_eq!(s.take_phone_prefs_change(), None);
        s.set_phone_prefs(0, true);
        // La section Téléphone est dans le menu ⋮.
        s.open_main_menu(&p);
        let labels: Vec<String> = s.mobile.menu.as_ref().unwrap().level().items.iter().map(|i| plain(&i.label)).collect();
        assert!(labels.iter().any(|l| l == "Téléphone"), "{labels:?}");
        s.mobile.menu = None;
        s.main_menu_action(M_BG_BASE + 15, &mut p);
        assert_eq!(s.take_phone_prefs_change(), Some((15, true)));
        assert_eq!(s.take_phone_prefs_change(), None);
        s.main_menu_action(M_VIBRATE, &mut p);
        assert_eq!(s.take_phone_prefs_change(), Some((15, false)));
        // « Autre durée… » : une valeur absurde garde la boîte ouverte ; 45 la ferme et devient le réglage.
        s.main_menu_action(M_BG_OTHER, &mut p);
        assert!(!s.modals.is_empty());
        s.modals.dialogs.last_mut().unwrap().1.set_text(2, "99999");
        s.dialog_input(&Input::key(Key::Enter, Mods::NONE), (0, 0), &mut p);
        assert!(!s.modals.is_empty(), "plus de 1440 minutes : refusé");
        assert_eq!(s.take_phone_prefs_change(), None);
        s.modals.dialogs.last_mut().unwrap().1.set_text(2, "45");
        s.dialog_input(&Input::key(Key::Enter, Mods::NONE), (0, 0), &mut p);
        assert!(s.modals.is_empty());
        assert_eq!(s.take_phone_prefs_change(), Some((45, false)));
    }

    #[test]
    fn deconnexion_en_arriere_plan_puis_reconnexion() {
        let mut p = P::default();
        let mut s = phone("bg", &mut p);
        s.client.settings_mut().networks.insert(0, Network::new("Local", vec![Server::new("127.0.0.1", 1, false)]));
        let st = status(&s);
        s.client.connect(st, "Local", None, p.now);
        s.tick(&mut p);
        let conn = s.client.window(st).and_then(|w| w.conn).unwrap();
        let a: std::net::SocketAddr = "127.0.0.1:1".parse().unwrap();
        s.on_net(NetEvent::Connected { conn, peer: a, local: a, tls: None }, &mut p);
        s.on_net(NetEvent::Line { conn, bytes: b":srv 001 Guiwald :Welcome".to_vec() }, &mut p);
        assert_eq!(s.client.conn_info(conn).unwrap().state, ConnState::Connected);
        assert_eq!(s.mobile_background_disconnect(&mut p), 1);
        assert_ne!(s.client.conn_info(conn).unwrap().state, ConnState::Connected);
        s.on_net(NetEvent::Closed { conn, error: None }, &mut p);
        assert_eq!(s.client.conn_info(conn).unwrap().state, ConnState::Disconnected);
        // Retour au premier plan : reconnexion au même serveur, une seule fois.
        assert_eq!(s.mobile_background_reconnect(&mut p), 1);
        assert_ne!(s.client.conn_info(conn).unwrap().state, ConnState::Disconnected);
        assert_eq!(s.mobile_background_reconnect(&mut p), 0);
        // Rien d'ouvert : rien à fermer.
        let mut q = P::default();
        let mut idle = phone("bg-idle", &mut q);
        assert_eq!(idle.mobile_background_disconnect(&mut q), 0);
    }
}
