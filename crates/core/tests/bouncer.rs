//! Bouncer soju (et relais d'AlpIRC) : connexion parente non liée, notifications partielles des réseaux, liste
//! des réseaux, nos messages privés écrits sur un autre appareil.

mod common;

use alpirc_core::WinKind;
use alpirc_core::settings::{Favorite, NotifyEntry};
use common::*;

/// Connexion au bouncer, capacités données acceptées, enregistrement complet.
fn bouncer_with(h: &mut H, caps: &str) {
    h.connect();
    // L'ouverture de la connexion parente elle-même n'est pas une connexion liée.
    let _ = h.connects();
    h.feed(&format!(":bnc CAP * LS :{caps}"));
    h.feed(&format!(":bnc CAP * ACK :{caps}"));
    h.feed(":bnc 001 Guiwald :Welcome to soju");
    h.feed(":bnc 005 Guiwald CHANTYPES=# :are supported");
    h.feed(":bnc 376 Guiwald :End");
}

fn status_count(h: &H, needle: &str) -> usize {
    h.status_lines().iter().filter(|l| l.contains(needle)).count()
}

#[test]
fn listnetworks_only_without_notify() {
    let mut h = H::new();
    bouncer_with(&mut h, "batch soju.im/bouncer-networks");
    assert!(h.sends().contains(&"BOUNCER LISTNETWORKS".to_string()), "sans -notify : on demande la liste");
    let mut h = H::new();
    bouncer_with(&mut h, "batch soju.im/bouncer-networks soju.im/bouncer-networks-notify");
    assert!(!h.sends().contains(&"BOUNCER LISTNETWORKS".to_string()), "avec -notify : la liste vient d'elle-même");
    // La liste initiale n'est affichée qu'une fois.
    h.feed(":bnc BATCH +n soju.im/bouncer-networks");
    h.feed("@batch=n :bnc BOUNCER NETWORK 1 name=Libera;state=connected");
    h.feed(":bnc BATCH -n");
    assert_eq!(status_count(&h, "Bouncer networks"), 1);
}

#[test]
fn partial_notifications_keep_the_name() {
    let mut h = H::new();
    bouncer_with(&mut h, "batch soju.im/bouncer-networks soju.im/bouncer-networks-notify");
    h.feed(":bnc BATCH +n soju.im/bouncer-networks");
    h.feed("@batch=n :bnc BOUNCER NETWORK 1 name=Libera;state=connected;host=irc.libera.chat");
    h.feed(":bnc BATCH -n");
    let child = h.connects()[0].0;
    assert_eq!(h.c.conn_info(child).unwrap().network, "Libera");
    h.clear();
    // Changements d'état sans le nom : le nom reste, rien n'est réaffiché ni réenregistré, pas de 2e connexion.
    h.feed(":bnc BOUNCER NETWORK 1 state=disconnected;error=Connection\\sreset");
    h.feed(":bnc BOUNCER NETWORK 1 state=connected;error");
    assert_eq!(h.c.conn_info(child).unwrap().network, "Libera");
    assert_eq!(h.c.settings().network("TestNet").unwrap().bouncer_networks[0].name, "Libera");
    assert_eq!(status_count(&h, "Bouncer networks"), 1);
    let fx = h.effects();
    assert!(!fx.contains(&alpirc_core::Effect::SaveSettings), "{fx:?}");
    assert!(h.connects().is_empty());
    // Un vrai renommage : la connexion liée prend le nouveau nom, la liste est réaffichée.
    h.feed(":bnc BOUNCER NETWORK 1 name=Libera.Chat");
    assert_eq!(h.c.conn_info(child).unwrap().network, "Libera.Chat");
    assert_eq!(h.last(h.status), "* Bouncer networks: Libera.Chat (1)");
    assert_eq!(h.c.settings().network("TestNet").unwrap().bouncer_networks[0].name, "Libera.Chat");
}

#[test]
fn name_comes_back_from_settings_after_restart() {
    let mut s = settings();
    s.network_mut("TestNet")
        .unwrap()
        .bouncer_networks
        .push(alpirc_core::settings::BouncerNetwork { id: "4".into(), name: "OFTC".into() });
    let mut h = H::with(s);
    bouncer_with(&mut h, "batch soju.im/bouncer-networks soju.im/bouncer-networks-notify");
    // Première nouvelle du réseau sans son nom : celui des réglages sert.
    h.feed(":bnc BOUNCER NETWORK 4 state=connecting");
    let child = h.connects()[0].0;
    assert_eq!(h.c.conn_info(child).unwrap().network, "OFTC");
}

