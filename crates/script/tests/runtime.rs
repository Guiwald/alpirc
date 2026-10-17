//! Le moteur en marche : événements, popups, timers, tables, fichiers, dialogues, utilisateurs, scripts.

mod common;

use alpirc_script::{Ctx, DialogEvent, EventInfo, EventKind, MenuItem, PopupKind, ScriptError, ScriptKind};
use common::*;

fn text_ev(nick: &str, chan: Option<&str>, text: &str) -> EventInfo {
    EventInfo {
        kind: Some(EventKind::Text),
        conn: Some(1),
        nick: nick.into(),
        address: format!("{}@host.example", nick.to_lowercase()),
        chan: chan.map(|c| c.to_string()),
        target: Some(chan.unwrap_or("Guiwald").to_string()),
        text: text.into(),
        ..Default::default()
    }
}

fn kind_ev(kind: EventKind, nick: &str, chan: Option<&str>) -> EventInfo {
    EventInfo {
        kind: Some(kind),
        conn: Some(1),
        nick: nick.into(),
        address: format!("{}@host.example", nick.to_lowercase()),
        chan: chan.map(|c| c.to_string()),
        target: chan.map(|c| c.to_string()),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Événements
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn text_wildcard_channel() {
    let (mut e, mut h) = setup("on *:TEXT:*hello*:#:{ msg $chan Hi $nick }");
    let r = e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "well HELLO there"));
    assert!(!r.haltdef);
    assert_eq!(h.last(), ("msg".into(), "#test Hi Bob".into()));
    h.clear();
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "bye"));
    assert!(h.cmds.is_empty());
}

#[test]
fn text_targets() {
    let (mut e, mut h) = setup(
        "on *:TEXT:*:?:{ echo -a private $1- }\non *:TEXT:*:#a,#b:{ echo -a listed $chan }\non *:TEXT:*:*:{ echo -a any }",
    );
    e.fire(&mut h, &ctx(), &text_ev("Bob", None, "psst"));
    assert_eq!(h.says(), vec!["private psst"]);
    h.clear();
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#b"), "x"));
    assert_eq!(h.says(), vec!["listed #b"]);
    h.clear();
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#zzz"), "x"));
    assert_eq!(h.says(), vec!["any"]);
}

#[test]
fn text_params_and_event_name() {
    let (mut e, mut h) = setup("on *:TEXT:!say *:#:{ echo -a $event $2- / $0 / $address / $fulladdress / $site / $wildsite }");
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "!say hello world"));
    assert_eq!(
        h.says(),
        vec!["text hello world / 3 / bob@host.example / Bob!bob@host.example / host.example / *!*@host.example"]
    );
}

#[test]
fn not_me_prefix() {
    let (mut e, mut h) = setup("on !*:JOIN:#:{ echo -a joined $nick }");
    e.fire(&mut h, &ctx(), &kind_ev(EventKind::Join, "Guiwald", Some("#test")));
    assert!(h.cmds.is_empty());
    e.fire(&mut h, &ctx(), &kind_ev(EventKind::Join, "Bob", Some("#test")));
    assert_eq!(h.says(), vec!["joined Bob"]);
}

#[test]
fn me_prefix() {
    let (mut e, mut h) = setup("on me:*:JOIN:#:{ echo -a I joined $chan }");
    e.fire(&mut h, &ctx(), &kind_ev(EventKind::Join, "Bob", Some("#test")));
    assert!(h.cmds.is_empty());
    e.fire(&mut h, &ctx(), &kind_ev(EventKind::Join, "Guiwald", Some("#test")));
    assert_eq!(h.says(), vec!["I joined #test"]);
}

#[test]
fn levels_from_users_list() {
    let (mut e, mut h) = setup("on 5:TEXT:!op:#:{ mode # +o $nick | echo -a level $ulevel }");
    e.set_users_text("10:*!*@trusted.host\n");
    let mut ev = text_ev("Bob", Some("#test"), "!op");
    ev.address = "bob@trusted.host".into();
    e.fire(&mut h, &ctx(), &ev);
    assert_eq!(h.find("mode"), vec!["#test +o Bob"]);
    assert_eq!(h.says(), vec!["level 10"]);
    h.clear();
    e.fire(&mut h, &ctx(), &text_ev("Eve", Some("#test"), "!op"));
    assert!(h.cmds.is_empty());
}

#[test]
fn named_and_exact_levels() {
    let (mut e, mut h) = setup("on friend:JOIN:#:{ echo -a friend $nick }\non +1:PART:#:{ echo -a part $nick }");
    e.set_users_text("friend:Bob\n5:Carl\n");
    e.fire(&mut h, &ctx(), &kind_ev(EventKind::Join, "Bob", Some("#test")));
    e.fire(&mut h, &ctx(), &kind_ev(EventKind::Join, "Carl", Some("#test")));
    e.fire(&mut h, &ctx(), &kind_ev(EventKind::Part, "Carl", Some("#test")));
    e.fire(&mut h, &ctx(), &kind_ev(EventKind::Part, "Zed", Some("#test")));
    assert_eq!(h.says(), vec!["friend Bob", "part Zed"]);
}

#[test]
fn haltdef_and_halt_results() {
    let (mut e, mut h) = setup("on ^*:TEXT:hide*:#:{ haltdef }\non ^*:TEXT:stop*:#:{ halt }");
    assert!(e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "hide me")).haltdef);
    assert!(e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "stop it")).haltdef);
    assert!(!e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "show")).haltdef);
}

#[test]
fn amp_prefix_skips_after_haltdef() {
    let mut e = engine();
    let mut h = FakeHost::new();
    e.load_text("a.mrc", ScriptKind::Remote, "on *:TEXT:block*:#:{ haltdef }").unwrap();
    e.load_text("b.mrc", ScriptKind::Remote, "on &*:TEXT:*:#:{ echo -a amp }\non *:TEXT:*:#:{ echo -a plain }").unwrap();
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "block this"));
    assert_eq!(h.says(), vec!["plain"]);
    h.clear();
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "hello"));
    assert_eq!(h.says(), vec!["amp"]);
}

#[test]
fn first_match_per_file() {
    let mut e = engine();
    let mut h = FakeHost::new();
    e.load_text("a.mrc", ScriptKind::Remote, "on *:TEXT:*:#:{ echo -a a1 }\non *:TEXT:*:#:{ echo -a a2 }").unwrap();
    e.load_text("b.mrc", ScriptKind::Remote, "on *:TEXT:nope:#:{ echo -a b0 }\non *:TEXT:*:#:{ echo -a b1 }").unwrap();
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "x"));
    assert_eq!(h.says(), vec!["a1", "b1"]);
}

#[test]
fn groups_off_and_enable() {
    let (mut e, mut h) = setup("#grp off\non *:TEXT:*:#:{ echo -a grouped }\nalias galias echo -a in group\n#grp end\nalias g return $group(#grp)");
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "x"));
    assert!(h.cmds.is_empty());
    assert!(!e.has_alias("galias"));
    assert_eq!(ev(&mut e, &mut h, "$g"), "off");
    run(&mut e, &mut h, "/enable #grp");
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "x"));
    assert_eq!(h.says(), vec!["grouped"]);
    assert!(e.script_text("test.mrc").unwrap().contains("#grp on"));
    assert_eq!(ev(&mut e, &mut h, "$g"), "on");
    run(&mut e, &mut h, "/disable #g*");
    assert!(e.script_text("test.mrc").unwrap().contains("#grp off"));
}

