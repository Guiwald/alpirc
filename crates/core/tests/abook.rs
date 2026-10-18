//! Carnet d'adresses : Ignore (par sortes, codes, durée), Notify (MONITOR et ISON), Control (auto-op,
//! auto-voice, protection).

mod common;

use alpirc_core::Effect;
use alpirc_core::settings::{ControlEntry, ControlKind, IgnoreEntry, NotifyEntry};
use alpirc_script::ScriptKind;
use common::*;

fn chan() -> (H, alpirc_core::WinId) {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "@Guiwald Bob Pat");
    h.feed(":Bob!bob@bob.host PRIVMSG #lyon :je suis là");
    h.clear();
    (h, w)
}

#[test]
fn ignore_all_types() {
    let (mut h, w) = chan();
    h.input(w, "/ignore Bob");
    assert_eq!(h.last(w), "* Ignoring Bob!*@*");
    assert!(h.effects().contains(&Effect::SaveSettings));
    let e = &h.c.settings().address_book.ignore[0];
    assert!(e.private && e.channel && e.notice && e.ctcp && e.invite && e.dcc && !e.codes);
    h.feed(":Bob!bob@bob.host PRIVMSG #lyon :ignoré");
    h.feed(":Bob!bob@bob.host PRIVMSG Guiwald :privé");
    h.feed(":Bob!bob@bob.host NOTICE Guiwald :avis");
    h.feed(":Bob!bob@bob.host PRIVMSG Guiwald :\x01VERSION\x01");
    h.feed(":Bob!bob@bob.host INVITE Guiwald #x");
    assert_eq!(h.last(w), "* Ignoring Bob!*@*");
    assert!(!h.has_win("Bob"));
    assert!(h.sends().is_empty(), "pas de réponse CTCP");
    // Les autres passent.
    h.feed(":Pat!p@h PRIVMSG #lyon :moi oui");
    assert_eq!(h.last(w), "<Pat> moi oui");
    h.input(w, "/ignore -r Bob");
    assert_eq!(h.last(w), "* No longer ignoring Bob!*@*");
    h.feed(":Bob!bob@bob.host PRIVMSG #lyon :de retour");
    assert_eq!(h.last(w), "<Bob> de retour");
}

#[test]
fn ignore_by_type_only() {
    let (mut h, w) = chan();
    h.input(w, "/ignore -c Bob");
    h.feed(":Bob!bob@bob.host PRIVMSG #lyon :canal");
    assert_eq!(h.last(w), "* Ignoring Bob!*@*");
    h.feed(":Bob!bob@bob.host PRIVMSG Guiwald :privé");
    assert_eq!(h.last(h.win("Bob")), "<Bob> privé");
    h.input(w, "/ignore -pt *!*@bob.host");
    h.feed(":Bob!bob@bob.host PRIVMSG Guiwald :encore");
    assert_eq!(h.last(h.win("Bob")), "<Bob> privé");
}

#[test]
fn ignore_codes_strips_colors() {
    let (mut h, w) = chan();
    h.input(w, "/ignore -k Bob");
    h.feed(":Bob!bob@bob.host PRIVMSG #lyon :\x034,1rouge\x03 \x02gras");
    assert_eq!(h.last(w), "<Bob> rouge gras");
}

#[test]
fn ignore_with_mask_type_and_expiry() {
    let (mut h, w) = chan();
    h.input(w, "/ignore -u60 Bob 2");
    assert_eq!(h.last(w), "* Ignoring *!*@bob.host");
    h.feed(":Robert!x@bob.host PRIVMSG #lyon :même hôte");
    assert_eq!(h.last(w), "* Ignoring *!*@bob.host");
    h.now += 60_000;
    h.feed(":Robert!x@bob.host PRIVMSG #lyon :fini");
    assert_eq!(h.last(w), "<Robert> fini");
}

