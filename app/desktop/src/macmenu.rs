//! La barre de menus native de macOS (muda) : le menu AlpIRC, puis ceux de l'application.

use alpirc_core::i18n::Lang;
use alpirc_shell::menus::ids;
use alpirc_shell::tr::{plain, t};
use alpirc_ui::MenuItem;
use muda::accelerator::Accelerator;
use muda::{CheckMenuItem, IsMenuItem, Menu, MenuEvent, PredefinedMenuItem, Submenu};

type Model = Vec<(String, Vec<MenuItem>)>;

pub struct NativeMenus {
    bar: Menu,
    model: Model,
    /// Commandes choisies, remplies par le gestionnaire d'événements de muda.
    chosen: std::sync::Arc<std::sync::Mutex<Vec<u32>>>,
}

/// Raccourci natif de quelques commandes (les autres passent par le clavier de l'application).
fn accel(id: u32) -> Option<Accelerator> {
    let s = match id {
        ids::NEW_STATUS => "CmdOrCtrl+N",
        ids::FIND => "CmdOrCtrl+F",
        ids::CLOSE => "CmdOrCtrl+W",
        ids::OPTIONS => "CmdOrCtrl+Comma",
        ids::EXIT => "CmdOrCtrl+Q",
        _ => return None,
    };
    s.parse().ok()
}

fn add(item: &MenuItem, push: &dyn Fn(&dyn IsMenuItem)) {
    if item.separator {
        push(&PredefinedMenuItem::separator());
    } else if !item.submenu.is_empty() {
        let sub = Submenu::new(plain(&item.label), item.enabled);
        for c in &item.submenu {
            add(c, &|i| {
                let _ = sub.append(i);
            });
        }
        push(&sub);
    } else if item.checked || item.radio {
        push(&CheckMenuItem::with_id(item.id.to_string(), plain(&item.label), item.enabled, true, accel(item.id)));
    } else {
        push(&muda::MenuItem::with_id(item.id.to_string(), plain(&item.label), item.enabled, accel(item.id)));
    }
}

fn build(model: &Model, lang: Lang) -> Menu {
    let bar = Menu::new();
    let app = Submenu::with_items(
        "AlpIRC",
        true,
        &[
            &muda::MenuItem::with_id(ids::ABOUT.to_string(), plain(&t(lang, "help.about")), true, None),
            &PredefinedMenuItem::separator(),
            &muda::MenuItem::with_id(ids::OPTIONS.to_string(), plain(&t(lang, "file.options")), true, accel(ids::OPTIONS)),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::hide(None),
            &PredefinedMenuItem::hide_others(None),
            &PredefinedMenuItem::show_all(None),
            &PredefinedMenuItem::separator(),
            // Pas l'élément « Quitter » prédéfini : il fermerait sans envoyer QUIT aux serveurs.
            &muda::MenuItem::with_id(ids::EXIT.to_string(), plain(&t(lang, "quit.title")), true, accel(ids::EXIT)),
        ],
    )
    .expect("menu AlpIRC");
    let _ = bar.append(&app);
    for (title, items) in model {
        let sub = Submenu::new(plain(title), true);
        for it in items {
            add(it, &|i| {
                let _ = sub.append(i);
            });
        }
        let _ = bar.append(&sub);
    }
    bar
}

impl NativeMenus {
    pub fn new(model: &Model, lang: Lang, wake: impl Fn() + Send + Sync + 'static) -> NativeMenus {
        let chosen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = chosen.clone();
        // Avec un gestionnaire, muda n'utilise plus son canal : on range l'id puis on réveille la boucle.
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            if let Ok(id) = e.id.0.parse::<u32>() {
                if let Ok(mut v) = sink.lock() {
                    v.push(id);
                }
            }
            wake();
        }));
        let bar = build(model, lang);
        bar.init_for_nsapp();
        NativeMenus { bar, model: model.clone(), chosen }
    }

    /// Reconstruit la barre si son contenu a changé.
    pub fn update(&mut self, model: &Model, lang: Lang) {
        if *model == self.model {
            return;
        }
        let bar = build(model, lang);
        bar.init_for_nsapp();
        self.bar = bar;
        self.model = model.clone();
    }

    /// Commandes choisies depuis le dernier appel.
    pub fn poll(&self) -> Vec<u32> {
        self.chosen.lock().map(|mut v| std::mem::take(&mut *v)).unwrap_or_default()
    }
}