#[test]
fn ctcp_handlers() {
    let (mut e, mut h) = setup(
        "ctcp *:VERSION:*:{ ctcpreply $nick VERSION AlpIRC | haltdef }\nctcp *:PING:*:{ echo -a ping $2 from $nick / $1 }",
    );
    let mut ev = kind_ev(EventKind::Ctcp, "Bob", None);
    ev.what = "VERSION".into();
    assert!(e.fire(&mut h, &ctx(), &ev).haltdef);
    assert_eq!(h.last(), ("ctcpreply".into(), "Bob VERSION AlpIRC".into()));
    ev.what = "PING".into();
    ev.text = "12345".into();
    e.fire(&mut h, &ctx(), &ev);
    assert_eq!(h.last(), ("echo".into(), "-a ping 12345 from Bob / PING".into()));
}

#[test]
fn raw_handlers() {
    let (mut e, mut h) = setup("raw 001:*:{ echo -s welcome $1 / $numeric }\nraw *:*:{ echo -s any $numeric }");
    let ev = EventInfo { kind: Some(EventKind::Raw), what: "001".into(), text: "Guiwald :Welcome".into(), ..Default::default() };
    e.fire(&mut h, &ctx(), &ev);
    assert_eq!(h.find("echo"), vec!["-s welcome Guiwald / 001"]);
    let ev = EventInfo { kind: Some(EventKind::Raw), what: "372".into(), text: "x".into(), ..Default::default() };
    e.fire(&mut h, &ctx(), &ev);
    assert_eq!(h.find("echo").last().unwrap(), "-s any 372");
}

#[test]
fn signal_command() {
    let (mut e, mut h) = setup("on *:SIGNAL:test:{ echo -a got $1- from $signal }\non *:SIGNAL:other*:{ echo -a other }");
    run(&mut e, &mut h, "/signal test a b");
    run(&mut e, &mut h, "/signal -n otherthing");
    run(&mut e, &mut h, "/signal none");
    assert_eq!(h.says(), vec!["got a b from test", "other"]);
}

#[test]
fn input_haltdef() {
    let (mut e, mut h) = setup("on *:INPUT:#:{ if ($1 == !secret) { haltdef | echo -a blocked } }\non *:INPUT:?:{ echo -a query }");
    let mut ev = EventInfo { kind: Some(EventKind::Input), target: Some("#test".into()), text: "!secret x".into(), ..Default::default() };
    assert!(e.fire(&mut h, &ctx(), &ev).haltdef);
    ev.text = "hi".into();
    assert!(!e.fire(&mut h, &ctx(), &ev).haltdef);
    ev.target = Some("Bob".into());
    e.fire(&mut h, &ctx(), &ev);
    assert_eq!(h.says(), vec!["blocked", "query"]);
}

#[test]
fn kick_nick_and_extra_values() {
    let (mut e, mut h) = setup(
        "on *:KICK:#:{ echo -a $knick kicked by $nick from $chan ( $+ $1- $+ ) }\non *:NICK:{ echo -a $nick is now $newnick }\non *:OP:#:{ echo -a $opnick opped }",
    );
    let mut ev = kind_ev(EventKind::Kick, "Bob", Some("#test"));
    ev.extra = vec![("knick".into(), "Carl".into())];
    ev.text = "bye".into();
    e.fire(&mut h, &ctx(), &ev);
    let mut ev = kind_ev(EventKind::Nick, "Bob", None);
    ev.extra = vec![("newnick".into(), "Robert".into())];
    e.fire(&mut h, &ctx(), &ev);
    let mut ev = kind_ev(EventKind::Op, "Bob", Some("#test"));
    ev.extra = vec![("opnick".into(), "Dave".into())];
    e.fire(&mut h, &ctx(), &ev);
    assert_eq!(h.says(), vec!["Carl kicked by Bob from #test (bye)", "Bob is now Robert", "Dave opped"]);
}

#[test]
fn matchtext_with_variable() {
    let (mut e, mut h) = setup("on *:TEXT:%trigger *:#:{ echo -a trig $2 }");
    run(&mut e, &mut h, "/.set %trigger !go");
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "!go now"));
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "!stop now"));
    assert_eq!(h.says(), vec!["trig now"]);
}

#[test]
fn regex_matchtext() {
    let (mut e, mut h) = setup("on $*:TEXT:/^!(\\w+)/i:#:{ echo -a cmd $regml(1) }");
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "!Seen bob"));
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "no command"));
    assert_eq!(h.says(), vec!["cmd Seen"]);
}

#[test]
fn word_wildcard_matchtext() {
    let (mut e, mut h) = setup("on *:TEXT:!seen &:#:{ echo -a seen $2 }");
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "!seen alice"));
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "!seen alice bob"));
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "!seen"));
    assert_eq!(h.says(), vec!["seen alice"]);
}

#[test]
fn op_prefix() {
    let (mut e, mut h) = setup("on @*:JOIN:#:{ mode # +v $nick }");
    e.fire(&mut h, &ctx(), &kind_ev(EventKind::Join, "Bob", Some("#test")));
    e.fire(&mut h, &ctx(), &kind_ev(EventKind::Join, "Bob", Some("#other")));
    assert_eq!(h.find("mode"), vec!["#test +v Bob"]);
}

#[test]
fn connect_and_disconnect_timers() {
    let (mut e, mut h) = setup("on *:CONNECT:{ .timeronline 0 60 echo -a on | .timeroff -o 0 60 echo -a off }");
    e.fire(&mut h, &ctx(), &kind_ev(EventKind::Connect, "", None));
    assert_eq!(ev(&mut e, &mut h, "$timer(0)"), "2");
    e.fire(&mut h, &ctx(), &kind_ev(EventKind::Disconnect, "", None));
    assert_eq!(ev(&mut e, &mut h, "$timer(0) $timer(off)"), "1 off");
}

#[test]
fn on_load_and_start_are_pending() {
    let mut e = engine();
    let mut h = FakeHost::new();
    e.load_text("l.mrc", ScriptKind::Remote, "on *:LOAD:{ echo -a loaded }\non *:START:{ echo -a started }").unwrap();
    assert!(e.runs_at_startup("l.mrc"));
    assert_eq!(e.next_timer_due(), Some(0));
    run(&mut e, &mut h, "/echo -a first");
    assert_eq!(h.says(), vec!["loaded", "started", "first"]);
    // Rechargé sous le même nom : pas de nouvel `on LOAD`.
    e.load_text("l.mrc", ScriptKind::Remote, "on *:LOAD:{ echo -a again }").unwrap();
    e.flush_pending(&mut h);
    assert_eq!(h.says().len(), 3);
    // Rétabli au démarrage : rien en attente.
    e.restore_text("r.mrc", ScriptKind::Remote, "on *:LOAD:{ echo -a never }").unwrap();
    e.flush_pending(&mut h);
    assert_eq!(h.says().len(), 3);
    // Refusé par l'utilisateur.
    e.load_text("d.mrc", ScriptKind::Remote, "on *:LOAD:{ echo -a discarded }").unwrap();
    e.discard_pending();
    e.flush_pending(&mut h);
    assert_eq!(h.says().len(), 3);
}

#[test]
fn on_unload() {
    let (mut e, mut h) = setup("on *:UNLOAD:{ echo -a bye }");
    assert!(e.unload("test.mrc"));
    assert!(!e.unload("test.mrc"));
    e.flush_pending(&mut h);
    assert_eq!(h.says(), vec!["bye"]);
    assert!(e.scripts().is_empty());
}

#[test]
fn unknown_events_load() {
    let (mut e, mut h) = setup("on *:SOCKREAD:sock:{ echo -a never }\non *:HOTLINK:*:*:{ echo -a never }\nalias ok echo -a ok");
    run(&mut e, &mut h, "/ok");
    assert_eq!(h.says(), vec!["ok"]);
}

