//! Textes de l'interface (menus, boîtes de dialogue), dans les sept langues d'AlpIRC (ADR 0012).
//!
//! Chaque entrée : clé, puis anglais, français, espagnol, allemand, italien, valencien, irlandais. Une langue
//! vide se replie sur l'anglais.

use alpirc_core::i18n::Lang;

/// (clé, [en, fr, es, de, it, va, ga])
pub(crate) const TABLE: &[(&str, [&str; 7])] = &[
    // Menus.
    ("menu.file", ["&File", "&Fichier", "", "", "", "", ""]),
    ("menu.view", ["&View", "&Affichage", "", "", "", "", ""]),
    ("menu.favorites", ["Fa&vorites", "Fa&voris", "", "", "", "", ""]),
    ("menu.tools", ["&Tools", "&Outils", "", "", "", "", ""]),
    ("menu.commands", ["&Commands", "&Commandes", "", "", "", "", ""]),
    ("menu.window", ["&Window", "Fe&nêtre", "", "", "", "", ""]),
    ("menu.help", ["&Help", "&Aide", "", "", "", "", ""]),
    ("file.new_status", ["&New Server Window", "&Nouvelle fenêtre de serveur", "", "", "", "", ""]),
    ("file.connect", ["&Connect", "&Se connecter", "", "", "", "", ""]),
    ("file.disconnect", ["&Disconnect", "Se &déconnecter", "", "", "", "", ""]),
    ("file.select_server", ["Select &Server...", "Choisir un &serveur...", "", "", "", "", ""]),
    ("file.options", ["&Options...", "&Options...", "", "", "", "", ""]),
    ("file.exit", ["E&xit", "&Quitter", "", "", "", "", ""]),
    ("view.toolbar", ["&Toolbar", "Barre d'&outils", "", "", "", "", ""]),
    ("view.switchbar", ["&Switchbar", "&Switchbar", "", "", "", "", ""]),
    ("view.treebar", ["T&reebar", "T&reebar", "", "", "", "", ""]),
    ("view.nicklist", ["&Nickname List", "Liste des &pseudos", "", "", "", "", ""]),
    ("view.theme", ["T&heme", "T&hème", "", "", "", "", ""]),
    ("view.theme.classic", ["&Classic", "&Classique", "", "", "", "", ""]),
    ("view.theme.dark", ["&Dark", "&Sombre", "", "", "", "", ""]),
    ("view.font_bigger", ["&Larger Font", "Police plus &grande", "", "", "", "", ""]),
    ("view.font_smaller", ["S&maller Font", "Police plus &petite", "", "", "", "", ""]),
    ("view.find", ["&Find Text...", "&Rechercher...", "", "", "", "", ""]),
    ("view.clear", ["C&lear Window", "&Effacer la fenêtre", "", "", "", "", ""]),
    ("fav.folder", ["&Channels Folder...", "&Dossier des canaux...", "", "", "", "", ""]),
    ("fav.add", ["&Add to Favorites", "&Ajouter aux favoris", "", "", "", "", ""]),
    ("tools.abook", ["&Address Book...", "&Carnet d'adresses...", "", "", "", "", ""]),
    ("tools.colors", ["C&olors...", "C&ouleurs...", "", "", "", "", ""]),
    ("tools.scripts", ["Scripts &Editor...", "Éditeur de &scripts...", "", "", "", "", ""]),
    ("tools.chanlist", ["Channels &List", "&Liste des canaux", "", "", "", "", ""]),
    ("tools.urls", ["&URL Catcher", "Adresses &attrapées", "", "", "", "", ""]),
    ("tools.notify", ["&Notify List", "Liste &Notify", "", "", "", "", ""]),
    ("tools.dcc_send", ["DCC &Send...", "DCC : &envoyer un fichier...", "", "", "", "", ""]),
    ("tools.dcc_chat", ["DCC &Chat...", "DCC : &discussion...", "", "", "", "", ""]),
    ("tools.transfers", ["DCC &Transfers", "DCC : &transferts", "", "", "", "", ""]),
    ("tools.logs", ["Log &Files...", "&Journaux...", "", "", "", "", ""]),
    ("win.cascade", ["&Cascade", "&Cascade", "", "", "", "", ""]),
    ("win.tile_h", ["Tile &Horizontal", "Mosaïque &horizontale", "", "", "", "", ""]),
    ("win.tile_v", ["Tile &Vertical", "Mosaïque &verticale", "", "", "", "", ""]),
    ("win.arrange", ["&Arrange Icons", "&Réorganiser les icônes", "", "", "", "", ""]),
    ("win.next", ["&Next Window", "Fenêtre &suivante", "", "", "", "", ""]),
    ("win.close", ["Cl&ose Window", "&Fermer la fenêtre", "", "", "", "", ""]),
    ("help.readme", ["AlpIRC &Help", "&Aide d'AlpIRC", "", "", "", "", ""]),
    ("help.about", ["&About AlpIRC...", "À &propos d'AlpIRC...", "", "", "", "", ""]),
    ("menu.empty", ["(none)", "(aucun)", "", "", "", "", ""]),
    // Menu de la switchbar et de la fenêtre.
    ("swb.close", ["&Close", "&Fermer", "", "", "", "", ""]),
    ("swb.minimize", ["Mi&nimize", "&Réduire", "", "", "", "", ""]),
    ("swb.maximize", ["Ma&ximize", "&Agrandir", "", "", "", "", ""]),
    ("swb.restore", ["&Restore", "Res&taurer", "", "", "", "", ""]),
    ("swb.log", ["&Logging", "&Journal", "", "", "", "", ""]),
    ("swb.clear", ["C&lear", "&Effacer", "", "", "", "", ""]),
    ("swb.copy", ["&Copy", "&Copier", "", "", "", "", ""]),
    ("swb.select_all", ["Select &All", "&Tout sélectionner", "", "", "", "", ""]),
    // Barre d'outils.
    ("tb.connect", ["Connect", "Se connecter", "", "", "", "", ""]),
    ("tb.disconnect", ["Disconnect", "Se déconnecter", "", "", "", "", ""]),
    ("tb.options", ["Options", "Options", "", "", "", "", ""]),
    ("tb.favorites", ["Channels Folder", "Dossier des canaux", "", "", "", "", ""]),
    ("tb.chanlist", ["List Channels", "Liste des canaux", "", "", "", "", ""]),
    ("tb.scripts", ["Scripts Editor", "Éditeur de scripts", "", "", "", "", ""]),
    ("tb.abook", ["Address Book", "Carnet d'adresses", "", "", "", "", ""]),
    ("tb.dcc_send", ["DCC Send", "Envoyer un fichier", "", "", "", "", ""]),
    ("tb.dcc_chat", ["DCC Chat", "Discussion DCC", "", "", "", "", ""]),
    ("tb.transfers", ["DCC Transfers", "Transferts DCC", "", "", "", "", ""]),
    ("tb.notify", ["Notify List", "Liste Notify", "", "", "", "", ""]),
    ("tb.urls", ["URL Catcher", "Adresses attrapées", "", "", "", "", ""]),
    ("tb.cascade", ["Cascade", "Cascade", "", "", "", "", ""]),
    ("tb.tile_h", ["Tile Horizontal", "Mosaïque horizontale", "", "", "", "", ""]),
    ("tb.tile_v", ["Tile Vertical", "Mosaïque verticale", "", "", "", "", ""]),
    ("tb.colors", ["Colors", "Couleurs", "", "", "", "", ""]),
    ("tb.help", ["Help", "Aide", "", "", "", "", ""]),
    // Boîtes.
    ("dlg.ok", ["OK", "OK", "", "", "", "", ""]),
    ("dlg.cancel", ["Cancel", "Annuler", "", "", "", "", ""]),
    ("dlg.close", ["Close", "Fermer", "", "", "", "", ""]),
    ("dlg.yes", ["&Yes", "&Oui", "", "", "", "", ""]),
    ("dlg.no", ["&No", "&Non", "", "", "", "", ""]),
    ("paste.title", ["Paste", "Coller", "", "", "", "", ""]),
    ("paste.text", ["You are about to paste {n} lines into {win}. Send them?", "Vous allez coller {n} lignes dans {win}. Les envoyer ?", "", "", "", "", ""]),
    ("paste.send", ["&Send", "&Envoyer", "", "", "", "", ""]),
    ("paste.edit", ["Send as &one line", "En &une seule ligne", "", "", "", "", ""]),
    ("join.title", ["Join Channel", "Rejoindre un canal", "", "", "", "", ""]),
    ("join.prompt", ["Enter the name of the channel to join:", "Nom du canal à rejoindre :", "", "", "", "", ""]),
    ("about.title", ["About AlpIRC", "À propos d'AlpIRC", "", "", "", "", ""]),
    ("about.text", [
        "AlpIRC {v}\nThe IRC client of the 2000s, redone from scratch in Rust.\n\nMIT License — https://github.com/Guiwald/alpirc\nNot affiliated with mIRC or mIRC Co. Ltd.\n\nFonts: Fixedsys Excelsior (public domain), Pixel Operator (CC0).",
        "AlpIRC {v}\nLe client IRC des années 2000, refait de zéro en Rust.\n\nLicence MIT — https://github.com/Guiwald/alpirc\nSans lien avec mIRC ni mIRC Co. Ltd.\n\nPolices : Fixedsys Excelsior (domaine public), Pixel Operator (CC0).",
        "", "", "", "", "",
    ]),
    ("soon.title", ["Coming soon", "Bientôt", "", "", "", "", ""]),
    ("soon.text", ["This window arrives in a next version of AlpIRC.", "Cette fenêtre arrive dans une prochaine version d'AlpIRC.", "", "", "", "", ""]),
    ("find.title", ["Find Text", "Rechercher", "", "", "", "", ""]),
    ("find.prompt", ["Find:", "Rechercher :", "", "", "", "", ""]),
    ("find.next", ["&Next", "&Suivant", "", "", "", "", ""]),
    ("find.prev", ["&Previous", "&Précédent", "", "", "", "", ""]),
    ("quit.title", ["Exit AlpIRC", "Quitter AlpIRC", "", "", "", "", ""]),
    ("quit.text", ["You are connected to {n} server(s). Quit anyway?", "Vous êtes connecté à {n} serveur(s). Quitter quand même ?", "", "", "", "", ""]),
    ("chanlist.name", ["Channel", "Canal", "", "", "", "", ""]),
    ("chanlist.users", ["Users", "Pers.", "", "", "", "", ""]),
    ("chanlist.topic", ["Topic", "Sujet", "", "", "", "", ""]),
    ("urls.url", ["Address", "Adresse", "", "", "", "", ""]),
    ("urls.nick", ["Nick", "Pseudo", "", "", "", "", ""]),
    ("urls.where", ["Window", "Fenêtre", "", "", "", "", ""]),
    ("urls.time", ["Time", "Heure", "", "", "", "", ""]),
    ("notify.nick", ["Nick", "Pseudo", "", "", "", "", ""]),
    ("notify.state", ["State", "État", "", "", "", "", ""]),
    ("notify.note", ["Note", "Note", "", "", "", "", ""]),
    ("notify.online", ["online", "en ligne", "", "", "", "", ""]),
    ("notify.offline", ["offline", "absent", "", "", "", "", ""]),
    ("win.chanlist", ["Channels List", "Liste des canaux", "", "", "", "", ""]),
    ("win.urls", ["URL Catcher", "Adresses attrapées", "", "", "", "", ""]),
    ("win.notify", ["Notify List", "Liste Notify", "", "", "", "", ""]),
];