#[test]
fn ignore_network_filter_and_list() {
    let mut s = settings();
    s.address_book.ignore.push(IgnoreEntry { mask: "Bob".into(), channel: true, network: Some("Undernet".into()), ..IgnoreEntry::default() });
    let mut h = H::with(s);
    h.up();
    let w = h.join("#lyon", "Guiwald Bob");
    h.feed(":Bob!b@h PRIVMSG #lyon :pas sur Undernet");
    assert_eq!(h.last(w), "<Bob> pas sur Undernet");
    h.input(w, "/ignore");
    assert_eq!(h.last(w), "* Ignore list: Bob");
}

#[test]
fn notify_with_monitor() {
    let mut s = settings();
    s.address_book.notify.push(NotifyEntry { nick: "Bob".into(), note: "copain".into(), network: None });
    s.address_book.notify.push(NotifyEntry { nick: "Zed".into(), note: String::new(), network: Some("Undernet".into()) });
    let mut h = H::with(s);
    h.c.engine_mut()
        .load_text("t.mrc", ScriptKind::Remote, "on *:NOTIFY:{ echo -s notify $nick }\non *:UNOTIFY:{ echo -s unotify $nick }")
        .unwrap();
    h.connect();
    h.register_with("PREFIX=(ov)@+ CHANTYPES=# MONITOR=100");
    assert!(h.sends().contains(&"MONITOR + Bob".to_string()));
    h.feed(":irc.test.net 730 Guiwald :Bob!b@h");
    assert!(h.has_line(h.status, "* Bob is on IRC"));
    assert!(h.has_line(h.status, "notify Bob"));
    let n = h.c.notify_list();
    assert_eq!(n[0].online, vec![(h.conn, Some(true))]);
    assert_eq!(n[0].note, "copain");
    assert_eq!(n[0].last_seen_ms, Some(h.now));
    assert_eq!(n[1].online, vec![(h.conn, None)]);
    h.feed(":irc.test.net 730 Guiwald :Bob!b@h");
    assert_eq!(h.status_lines().iter().filter(|l| l.as_str() == "* Bob is on IRC").count(), 1, "pas de doublon");
    h.feed(":irc.test.net 731 Guiwald :Bob");
    assert!(h.has_line(h.status, "* Bob has left IRC"));
    assert!(h.has_line(h.status, "unotify Bob"));
    // Ajout pendant la connexion.
    h.input(h.status, "/notify Carl ami");
    assert_eq!(h.sends(), vec!["MONITOR + Carl"]);
    assert_eq!(h.last(h.status), "* Added Carl to notify list");
    h.input(h.status, "/notify -r Carl");
    assert_eq!(h.sends(), vec!["MONITOR - Carl"]);
    h.input(h.status, "/notify -s");
    assert_eq!(h.last(h.status), "* Notify list: Bob, Zed");
    // Sans argument : la fenêtre Notify.
    h.input(h.status, "/notify");
    let notify = h.c.windows().iter().find(|w| w.kind == alpirc_core::WinKind::Notify);
    let w = notify.map(|w| w.id).expect("fenêtre Notify");
    assert_eq!(h.c.active(), Some(w));
}

#[test]
fn notify_with_ison_polling() {
    let mut s = settings();
    s.address_book.notify.push(NotifyEntry { nick: "Bob".into(), ..NotifyEntry::default() });
    s.address_book.notify.push(NotifyEntry { nick: "Carl".into(), ..NotifyEntry::default() });
    let mut h = H::with(s);
    h.connect();
    h.register();
    h.clear();
    h.advance(0);
    assert_eq!(h.sends(), vec!["ISON :Bob Carl"]);
    h.feed(":irc.test.net 303 Guiwald :Bob");
    assert!(h.has_line(h.status, "* Bob is on IRC"));
    assert!(!h.status_lines().iter().any(|l| l == "Bob"), "réponse ISON non affichée");
    h.advance(29_000);
    assert!(h.sends().is_empty());
    h.advance(1_000);
    assert_eq!(h.sends(), vec!["ISON :Bob Carl"]);
    h.feed(":irc.test.net 303 Guiwald :Carl");
    assert!(h.has_line(h.status, "* Bob has left IRC"));
    assert!(h.has_line(h.status, "* Carl is on IRC"));
    // Un /ison tapé s'affiche.
    h.input(h.status, "/quote ISON Bob");
    h.feed(":irc.test.net 303 Guiwald :Bob");
    assert_eq!(h.last(h.status), "Bob");
}