#[test]
fn handler_errors_are_reported() {
    let (mut e, mut h) = setup("on *:TEXT:*:#:{\n  echo -a before\n  fail\n}");
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "x"));
    assert_eq!(h.errors, vec!["* /fail: insufficient parameters (line 3, test.mrc)"]);
}

#[test]
fn script_and_scriptline() {
    let (mut e, mut h) = setup("\n\non *:TEXT:*:#:{\n  echo -a $script $scriptline\n}");
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "x"));
    assert_eq!(h.says(), vec!["test.mrc 4"]);
}

#[test]
fn hash_channel_shortcut() {
    let (mut e, mut h) = setup("on *:TEXT:*:#:{ msg # seen | join #$1 | join #$2 }");
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#lyon"), "paris #nice"));
    assert_eq!(h.names(), vec!["msg", "join", "join"]);
    assert_eq!(h.find("msg"), vec!["#lyon seen"]);
    assert_eq!(h.find("join"), vec!["#paris", "#nice"]);
}

#[test]
fn notify_and_misc_events() {
    let (mut e, mut h) = setup(
        "on *:NOTIFY:{ echo -a notify $nick }\non *:TOPIC:#:{ echo -a topic $1- }\non *:ACTION:*dances*:#:{ echo -a action }\non *:NOTICE:*:?:{ echo -a notice $1 }",
    );
    e.fire(&mut h, &ctx(), &kind_ev(EventKind::Notify, "Bob", None));
    let mut t = kind_ev(EventKind::Topic, "Bob", Some("#test"));
    t.text = "new topic".into();
    e.fire(&mut h, &ctx(), &t);
    let mut a = text_ev("Bob", Some("#test"), "dances around");
    a.kind = Some(EventKind::Action);
    e.fire(&mut h, &ctx(), &a);
    let mut n = text_ev("Bob", None, "hey");
    n.kind = Some(EventKind::Notice);
    e.fire(&mut h, &ctx(), &n);
    assert_eq!(h.says(), vec!["notify Bob", "topic new topic", "action", "notice hey"]);
}

#[test]
fn keydown_and_open_events() {
    let (mut e, mut h) = setup("on *:KEYDOWN:@win:13,32:{ echo -a key $keyval }\non *:OPEN:?:*hi*:{ echo -a opened $nick }");
    let mut k = EventInfo { kind: Some(EventKind::KeyDown), target: Some("@win".into()), ..Default::default() };
    k.extra = vec![("keyval".into(), "13".into())];
    e.fire(&mut h, &ctx(), &k);
    k.extra = vec![("keyval".into(), "65".into())];
    e.fire(&mut h, &ctx(), &k);
    let mut o = text_ev("Bob", None, "oh hi");
    o.kind = Some(EventKind::Open);
    o.target = Some("Bob".into());
    e.fire(&mut h, &ctx(), &o);
    assert_eq!(h.says(), vec!["key 13", "opened Bob"]);
}

// ---------------------------------------------------------------------------------------------------------------
// Popups
// ---------------------------------------------------------------------------------------------------------------

const MENU: &str = "menu nicklist {
  Whois:whois $$1
  Control
  .Op:mode # +o $$1
  .Deop:mode # -o $$1
  ..Deep:echo -a deep
  -
  $iif($me == Guiwald,Mine,NotMine):echo -a mine
  $iif($me == Nobody,Hidden):echo -a hidden
  $style(1) Checked:echo -a c
  $style(2) Disabled:echo -a d
  $iif(0,Gone)
  .Child:echo -a child
  Multi:{
    echo -a m1 $1
    echo -a m2 $snicks
  }
}
menu channel,status {
  Part:part #
}
";

fn labels(items: &[MenuItem]) -> Vec<String> {
    items.iter().map(|i| i.label.clone()).collect()
}

fn find<'a>(items: &'a [MenuItem], label: &str) -> &'a MenuItem {
    items.iter().find(|i| i.label == label).unwrap_or_else(|| panic!("pas d'entrée {label}"))
}

#[test]
fn popup_structure() {
    let (mut e, mut h) = setup(MENU);
    let c = Ctx { selected: vec!["Bob".into()], ..ctx() };
    let m = e.popup(&mut h, &c, PopupKind::Nicklist, None);
    assert_eq!(labels(&m), vec!["Whois", "Control", "-", "Mine", "Checked", "Disabled", "Multi"]);
    let control = find(&m, "Control");
    assert_eq!(control.action, None);
    assert_eq!(labels(&control.children), vec!["Op", "Deop"]);
    assert_eq!(labels(&control.children[1].children), vec!["Deep"]);
    assert!(find(&m, "Checked").checked);
    assert!(!find(&m, "Disabled").enabled);
    assert!(find(&m, "Whois").enabled);
    assert!(find(&m, "Whois").action.is_some());
}

#[test]
fn popup_run_with_selected_nick() {
    let (mut e, mut h) = setup(MENU);
    let c = Ctx { selected: vec!["Bob".into(), "Carl".into()], ..ctx() };
    let m = e.popup(&mut h, &c, PopupKind::Nicklist, None);
    e.popup_run(&mut h, &c, find(&m, "Whois").action.unwrap()).unwrap();
    assert_eq!(h.last(), ("whois".into(), "Bob".into()));
    let op = &find(&m, "Control").children[0];
    e.popup_run(&mut h, &c, op.action.unwrap()).unwrap();
    assert_eq!(h.last(), ("mode".into(), "#test +o Bob".into()));
    e.popup_run(&mut h, &c, find(&m, "Multi").action.unwrap()).unwrap();
    assert_eq!(h.says(), vec!["m1 Bob", "m2 Bob,Carl"]);
    assert!(e.popup_run(&mut h, &c, 999).is_err());
}

#[test]
fn popup_kinds_combined() {
    let (mut e, mut h) = setup(MENU);
    assert_eq!(labels(&e.popup(&mut h, &ctx(), PopupKind::Channel, None)), vec!["Part"]);
    assert_eq!(labels(&e.popup(&mut h, &ctx(), PopupKind::Status, None)), vec!["Part"]);
    assert!(e.popup(&mut h, &ctx(), PopupKind::Query, None).is_empty());
    assert!(e.popup(&mut h, &ctx(), PopupKind::Menubar, None).is_empty());
}

#[test]
fn popups_file_sections() {
    let mut e = engine();
    let mut h = FakeHost::new();
    e.load_text("popups.txt", ScriptKind::Popups, "[channel]\nTopic:topic # $?=\"Topic\"\n[query]\nWhois:whois $1\n.Sub:echo -a sub")
        .unwrap();
    let m = e.popup(&mut h, &ctx(), PopupKind::Channel, None);
    assert_eq!(labels(&m), vec!["Topic"]);
    e.popup_run(&mut h, &ctx(), m[0].action.unwrap()).unwrap();
    assert_eq!(h.last(), ("topic".into(), "#test answer".into()));
    let q = e.popup(&mut h, &ctx(), PopupKind::Query, None);
    assert_eq!(labels(&q), vec!["Whois"]);
    assert_eq!(labels(&q[0].children), vec!["Sub"]);
}

#[test]
fn popups_ini_format() {
    let mut e = engine();
    let mut h = FakeHost::new();
    e.load_text("popups.ini", ScriptKind::Popups, "[cpopup]\nn0=Hello:say hello\nn1=-\nn2=Bye:part #\n[lpopup]\nn0=Slap:me slaps $$1")
        .unwrap();
    assert_eq!(labels(&e.popup(&mut h, &ctx(), PopupKind::Channel, None)), vec!["Hello", "-", "Bye"]);
    assert_eq!(labels(&e.popup(&mut h, &ctx(), PopupKind::Nicklist, None)), vec!["Slap"]);
}