fn column(lang: Lang) -> usize {
    match lang {
        Lang::En => 0,
        Lang::Fr => 1,
        Lang::Es => 2,
        Lang::De => 3,
        Lang::It => 4,
        Lang::Va => 5,
        Lang::Ga => 6,
    }
}

/// Une table de textes : (clé, [en, fr, es, de, it, va, ga]).
pub type Table = &'static [(&'static str, [&'static str; 7])];

/// Toutes les tables : celle-ci, puis celles des modules (boîtes de dialogue, DCC, téléphone).
pub(crate) const TABLES: &[Table] = &[TABLE, crate::dialogs::TR, crate::dcc::TR, crate::mobile::TR];

fn find(key: &str) -> Option<&'static [&'static str; 7]> {
    TABLES.iter().flat_map(|t| t.iter()).find(|(k, _)| *k == key).map(|(_, v)| v)
}

/// Texte d'une clé dans une langue (anglais si absent ; la clé elle-même si inconnue).
pub fn t(lang: Lang, key: &str) -> String {
    match find(key) {
        Some(v) => {
            let s = v[column(lang)];
            if s.is_empty() { v[0].to_string() } else { s.to_string() }
        }
        None => key.to_string(),
    }
}

/// Texte avec remplacements `{nom}`.
pub fn tf(lang: Lang, key: &str, args: &[(&str, &str)]) -> String {
    let mut s = t(lang, key);
    for (k, v) in args {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    s
}

/// Retire les `&` de raccourci (titres de boîtes, infobulles).
pub fn plain(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            if chars.peek() == Some(&'&') {
                out.push('&');
                chars.next();
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_unique_and_complete() {
        let mut keys: Vec<&str> = TABLES.iter().flat_map(|t| t.iter()).map(|(k, _)| *k).collect();
        keys.sort();
        let n = keys.len();
        keys.dedup();
        assert_eq!(n, keys.len(), "clés en double");
        for (k, v) in TABLES.iter().flat_map(|t| t.iter()) {
            assert!(!v[0].is_empty() && !v[1].is_empty(), "{k} : anglais et français obligatoires");
        }
    }

    #[test]
    fn fallback() {
        assert_eq!(t(Lang::De, "file.exit"), "E&xit");
        assert_eq!(t(Lang::Fr, "file.exit"), "&Quitter");
        assert_eq!(plain("&Quitter"), "Quitter");
        assert_eq!(tf(Lang::Fr, "paste.text", &[("n", "3"), ("win", "#lyon")]), "Vous allez coller 3 lignes dans #lyon. Les envoyer ?");
    }
}