#[test]
fn parent_skips_perform_favorites_and_notify() {
    let mut s = settings();
    s.network_mut("TestNet").unwrap().perform = vec!["/join #perf".into()];
    s.favorites.push(Favorite {
        channel: "#fav".into(),
        network: "TestNet".into(),
        join_on_connect: true,
        ..Favorite::default()
    });
    s.address_book.notify.push(NotifyEntry { nick: "Bob".into(), note: String::new(), network: None });
    let mut h = H::with(s);
    bouncer_with(&mut h, "batch soju.im/bouncer-networks soju.im/bouncer-networks-notify");
    h.advance(40_000);
    let parent = h.sends_for(h.conn);
    assert!(
        !parent.iter().any(|l| l.starts_with("JOIN")
            || l.starts_with("ISON")
            || l.starts_with("MONITOR")
            || l.starts_with("PRIVMSG")),
        "la connexion parente ne fait rien de tout cela : {parent:?}"
    );
    // /notify pendant la connexion : toujours rien sur la parente.
    h.input(h.status, "/notify Carl");
    h.advance(31_000);
    assert!(!h.sends_for(h.conn).iter().any(|l| l.starts_with("ISON")));

    // La connexion liée, elle, fait tout.
    h.feed(":bnc BOUNCER NETWORK 1 name=Libera");
    let child = h.connects()[0].0;
    h.c.on_connected(child, h.now);
    h.feed_conn(child, ":bnc CAP * LS :soju.im/bouncer-networks");
    h.feed_conn(child, ":bnc CAP * ACK :soju.im/bouncer-networks");
    h.feed_conn(child, ":bnc 001 Guiwald :Welcome");
    h.feed_conn(child, ":bnc 005 Guiwald CHANTYPES=# NETWORK=Libera.Chat :are supported");
    h.feed_conn(child, ":bnc 376 Guiwald :End");
    h.advance(10);
    let sent = h.sends_for(child);
    assert!(sent.contains(&"JOIN #perf".to_string()), "{sent:?}");
    assert!(sent.iter().any(|l| l.starts_with("JOIN") && l.contains("#fav")), "{sent:?}");
    assert!(sent.iter().any(|l| l.starts_with("ISON") && l.contains("Bob")), "{sent:?}");
}

#[test]
fn bouncer_option_off_keeps_the_connection_complete() {
    let mut s = settings();
    s.network_mut("TestNet").unwrap().bouncer = false;
    s.network_mut("TestNet").unwrap().perform = vec!["/join #perf".into()];
    let mut h = H::with(s);
    bouncer_with(&mut h, "batch soju.im/bouncer-networks");
    let sent = h.sends();
    assert!(sent.contains(&"JOIN #perf".to_string()), "pas de connexions liées : celle-ci reste complète ; {sent:?}");
    assert!(!sent.contains(&"BOUNCER LISTNETWORKS".to_string()));
}

#[test]
fn own_private_message_from_another_device_opens_the_query() {
    let mut h = H::new();
    h.up_with_caps("echo-message server-time");
    let chan = h.join("#lyon", "Guiwald Bob");
    // Écrit sur le téléphone, renvoyé par le bouncer : le privé s'ouvre (sans devenir actif).
    h.feed("@time=2026-10-18T10:00:00.000Z :Guiwald!alpirc@host.test PRIVMSG Bob :écrit sur le téléphone");
    assert_eq!(h.kind_of("Bob"), WinKind::Query);
    assert_eq!(h.last(h.win("Bob")), "<Guiwald> écrit sur le téléphone");
    assert_eq!(h.c.active(), Some(chan), "la fenêtre active ne change pas");
    assert!(!h.lines(chan).iter().any(|l| l.contains("écrit sur le téléphone")));
    // Une action aussi.
    h.feed(":Guiwald!alpirc@host.test PRIVMSG Alice :\x01ACTION salue\x01");
    assert_eq!(h.last(h.win("Alice")), "* Guiwald salue");
    // Un CTCP (autre qu'ACTION) n'ouvre rien.
    h.feed(":Guiwald!alpirc@host.test PRIVMSG Zed :\x01VERSION\x01");
    assert!(!h.has_win("Zed"));
    // L'écho d'un /msg tapé ici reste à la manière de mIRC, dans la fenêtre active.
    h.input(chan, "/msg Carl salut");
    assert!(h.sends().contains(&"PRIVMSG Carl :salut".to_string()));
    h.feed(":Guiwald!alpirc@host.test PRIVMSG Carl :salut");
    assert!(!h.has_win("Carl"));
    assert_eq!(h.last(chan), "-> *Carl* salut");
    // Le même texte vu une seconde fois (envoyé ailleurs) : cette fois le privé s'ouvre.
    h.feed(":Guiwald!alpirc@host.test PRIVMSG Carl :salut");
    assert!(h.has_win("Carl"));
}

#[test]
fn own_private_message_without_echo_message() {
    // Sans echo-message, tout message à nous renvoyé vient d'ailleurs (le bouncer le relaie).
    let mut h = H::new();
    h.up();
    h.feed(":Guiwald!alpirc@host.test PRIVMSG Bob :depuis le Mac");
    assert_eq!(h.last(h.win("Bob")), "<Guiwald> depuis le Mac");
    // Réglage « pas de privé automatique » : comme avant, dans la fenêtre active.
    let mut s = settings();
    s.irc.query_on_private = false;
    let mut h = H::with(s);
    h.up();
    h.feed(":Guiwald!alpirc@host.test PRIVMSG Bob :depuis le Mac");
    assert!(!h.has_win("Bob"));
    assert_eq!(h.last(h.status), "-> *Bob* depuis le Mac");
}

#[test]
fn nickserv_identify_echo_does_not_open_a_query() {
    let mut s = settings();
    s.network_mut("TestNet").unwrap().nickserv_password = Some("secret".into());
    let mut h = H::with(s);
    h.up_with_caps("echo-message");
    h.feed(":Guiwald!alpirc@host.test PRIVMSG NickServ :IDENTIFY secret");
    assert!(!h.has_win("NickServ"), "l'IDENTIFY envoyé par la session est le nôtre");
}