#[test]
fn custom_window_menu() {
    let (mut e, mut h) = setup("menu @mywin {\n  dclick:echo -a dbl\n  Clear:clear @mywin\n}\nmenu @other { Nope:noop }");
    let m = e.popup(&mut h, &ctx(), PopupKind::Custom, Some("@mywin"));
    assert_eq!(labels(&m), vec!["Clear"]);
}

#[test]
fn dynamic_submenu() {
    let (mut e, mut h) = setup(
        "menu channel {\n  Chans\n  .$submenu($chans($1))\n}\nalias chans {\n  if ($1 == begin) return -\n  if ($1 isnum 1-2) return #c $+ $1 $+ :join #c $+ $1\n  if ($1 == end) return -\n}",
    );
    let m = e.popup(&mut h, &ctx(), PopupKind::Channel, None);
    assert_eq!(labels(&m), vec!["Chans"]);
    assert_eq!(labels(&m[0].children), vec!["-", "#c1", "#c2", "-"]);
    e.popup_run(&mut h, &ctx(), m[0].children[2].action.unwrap()).unwrap();
    assert_eq!(h.last(), ("join".into(), "#c2".into()));
}

#[test]
fn menu_query_label_uses_identifiers() {
    let (mut e, mut h) = setup("menu query {\n  Talk to $1:msg $1 hi\n  $iif(%away,Back,Away):echo -a toggle\n}");
    let c = Ctx { selected: vec!["Bob".into()], ..ctx() };
    assert_eq!(labels(&e.popup(&mut h, &c, PopupKind::Query, None)), vec!["Talk to Bob", "Away"]);
    run(&mut e, &mut h, "/.set %away 1");
    assert_eq!(labels(&e.popup(&mut h, &c, PopupKind::Query, None)), vec!["Talk to Bob", "Back"]);
}

// ---------------------------------------------------------------------------------------------------------------
// Timers
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn timer_repetitions() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    run(&mut e, &mut h, "/timer 3 1 echo -a hi");
    assert_eq!(h.echoes(), vec!["* Timer 1 activated"]);
    let t0 = h.now;
    assert_eq!(e.next_timer_due(), Some(t0 + 1000));
    e.tick(&mut h, t0 + 500);
    assert!(h.says().is_empty());
    for k in 1..=4 {
        e.tick(&mut h, t0 + k * 1000);
    }
    assert_eq!(h.says(), vec!["hi", "hi", "hi"]);
    assert_eq!(ev(&mut e, &mut h, "$timer(0)"), "0");
    assert_eq!(e.next_timer_due(), None);
}

#[test]
fn named_timer_off() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    run(&mut e, &mut h, "/timerfoo 0 1 echo -a x");
    run(&mut e, &mut h, "/.timer 0 5 echo -a y");
    assert_eq!(ev(&mut e, &mut h, "$timer(foo) $timer(0) $timer(2)"), "foo 2 1");
    run(&mut e, &mut h, "/timerfoo off");
    assert_eq!(h.echoes().last().unwrap(), "* Timer foo halted");
    assert_eq!(ev(&mut e, &mut h, "$timer(foo) $timer(0)"), " 1");
    run(&mut e, &mut h, "/timers off");
    assert_eq!(ev(&mut e, &mut h, "$timer(0)"), "0");
}

#[test]
fn timer_milliseconds() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    run(&mut e, &mut h, "/.timer -m 2 500 echo -a ms");
    let t0 = h.now;
    e.tick(&mut h, t0 + 500);
    e.tick(&mut h, t0 + 1000);
    assert_eq!(h.says(), vec!["ms", "ms"]);
}

#[test]
fn timer_evaluation_twice() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    run(&mut e, &mut h, "/.set %v 1");
    run(&mut e, &mut h, "//.timer 1 1 echo -a %v $!calc(1+1) $!me");
    run(&mut e, &mut h, "/.set %v 2");
    assert_eq!(ev(&mut e, &mut h, "$timer(1).com"), "echo -a 1 $calc(1+1) $me");
    let t = h.now + 1000;
    e.tick(&mut h, t);
    assert_eq!(h.says(), vec!["1 2 Guiwald"]);
}

#[test]
fn timer_props() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    run(&mut e, &mut h, "/.timerx 5 10 echo -a x");
    assert_eq!(ev(&mut e, &mut h, "$timer(x).secs $timer(x).reps $timer(x).delay $timer(x).type"), "10 5 10 online");
    h.now += 4000;
    assert_eq!(ev(&mut e, &mut h, "$timer(x).secs"), "6");
    assert_eq!(ev(&mut e, &mut h, "$ltimer"), "x");
}

#[test]
fn timer_pause_resume() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    run(&mut e, &mut h, "/.timerp 1 10 echo -a fired");
    let t0 = h.now;
    h.now = t0 + 4000;
    run(&mut e, &mut h, "/timerp -p");
    e.tick(&mut h, t0 + 20_000);
    assert!(h.says().is_empty());
    h.now = t0 + 20_000;
    run(&mut e, &mut h, "/timerp -r");
    e.tick(&mut h, t0 + 25_000);
    assert!(h.says().is_empty());
    e.tick(&mut h, t0 + 26_000);
    assert_eq!(h.says(), vec!["fired"]);
}

#[test]
fn timer_stops_itself_and_lists() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    // Le ` | ` tapé sépare les commandes tout de suite ; `$chr(124)` le met dans la commande du timer.
    run(&mut e, &mut h, "//.timerself 0 1 echo -a tick $chr(124) .timerself off");
    run(&mut e, &mut h, "/.timerlist 0 30 echo -a l");
    let t = h.now + 1000;
    e.tick(&mut h, t);
    e.tick(&mut h, t + 1000);
    assert_eq!(h.says(), vec!["tick"]);
    h.clear();
    run(&mut e, &mut h, "/timers");
    assert_eq!(h.echoes(), vec!["* Active timers:", "* Timer list 30s delay echo -a l"]);
}

#[test]
fn timer_at_clock_time() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    // Maintenant : 22:13:20 UTC ; 23:00 → dans 46 min 40 s.
    run(&mut e, &mut h, "/.timerat 23:00 1 1 echo -a late");
    assert_eq!(e.next_timer_due(), Some(h.now + 2_800_000));
}

#[test]
fn timer_errors() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    assert!(run_err(&mut e, &mut h, "/timer x 1 echo").contains("/timer"));
    assert!(run_err(&mut e, &mut h, "/timer 1 1").contains("/timer"));
}

#[test]
fn timer_command_errors_are_reported() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    run(&mut e, &mut h, "/.timer 1 1 fail");
    let t = h.now + 1000;
    e.tick(&mut h, t);
    assert_eq!(h.errors, vec!["* /fail: insufficient parameters"]);
}

// ---------------------------------------------------------------------------------------------------------------
// Tables de hachage
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn hash_basics() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    run(&mut e, &mut h, "/hmake t 10 | hadd t a 1 | hadd t b two words");
    assert_eq!(ev(&mut e, &mut h, "$hget(t,a) / $hget(t,B) / $hget(t,0).item"), "1 / two words / 2");
    assert_eq!(ev(&mut e, &mut h, "$hget(t,1).item $hget(t,2).data"), "a two words");
    assert_eq!(ev(&mut e, &mut h, "$hget(t) $hget(t).size $hget(0) $hget(1)"), "t 10 1 t");
    run(&mut e, &mut h, "/hadd t a 3");
    assert_eq!(ev(&mut e, &mut h, "$hget(t,a) $hget(t,0).item"), "3 2");
}

