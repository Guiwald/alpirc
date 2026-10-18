//! `/list` lancé par la boîte « Liste des canaux » : motif, nombre de personnes, et recherche dans les noms
//! seulement ou dans les sujets aussi.

mod common;

use common::H;

fn names(h: &H, w: alpirc_core::WinId) -> Vec<String> {
    h.c.chanlist(w).unwrap().entries.iter().map(|e| e.name.clone()).collect()
}

fn feed_list(h: &mut H) {
    h.feed(":irc.test.net 321 Guiwald Channel :Users  Name");
    h.feed(":irc.test.net 322 Guiwald #lyon 42 :[+nt] Bienvenue à Lyon");
    h.feed(":irc.test.net 322 Guiwald #rhone 8 :Les amis de Lyon");
    h.feed(":irc.test.net 322 Guiwald #petit 2 :lyon aussi");
    h.feed(":irc.test.net 322 Guiwald #grenoble 12 :Grenoble");
    h.feed(":irc.test.net 323 Guiwald :End of /LIST");
}

#[test]
fn names_only_or_topics_too() {
    let mut h = H::new();
    h.up();
    let st = h.status;
    h.clear();
    // Noms seulement, au moins 3 personnes.
    let w = h.c.list_channels(st, "lyon", Some(3), None, false, h.now).expect("fenêtre de la liste");
    assert_eq!(h.sends(), vec!["LIST".to_string()]);
    feed_list(&mut h);
    assert_eq!(names(&h, w), vec!["#lyon"]);
    // Dans les sujets aussi, au plus 10 personnes.
    let w2 = h.c.list_channels(st, "lyon", None, Some(10), true, h.now).unwrap();
    assert_eq!(w, w2, "même fenêtre");
    feed_list(&mut h);
    assert_eq!(names(&h, w), vec!["#rhone", "#petit"]);
    // Sans motif : tout.
    h.c.list_channels(st, "  ", None, None, false, h.now);
    feed_list(&mut h);
    assert_eq!(names(&h, w).len(), 4);
}