fn control(kind: ControlKind, mask: &str) -> ControlEntry {
    ControlEntry { kind, mask: mask.into(), channels: Vec::new(), network: None }
}

#[test]
fn auto_op_batched_after_delay() {
    let mut s = settings();
    s.address_book.control.push(control(ControlKind::Autoop, "*!*@friend.host"));
    s.address_book.control.push(control(ControlKind::Autovoice, "Vic"));
    let mut h = H::with(s);
    h.up();
    h.join("#lyon", "@Guiwald");
    h.feed(":Bob!b@friend.host JOIN #lyon");
    h.feed(":Carl!c@friend.host JOIN #lyon");
    h.feed(":Eve!e@evil.host JOIN #lyon");
    h.feed(":Vic!v@x JOIN #lyon");
    assert!(h.sends().is_empty());
    h.advance(1_999);
    assert!(h.sends().is_empty());
    h.advance(1);
    assert_eq!(h.sends(), vec!["MODE #lyon +oov Bob Carl Vic"]);
}

#[test]
fn auto_op_skipped_when_not_op_or_already_op() {
    let mut s = settings();
    s.address_book.control.push(control(ControlKind::Autoop, "Bob"));
    let mut h = H::with(s);
    h.up();
    h.join("#lyon", "Guiwald");
    h.feed(":Bob!b@h JOIN #lyon");
    h.advance(3_000);
    assert!(h.sends().is_empty());
    h.join("#ops", "@Guiwald");
    h.feed(":Bob!b@h JOIN #ops");
    h.feed(":Other!o@h MODE #ops +o Bob");
    h.advance(3_000);
    assert!(h.sends().is_empty(), "déjà op");
}

#[test]
fn auto_op_channel_filter() {
    let mut s = settings();
    s.address_book.control.push(ControlEntry { kind: ControlKind::Autoop, mask: "Bob".into(), channels: vec!["#lyon".into()], network: None });
    let mut h = H::with(s);
    h.up();
    h.join("#lyon", "@Guiwald");
    h.join("#autre", "@Guiwald");
    h.feed(":Bob!b@h JOIN #autre");
    h.feed(":Bob!b@h JOIN #lyon");
    h.advance(2_000);
    assert_eq!(h.sends(), vec!["MODE #lyon +o Bob"]);
}

#[test]
fn protect_reops() {
    let mut s = settings();
    s.address_book.control.push(control(ControlKind::Protect, "Bob!*@bob.host"));
    let mut h = H::with(s);
    h.up();
    h.join("#lyon", "@Guiwald @Bob!b@bob.host @Eve");
    h.feed(":Bob!bob@bob.host PRIVMSG #lyon :x");
    h.clear();
    h.feed(":Eve!e@h MODE #lyon -o Bob");
    assert_eq!(h.sends(), vec!["MODE #lyon +o Bob"]);
    h.feed(":Bob!bob@bob.host MODE #lyon +o Bob");
    h.feed(":Bob!bob@bob.host MODE #lyon -o Bob");
    assert!(h.sends().is_empty(), "il s'est déopé lui-même");
}

#[test]
fn notify_list_api_without_connection() {
    let mut s = settings();
    s.address_book.notify.push(NotifyEntry { nick: "Bob".into(), ..NotifyEntry::default() });
    let h = H::with(s);
    let n = h.c.notify_list();
    assert_eq!(n.len(), 1);
    assert!(n[0].online.is_empty());
}