#[test]
fn hash_del_and_free() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    run(&mut e, &mut h, "/hmake t | hadd t a 1 | hadd t ab 2 | hadd t b 3");
    run(&mut e, &mut h, "/hdel t a");
    assert_eq!(ev(&mut e, &mut h, "< $+ $hget(t,a) $+ > $hget(t,0).item"), "<> 2");
    run(&mut e, &mut h, "/hdel -w t a*");
    assert_eq!(ev(&mut e, &mut h, "$hget(t,0).item"), "1");
    run(&mut e, &mut h, "/hfree t");
    assert_eq!(ev(&mut e, &mut h, "$hget(t)"), "");
    assert!(run_err(&mut e, &mut h, "/hfree t").contains("no such table"));
}

#[test]
fn hash_find() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    run(&mut e, &mut h, "/hmake t | hadd t apple red | hadd t avocado green | hadd t banana yellow");
    assert_eq!(ev(&mut e, &mut h, "$hfind(t,a*,0,w)"), "2");
    assert_eq!(ev(&mut e, &mut h, "$hfind(t,a*,2,w)"), "avocado");
    assert_eq!(ev(&mut e, &mut h, "$hfind(t,banana,1)"), "banana");
    assert_eq!(ev(&mut e, &mut h, "$hfind(t,*ee*,1,w).data"), "avocado");
    assert_eq!(ev(&mut e, &mut h, "$hfind(t,^b,1,r)"), "banana");
}

#[test]
fn hash_errors_and_make_switch() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    assert!(run_err(&mut e, &mut h, "/hadd nope a 1").contains("no such table 'nope'"));
    run(&mut e, &mut h, "/hadd -m auto k v");
    assert_eq!(ev(&mut e, &mut h, "$hget(auto,k)"), "v");
    assert!(run_err(&mut e, &mut h, "/hmake auto").contains("exists"));
}

#[test]
fn hash_inc_dec() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    run(&mut e, &mut h, "/hinc -m cnt x | hinc cnt x 5 | hdec cnt x 2");
    assert_eq!(ev(&mut e, &mut h, "$hget(cnt,x)"), "4");
}

#[test]
fn hash_save_load() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    run(&mut e, &mut h, "/hmake t | hadd t k1 v1 | hadd t k2 value two");
    run(&mut e, &mut h, "/hsave t data/table.txt");
    run(&mut e, &mut h, "/hsave -i t table.ini sect");
    run(&mut e, &mut h, "/hsave -n t lines.txt");
    let saved = std::fs::read_to_string(e.root().join("data/table.txt")).unwrap();
    assert_eq!(saved, "k1\nv1\nk2\nvalue two\n");
    run(&mut e, &mut h, "/hfree t | hmake t2 | hload t2 data/table.txt");
    assert_eq!(ev(&mut e, &mut h, "$hget(t2,k2) $hget(t2,0).item"), "value two 2");
    run(&mut e, &mut h, "/hload -mi t3 table.ini sect");
    assert_eq!(ev(&mut e, &mut h, "$hget(t3,k1)"), "v1");
    run(&mut e, &mut h, "/hload -mn t4 lines.txt");
    assert_eq!(ev(&mut e, &mut h, "$hget(t4,2)"), "value two");
}

#[test]
fn hash_sandbox() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    run(&mut e, &mut h, "/hmake t | hadd t k v");
    let err = e.run_line(&mut h, &ctx(), "/hsave t ../../evil.txt").unwrap_err();
    assert!(matches!(err, ScriptError::Sandbox(_)), "{err:?}");
}

#[test]
fn hash_item_expiry() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    run(&mut e, &mut h, "/hadd -mu5 t k v");
    let t = h.now;
    e.tick(&mut h, t + 4000);
    assert_eq!(ev(&mut e, &mut h, "$hget(t,k)"), "v");
    e.tick(&mut h, t + 5000);
    assert_eq!(ev(&mut e, &mut h, "$hget(t,k)"), "");
}

// ---------------------------------------------------------------------------------------------------------------
// Fichiers
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn write_and_read() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    run(&mut e, &mut h, "/write test.txt line1 | write test.txt line2");
    assert_eq!(ev(&mut e, &mut h, "$read(test.txt,n,1)"), "line1");
    assert_eq!(ev(&mut e, &mut h, "$read(test.txt,2)"), "line2");
    assert_eq!(ev(&mut e, &mut h, "$readn"), "2");
    assert_eq!(ev(&mut e, &mut h, "$lines(test.txt)"), "2");
    let r = ev(&mut e, &mut h, "$read(test.txt)");
    assert!(r == "line1" || r == "line2");
    assert_eq!(ev(&mut e, &mut h, "$read(nothere.txt)"), "");
    assert_eq!(ev(&mut e, &mut h, "$lines(nothere.txt)"), "0");
}

#[test]
fn read_scan_and_wildcard() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    run(&mut e, &mut h, "/write db.txt bob likes cheese | write db.txt alice likes tea | write db.txt carl 42");
    assert_eq!(ev(&mut e, &mut h, "$read(db.txt,s,alice)"), "likes tea");
    assert_eq!(ev(&mut e, &mut h, "$readn"), "2");
    assert_eq!(ev(&mut e, &mut h, "$read(db.txt,w,*42*)"), "carl 42");
    assert_eq!(ev(&mut e, &mut h, "$read(db.txt,r,/^b/)"), "bob likes cheese");
    assert_eq!(ev(&mut e, &mut h, "$read(db.txt,s,zed)"), "");
}

#[test]
fn read_evaluates_unless_n() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    std::fs::write(e.root().join("ev.txt"), "$me is here\n").unwrap();
    assert_eq!(ev(&mut e, &mut h, "$read(ev.txt,1)"), "Guiwald is here");
    assert_eq!(ev(&mut e, &mut h, "$read(ev.txt,n,1)"), "$me is here");
}

#[test]
fn read_count_first_line() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    std::fs::write(e.root().join("c.txt"), "2\nfirst\nsecond\n").unwrap();
    assert_eq!(ev(&mut e, &mut h, "$read(c.txt,1)"), "first");
    assert_eq!(ev(&mut e, &mut h, "$read(c.txt,t,1)"), "2");
    assert_eq!(ev(&mut e, &mut h, "$read(c.txt,0)"), "2");
}

#[test]
fn write_switches() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    run(&mut e, &mut h, "/write f.txt a | write f.txt b | write f.txt c");
    run(&mut e, &mut h, "/write -l2 f.txt B");
    run(&mut e, &mut h, "/write -il1 f.txt first");
    run(&mut e, &mut h, "/write -dl3 f.txt");
    run(&mut e, &mut h, "/write -a f.txt !");
    run(&mut e, &mut h, "/write -sfirst f.txt FIRST line");
    let t = std::fs::read_to_string(e.root().join("f.txt")).unwrap();
    assert_eq!(t, "FIRST line\na\nc!\n");
    run(&mut e, &mut h, "/write -c f.txt");
    assert_eq!(std::fs::read_to_string(e.root().join("f.txt")).unwrap(), "");
}

#[test]
fn sandbox_escapes_rejected() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    for line in [
        "/write ../escape.txt x",
        "/write /etc/passwd x",
        "/write C:\\mirc\\x.txt x",
        "/write sub/../../x.txt x",
        "//echo $read(/etc/passwd)",
        "/remove ../../x",
        "/rename a.txt ../b.txt",
        "/write \"..\\evil.txt\" x",
    ] {
        let err = e.run_line(&mut h, &ctx(), line).unwrap_err();
        assert!(matches!(err, ScriptError::Sandbox(_)), "{line}: {err:?}");
    }
    assert!(!e.root().parent().unwrap().join("escape.txt").exists());
    assert_eq!(ev(&mut e, &mut h, "$isfile(/etc/passwd) $exists(../)"), "$false $false");
}

#[test]
fn sandbox_symlink_escape() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    let outside = tmpdir();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside, e.root().join("link")).unwrap();
        let err = e.run_line(&mut h, &ctx(), "/write link/x.txt hi").unwrap_err();
        assert!(matches!(err, ScriptError::Sandbox(_)), "{err:?}");
        assert!(!outside.join("x.txt").exists());
    }
}

#[test]
fn absolute_path_inside_root_is_fine() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    run(&mut e, &mut h, "//write $mircdir $+ abs.txt ok");
    assert_eq!(ev(&mut e, &mut h, "$read($mircdir $+ abs.txt,n,1)"), "ok");
    run(&mut e, &mut h, "/write sub\\win.txt backslash");
    assert!(e.root().join("sub/win.txt").exists());
}

#[test]
fn file_identifiers_and_commands() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    run(&mut e, &mut h, "/write a.txt hello");
    assert_eq!(ev(&mut e, &mut h, "$isfile(a.txt) $exists(a.txt) $isdir(a.txt)"), "$true $true $false");
    assert_eq!(ev(&mut e, &mut h, "$file(a.txt).size $file(a.txt) $file(a.txt).ext"), "6 6 .txt");
    run(&mut e, &mut h, "/mkdir sub | copy a.txt sub | rename a.txt b.txt");
    assert_eq!(ev(&mut e, &mut h, "$isdir(sub) $isfile(sub/a.txt) $isfile(a.txt) $isfile(b.txt)"), "$true $true $false $true");
    run(&mut e, &mut h, "/remove b.txt");
    assert_eq!(ev(&mut e, &mut h, "$isfile(b.txt)"), "$false");
    assert_eq!(ev(&mut e, &mut h, "$nopath(dir/sub/file.txt) $nofile(dir/sub/file.txt)"), "file.txt dir/sub/");
    assert_eq!(ev(&mut e, &mut h, "$mkfn(a:b?c)"), "a_b_c");
}

#[test]
fn ini_files() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    run(&mut e, &mut h, "/writeini conf.ini main nick Guiwald | writeini conf.ini main color 4 | writeini conf.ini other x 1");
    assert_eq!(ev(&mut e, &mut h, "$readini(conf.ini,main,nick) $readini(conf.ini,n,main,color)"), "Guiwald 4");
    assert_eq!(ev(&mut e, &mut h, "$ini(conf.ini,0) $ini(conf.ini,2) $ini(conf.ini,main,0) $ini(conf.ini,main,2)"), "2 other 2 color");
    run(&mut e, &mut h, "/remini conf.ini main color");
    assert_eq!(ev(&mut e, &mut h, "$ini(conf.ini,main,0)"), "1");
    run(&mut e, &mut h, "/remini conf.ini other");
    assert_eq!(ev(&mut e, &mut h, "$ini(conf.ini,0)"), "1");
}

#[test]
fn mircdir_and_scriptdir() {
    let mut e = engine();
    let mut h = FakeHost::new();
    std::fs::create_dir_all(e.root().join("scripts")).unwrap();
    std::fs::write(e.root().join("scripts/s.mrc"), "alias where return $scriptdir").unwrap();
    let name = e.load_file(std::path::Path::new("scripts/s.mrc"), false).unwrap();
    assert_eq!(name, "scripts/s.mrc");
    let root = e.root().display().to_string();
    assert_eq!(ev(&mut e, &mut h, "$mircdir"), format!("{root}/"));
    assert_eq!(ev(&mut e, &mut h, "$where"), format!("{root}/scripts/"));
}

// ---------------------------------------------------------------------------------------------------------------
// Dialogues
// ---------------------------------------------------------------------------------------------------------------

const DIALOG: &str = "dialog test {
  title \"My dialog\"
  size -1 -1 200 120
  option dbu
  tab \"General\", 1, 2 2 196 100
  tab \"More\", 2
  text \"Name:\", 3, 10 20 30 10, tab 1
  edit \"\", 4, 40 20 80 10, tab 1
  list 5, 10 20 100 50, tab 2
  check \"Remember\", 6, 10 40 60 10, tab 1
  button \"Apply\", 7, 100 105 40 12
  button \"OK\", 8, 150 105 40 12, ok
}
on *:DIALOG:test:init:0:{ did -a test 5 first | did -a test 5 second | echo -a init $dname }
on *:DIALOG:test:sclick:7:{ echo -a apply $did(4).text $did(test,5).sel }
on *:DIALOG:test:sclick:8:{ echo -a ok pressed }
on *:DIALOG:test:edit:4:{ echo -a typed $did(4) }
on *:DIALOG:test:close:0:{ echo -a closed $devent }
";

#[test]
fn dialog_open_and_init() {
    let (mut e, mut h) = setup(DIALOG);
    let v0 = e.dialogs_version();
    run(&mut e, &mut h, "/dialog -m test test");
    assert!(e.dialogs_version() > v0);
    assert_eq!(h.says(), vec!["init test"]);
    let d = &e.dialogs()[0];
    assert_eq!(d.title, "My dialog");
    assert!(d.dbu);
    assert_eq!(d.active_tab, Some(1));
    assert_eq!(d.controls.len(), 8);
    let list = d.controls.iter().find(|c| c.id == 5).unwrap();
    assert_eq!(list.items, vec!["first", "second"]);
    assert_eq!(list.tab, Some(2));
    assert_eq!(ev(&mut e, &mut h, "$did(test,5).lines $did(test,5,2) $dialog(0) $dialog(test).title"), "2 second 1 My dialog");
    // Le client l'ouvre et envoie Init : déjà fait, rien de plus.
    e.dialog_event(&mut h, "test", DialogEvent::Init);
    assert_eq!(h.says().len(), 1);
}

#[test]
fn dialog_did_and_click() {
    let (mut e, mut h) = setup(DIALOG);
    run(&mut e, &mut h, "/dialog -m test test");
    run(&mut e, &mut h, "/did -c test 5 2");
    assert_eq!(ev(&mut e, &mut h, "$did(test,5).sel $did(test,5).seltext $did(test,5)"), "2 second second");
    e.dialog_mut("test").unwrap().controls.iter_mut().find(|c| c.id == 4).unwrap().text = "Bob".into();
    e.dialog_event(&mut h, "test", DialogEvent::Edit(4));
    e.dialog_event(&mut h, "test", DialogEvent::Click(7));
    assert_eq!(h.says()[1..], ["typed Bob", "apply Bob 2"]);
    assert_eq!(e.dialogs().len(), 1);
}

#[test]
fn dialog_ok_button_closes() {
    let (mut e, mut h) = setup(DIALOG);
    run(&mut e, &mut h, "/dialog -m test test");
    e.dialog_event(&mut h, "test", DialogEvent::Click(8));
    assert_eq!(h.says()[1..], ["ok pressed", "closed close"]);
    assert!(e.dialogs().is_empty());
    assert_eq!(ev(&mut e, &mut h, "$dialog(test)"), "");
}

#[test]
fn dialog_close_event() {
    let (mut e, mut h) = setup(DIALOG);
    run(&mut e, &mut h, "/dialog -m test test");
    e.dialog_event(&mut h, "test", DialogEvent::Close);
    assert_eq!(h.says()[1..], ["closed close"]);
    assert!(e.dialogs().is_empty());
}

#[test]
fn dialog_did_switches() {
    let (mut e, mut h) = setup(DIALOG);
    run(&mut e, &mut h, "/dialog -m test test");
    run(&mut e, &mut h, "/did -b test 7 | did -h test 3 | did -c test 6");
    assert_eq!(ev(&mut e, &mut h, "$did(test,7).enabled $did(test,3).visible $did(test,6).state"), "$false $false 1");
    run(&mut e, &mut h, "/did -e test 7 | did -v test 3 | did -u test 6");
    assert_eq!(ev(&mut e, &mut h, "$did(test,7).enabled $did(test,3).visible $did(test,6).state"), "$true $true 0");
    run(&mut e, &mut h, "/did -o test 5 1 premier | did -i test 5 1 zero");
    assert_eq!(ev(&mut e, &mut h, "$didtok(test,5,44)"), "zero,premier,second");
    run(&mut e, &mut h, "/did -d test 5 1");
    assert_eq!(ev(&mut e, &mut h, "$didtok(test,5,44) $didwm(test,5,sec*)"), "premier,second 2");
    run(&mut e, &mut h, "/did -ra test 4 new text");
    assert_eq!(ev(&mut e, &mut h, "$did(test,4) $did(test,4).len"), "new text 8");
    run(&mut e, &mut h, "/did -r test 5");
    assert_eq!(ev(&mut e, &mut h, "$did(test,5).lines"), "0");
    run(&mut e, &mut h, "/did -m test 4");
    assert!(e.dialogs()[0].controls.iter().find(|c| c.id == 4).unwrap().has_style("read"));
    assert_eq!(ev(&mut e, &mut h, "$did(test,99).isid $did(test,4).isid"), "$false $true");
}

#[test]
fn dialog_commands() {
    let (mut e, mut h) = setup(DIALOG);
    run(&mut e, &mut h, "/dialog -m test test");
    run(&mut e, &mut h, "/dialog -t test New title");
    assert_eq!(ev(&mut e, &mut h, "$dialog(test).title $dialog(1) $dialog(test).table"), "New title test test");
    assert!(run_err(&mut e, &mut h, "/dialog -m test test").contains("in use"));
    run(&mut e, &mut h, "/dialog -x test");
    assert!(e.dialogs().is_empty());
    assert!(!h.says().contains(&"closed close".to_string()));
    assert!(run_err(&mut e, &mut h, "/dialog -m x nosuch").contains("no such table"));
}

#[test]
fn dialog_modal_identifier_and_tabs() {
    let (mut e, mut h) = setup(DIALOG);
    assert_eq!(ev(&mut e, &mut h, "< $+ $dialog(m,test) $+ >"), "<>");
    assert!(e.dialogs()[0].modal);
    e.dialog_event(&mut h, "m", DialogEvent::Click(2));
    assert_eq!(e.dialogs()[0].active_tab, Some(2));
}

// ---------------------------------------------------------------------------------------------------------------
// Utilisateurs
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn users_text_roundtrip() {
    let mut e = engine();
    e.set_users_text("5,friend:*!*@a.host Some info\n10:Boss\n[users]\nn0=3:*!*@ini.host\n");
    assert_eq!(e.users_text(), "5,friend:*!*@a.host Some info\n10:Boss\n3:*!*@ini.host\n");
}

#[test]
fn users_commands() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    run(&mut e, &mut h, "/auser 5 *!*@host");
    run(&mut e, &mut h, "/auser -a friend *!*@host");
    assert_eq!(e.users_text(), "5,friend:*!*@host\n");
    assert_eq!(ev(&mut e, &mut h, "$level(nick!u@host) $level(other!u@else)"), "5,friend 1");
    run(&mut e, &mut h, "/guser 10 Bob 2");
    assert!(e.users_text().contains("10:Bob!bob@op.host"));
    assert_eq!(ev(&mut e, &mut h, "$ulist(*,5,0) $ulist(*,10,1)"), "1 Bob!bob@op.host");
    run(&mut e, &mut h, "/ruser friend *!*@host");
    assert_eq!(ev(&mut e, &mut h, "$level(nick!u@host)"), "5");
    run(&mut e, &mut h, "/ruser *!*@host");
    assert_eq!(e.users_text(), "10:Bob!bob@op.host\n");
}

#[test]
fn users_protect_operators() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    e.set_users_text("aop:*!*@op.host\nprotect#test:*!*@safe.host\n");
    assert!(cond(&mut e, &mut h, "Bob isaop #test"));
    assert!(cond(&mut e, &mut h, "x!y@op.host isaop"));
    assert!(!cond(&mut e, &mut h, "x!y@op.host isavoice"));
    assert!(cond(&mut e, &mut h, "x!y@safe.host isprotect #test"));
    assert!(!cond(&mut e, &mut h, "x!y@safe.host isprotect #other"));
}

// ---------------------------------------------------------------------------------------------------------------
// Scripts
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn ini_remote_load() {
    let mut e = engine();
    let mut h = FakeHost::new();
    e.load_text(
        "remote.ini",
        ScriptKind::Remote,
        "[script]\nn0=on *:JOIN:#:{\nn1=  msg $chan Welcome $nick\nn2=}\nn3=alias hello echo -a hello",
    )
    .unwrap();
    e.fire(&mut h, &ctx(), &kind_ev(EventKind::Join, "Bob", Some("#test")));
    assert_eq!(h.last(), ("msg".into(), "#test Welcome Bob".into()));
    run(&mut e, &mut h, "/hello");
    assert_eq!(h.last().1, "-a hello");
}

#[test]
fn load_file_sandbox_and_trusted() {
    let mut e = engine();
    std::fs::write(e.root().join("in.mrc"), "alias inside return in").unwrap();
    let out = tmpdir().join("out.mrc");
    std::fs::write(&out, "alias outside return out").unwrap();
    assert_eq!(e.load_file(std::path::Path::new("in.mrc"), false).unwrap(), "in.mrc");
    assert!(matches!(e.load_file(&out, false), Err(ScriptError::Sandbox(_))));
    let name = e.load_file(&out, true).unwrap();
    assert_eq!(name, out.display().to_string());
    assert!(e.has_alias("outside"));
    assert_eq!(e.scripts().len(), 2);
    assert_eq!(e.scripts()[0].kind, ScriptKind::Remote);
    assert_eq!(e.script_text("in.mrc"), Some("alias inside return in"));
    assert!(e.load_file(std::path::Path::new("missing.mrc"), false).is_err());
}

#[test]
fn load_file_latin1_and_ini_kind() {
    let mut e = engine();
    let mut h = FakeHost::new();
    std::fs::write(e.root().join("old.mrc"), b"alias accent return caf\xe9\r\n").unwrap();
    std::fs::write(e.root().join("aliases.ini"), "[aliases]\nn0=/a2 return ini\n").unwrap();
    e.load_file(std::path::Path::new("old.mrc"), false).unwrap();
    e.load_file(std::path::Path::new("aliases.ini"), false).unwrap();
    assert_eq!(ev(&mut e, &mut h, "$accent $a2"), "café ini");
    assert_eq!(e.scripts()[1].kind, ScriptKind::Aliases);
}

#[test]
fn load_command_fires_on_load() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    std::fs::write(e.root().join("x.mrc"), "on *:LOAD:{ echo -a x loaded }\nalias xx return xx").unwrap();
    run(&mut e, &mut h, "/load -rs x.mrc");
    assert_eq!(h.says(), vec!["x loaded"]);
    assert_eq!(ev(&mut e, &mut h, "$xx $script(0) $script(1)"), "xx 1 x.mrc");
    std::fs::write(e.root().join("x.mrc"), "alias xx return reloaded").unwrap();
    run(&mut e, &mut h, "/reload -rs x.mrc");
    assert_eq!(ev(&mut e, &mut h, "$xx"), "reloaded");
    run(&mut e, &mut h, "/unload -rs x.mrc");
    assert!(e.scripts().is_empty());
    assert!(run_err(&mut e, &mut h, "/load -rs ../x.mrc").contains("accès refusé"));
}

#[test]
fn load_vars_and_users_files() {
    let (mut e, mut h) = (engine(), FakeHost::new());
    std::fs::write(e.root().join("vars.ini"), "[variables]\nn0=%loaded yes\n").unwrap();
    std::fs::write(e.root().join("users.ini"), "[users]\nn0=7:*!*@x\n").unwrap();
    run(&mut e, &mut h, "/load -rv vars.ini | load -ru users.ini");
    assert_eq!(ev(&mut e, &mut h, "%loaded"), "yes");
    assert_eq!(e.users_text(), "7:*!*@x\n");
}

#[test]
fn replace_script_keeps_order() {
    let mut e = engine();
    e.load_text("a", ScriptKind::Remote, "alias a1 return 1").unwrap();
    e.load_text("b", ScriptKind::Remote, "alias b1 return 1").unwrap();
    e.load_text("a", ScriptKind::Remote, "alias a2 return 2").unwrap();
    let names: Vec<&str> = e.scripts().iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["a", "b"]);
    assert!(!e.has_alias("a1"));
    assert!(e.has_alias("a2"));
}

#[test]
fn parse_error_reports_line() {
    let mut e = engine();
    let r = e.load_text("p.mrc", ScriptKind::Remote, "alias ok echo ok\n\non *:TEXT:*:#:{\n  echo never closed\n");
    match r {
        Err(ScriptError::Parse { line, .. }) => assert_eq!(line, 3),
        other => panic!("{other:?}"),
    }
    assert!(e.scripts().is_empty());
}

#[test]
fn allman_braces() {
    let (mut e, mut h) = setup("alias t\n{\n  echo -a allman\n}\non *:TEXT:*:#:\n{\n  echo -a ev\n}");
    run(&mut e, &mut h, "/t");
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "x"));
    assert_eq!(h.says(), vec!["allman", "ev"]);
}

#[test]
fn big_script_smoke() {
    // Un petit bot complet : compteur de mots par pseudo, commandes !top et !seen.
    let script = r#"
on *:START:{ hmake words 100 }
on *:TEXT:*:#:{
  if (!$hget(words)) hmake words 100
  hinc words $nick $0
  hadd -m seen $nick $ctime $1-
  if ($1 == !count) msg # $nick has $hget(words,$nick) words
  elseif ($1 == !seen) && ($2) {
    var %s = $hget(seen,$2)
    if (%s) msg # $2 was last seen $duration($calc($ctime - $gettok(%s,1,32))) ago saying: $gettok(%s,2-,32)
    else msg # I have not seen $2
  }
}
"#;
    let (mut e, mut h) = setup(script);
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "hello there world"));
    h.now += 3_700_000;
    e.fire(&mut h, &ctx(), &text_ev("Carl", Some("#test"), "!count"));
    e.fire(&mut h, &ctx(), &text_ev("Carl", Some("#test"), "!seen bob"));
    e.fire(&mut h, &ctx(), &text_ev("Carl", Some("#test"), "!seen nobody"));
    assert_eq!(
        h.find("msg"),
        vec![
            "#test Carl has 1 words",
            "#test bob was last seen 1hr 1min 40secs ago saying: hello there world",
            "#test I have not seen nobody",
        ]
    );
    assert!(h.errors.is_empty(), "{:?}", h.errors);
}


#[test]
fn halted_identifier() {
    let mut e = engine();
    let mut h = FakeHost::new();
    e.load_text("a.mrc", ScriptKind::Remote, "on *:TEXT:*:#:{ if ($1 == stop) haltdef }").unwrap();
    e.load_text("b.mrc", ScriptKind::Remote, "on *:TEXT:*:#:{ echo -a halted $halted }").unwrap();
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "stop"));
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "go"));
    assert_eq!(h.says(), vec!["halted $true", "halted $false"]);
}

#[test]
fn many_aliases_lookup() {
    // Beaucoup d'alias dans plusieurs fichiers : l'index trouve le bon, dans l'ordre de chargement.
    let mut e = engine();
    let mut h = FakeHost::new();
    for f in 0..5 {
        let mut src = String::new();
        for i in 0..200 {
            src.push_str(&format!("alias a{i} return f{f}\n"));
        }
        e.load_text(&format!("f{f}.mrc"), ScriptKind::Remote, &src).unwrap();
    }
    assert_eq!(ev(&mut e, &mut h, "$a199 $a0"), "f0 f0");
    e.unload("f0.mrc");
    assert_eq!(ev(&mut e, &mut h, "$a199"), "f1");
    run(&mut e, &mut h, "//var %i = 0 | while (%i < 2000) { inc %i | noop $a150 }");
    assert!(h.errors.is_empty());
}

#[test]
fn multibyte_scripts_do_not_panic() {
    let src = "alias é { return accent }\nalias aé echo -a $1\nmenu channel {\n  ééééééééé:echo -a e\n  日本語の項目:echo -a jp\n  $submenu($sm($1))\n}\nalias sm if ($1 == 1) return ü:echo -a ü\n\
               on *:TEXT:*café*:#:{ echo -a $replace($1-,é,e) }\ndialog d { title \"Été\" | text \"Ünïcödé\", 1, 1 1 10 10 }";
    let (mut e, mut h) = setup(src);
    let m = e.popup(&mut h, &ctx(), PopupKind::Channel, None);
    assert_eq!(labels(&m), vec!["ééééééééé", "日本語の項目", "ü"]);
    e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), "un café noir"));
    assert_eq!(h.says(), vec!["un cafe noir"]);
    assert_eq!(ev(&mut e, &mut h, "$ctime(aaé 2020)"), "");
    assert_eq!(ev(&mut e, &mut h, "$left(日本語,-1) $mid(日本語,2,1) $pos(日本語,語)"), "日本 本 3");
}

/// Textes au hasard faits des caractères qui comptent en mSL : rien ne doit paniquer ni boucler.
#[test]
fn garbage_never_panics() {
    let pieces = [
        "$", "%", "#", "(", ")", "[", "]", " [ ", " ] ", ",", " | ", "{", "}", " { ", " } ", ":", ";", "\n", " ", "é", "日",
        "$+", " $+ ", "$1-", "$$1", "$!", "$?", "$*", "%x", "$calc(", "$iif(", "if (", "while (", "else", "goto a", ":a",
        "$gettok(", "46", "-", "\\", "\"", "/", "//", "!", "&&", "||", "==", "isin", "$regex(", "/(.)/g", "on *:TEXT:",
        "alias ", "menu ", "dialog ", "#g on", "#g end", "$&", "/*", "*/", "$eval(", ",0)", "$read(", "../", "set ", "var ",
        "inc ", "timer ", "1 ", "hadd -m t ", "did -a ", "dialog -m ", "$did(", "return ", "halt", "tokenize 32 ", "$submenu(",
    ];
    let mut seed: u64 = 0x2545F4914F6CDD1D;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut e = engine();
    e.set_limits(2_000, 30);
    let mut h = FakeHost::new();
    for round in 0..3000 {
        let n = (next() % 25) as usize + 1;
        let mut s = String::new();
        for _ in 0..n {
            s.push_str(pieces[(next() % pieces.len() as u64) as usize]);
        }
        let _ = e.evaluate(&mut h, &ctx(), &s);
        let _ = e.run_line(&mut h, &ctx(), &format!("//{s}"));
        if round % 3 == 0 {
            let _ = e.load_text("fuzz.mrc", ScriptKind::Remote, &s);
            let _ = e.load_text("fuzz.ini", ScriptKind::Popups, &s);
            let _ = e.fire(&mut h, &ctx(), &text_ev("Bob", Some("#test"), &s));
            let _ = e.popup(&mut h, &ctx(), PopupKind::Channel, None);
        }
        e.discard_pending();
        h.clear();
    }
    let t = h.now + 100_000;
    e.tick(&mut h, t);
}
