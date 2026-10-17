//! Le moteur mSL branché sur le client : événements (et /haltdef), alias, identifiants de l'hôte, popups par
//! défaut, timers, scripts du premier démarrage, enregistrement.

mod common;

use alpirc_core::{Effect, LineKind};
use alpirc_script::{EventKind, MenuItem, PopupKind, ScriptKind};
use common::*;

fn labels(m: &[MenuItem]) -> Vec<String> {
    m.iter().map(|i| i.label.clone()).collect()
}

fn find<'a>(m: &'a [MenuItem], l: &str) -> &'a MenuItem {
    m.iter().find(|i| i.label == l).unwrap_or_else(|| panic!("pas d'entrée {l}"))
}

fn setup(script: &str) -> (H, alpirc_core::WinId) {
    let mut h = H::new();
    h.c.engine_mut().load_text("t.mrc", ScriptKind::Remote, script).unwrap();
    h.up();
    let w = h.join("#lyon", "Guiwald @Pat Bob");
    h.feed(":Bob!bob@bob.host PRIVMSG #lyon :hello");
    h.clear();
    (h, w)
}

#[test]
fn on_text_replies() {
    let (mut h, w) = setup("on *:TEXT:!hello:#:{ msg # hi $nick }");
    h.feed(":Pat!p@h PRIVMSG #lyon :!hello");
    assert_eq!(h.sends(), vec!["PRIVMSG #lyon :hi Pat"]);
    let l = h.lines(w);
    assert_eq!(&l[l.len() - 2..], &["<Pat> !hello", "<Guiwald> hi Pat"]);
}

#[test]
fn haltdef_suppresses_default_display() {
    let (mut h, w) = setup("on ^*:TEXT:*secret*:#:{ echo -a [filtré] | haltdef }");
    h.feed(":Pat!p@h PRIVMSG #lyon :un secret ici");
    assert_eq!(h.last(w), "[filtré]");
    h.feed(":Pat!p@h PRIVMSG #lyon :rien");
    assert_eq!(h.last(w), "<Pat> rien");
}

#[test]
fn own_messages_do_not_fire_text() {
    let (mut h, w) = setup("on *:TEXT:*:#:{ inc %count }");
    h.input(w, "moi");
    h.feed(":Pat!p@h PRIVMSG #lyon :lui");
    h.input(w, "//echo -a count= $+ %count");
    // `hello` de Bob (dans setup) et `lui` de Pat : 2 ; notre `moi` ne compte pas.
    assert_eq!(h.last(w), "count=2");
}

#[test]
fn join_events() {
    let (mut h, w) = setup("on !*:JOIN:#lyon:{ notice $nick Bienvenue $nick }\non me:*:JOIN:#:{ echo -s je suis sur $chan }");
    h.feed(":Carl!c@h JOIN #lyon");
    assert_eq!(h.sends(), vec!["NOTICE Carl :Bienvenue Carl"]);
    h.feed(":Guiwald!alpirc@host.test JOIN #autre");
    assert_eq!(h.last(h.status), "je suis sur #autre");
    assert!(h.sends().iter().all(|l| !l.starts_with("NOTICE")));
    let _ = w;
}

#[test]
fn input_event_can_block() {
    let (mut h, w) = setup("on *:INPUT:#:{ if ($1 == secret) { echo -a bloqué | haltdef } }");
    h.input(w, "secret défense");
    assert!(h.sends().is_empty());
    assert_eq!(h.last(w), "bloqué");
    h.input(w, "public");
    assert_eq!(h.sends(), vec!["PRIVMSG #lyon :public"]);
}

#[test]
fn raw_event_and_haltdef() {
    let (mut h, w) = setup("raw 401:*:{ echo -s pas de $2 | haltdef }\nraw 999:*:{ echo -s raw $numeric : $3- }");
    h.feed(":irc.test.net 401 Guiwald Zed :No such nick/channel");
    assert_eq!(h.last(h.status), "pas de Zed");
    assert!(!h.lines(w).iter().any(|l| l.contains("No such nick")));
    h.feed(":irc.test.net 999 Guiwald a :b c");
    let l = h.status_lines();
    assert_eq!(&l[l.len() - 2..], &["raw 999 : b c", "a b c"]);
}

#[test]
fn ctcp_event_replaces_default_reply() {
    let (mut h, w) = setup("ctcp *:VERSION:*:{ ctcpreply $nick VERSION mon script | halt }");
    h.feed(":Pat!p@h PRIVMSG Guiwald :\x01VERSION\x01");
    assert_eq!(h.sends(), vec!["NOTICE Pat :\x01VERSION mon script\x01"]);
    assert!(!h.lines(w).iter().any(|l| l.contains("[Pat VERSION]")));
    h.feed(":Pat!p@h PRIVMSG Guiwald :\x01TIME\x01");
    assert_eq!(h.sends().len(), 1);
}

#[test]
fn ctcpreply_event() {
    let (mut h, w) = setup("on *:CTCPREPLY:VERSION*:{ echo -a version de $nick : $2- | haltdef }");
    h.feed(":Pat!p@h NOTICE Guiwald :\x01VERSION irssi 1.4\x01");
    assert_eq!(h.last(w), "version de Pat : irssi 1.4");
}

#[test]
fn alias_takes_precedence_and_bang_bypasses() {
    let (mut h, w) = setup("alias join { echo -a pas de join pour $1 }");
    h.input(w, "/join #x");
    assert!(h.sends().is_empty());
    assert_eq!(h.last(w), "pas de join pour #x");
    h.input(w, "/!join #x");
    assert_eq!(h.sends(), vec!["JOIN #x"]);
}

#[test]
fn default_aliases_loaded() {
    let (mut h, w) = setup("");
    h.input(w, "/w Bob");
    h.input(w, "/j #x");
    h.input(w, "/p");
    h.input(w, "/n");
    h.input(w, "/k Bob bye");
    h.input(w, "/ping Bob");
    assert_eq!(h.sends()[..5], ["WHOIS Bob", "JOIN #x", "PART #lyon", "NAMES #lyon", "KICK #lyon Bob bye"]);
    assert!(h.sends().is_empty());
    h.input(w, "/q Bob");
    assert!(h.has_win("Bob"));
}

#[test]
fn ping_alias_is_ctcp_ping() {
    let (mut h, w) = setup("");
    h.input(w, "/ping Bob");
    assert_eq!(h.sends(), vec![format!("PRIVMSG Bob :\x01PING {}\x01", h.now)]);
}

#[test]
fn host_identifiers() {
    let (mut h, w) = setup("");
    h.input(w, "//echo -a $me $network $server $status $chan(0) $chan(1) $nick(#lyon,0) $cid $active $scon(0)");
    assert_eq!(h.last(w), format!("Guiwald TestNet irc.test.net connected 1 #lyon 3 {} #lyon 1", h.conn));
    h.input(w, "//echo -a $nick(#lyon,1) $nick(#lyon,1).pnick $nick(#lyon,Bob) $opnick(#lyon,0) $nick(#lyon,0,o)");
    assert_eq!(h.last(w), "Pat @Pat 2 1 1");
    h.input(w, "//echo -a $address(Bob,2) $address(Bob,5) $comchan(Bob,0) $comchan(Bob,1)");
    assert_eq!(h.last(w), "*!*@bob.host Bob!bob@bob.host 1 #lyon");
    h.input(w, "//echo -a $ial(*!*@bob.host,1).nick $ial(*,0)");
    assert!(h.last(w).starts_with("Bob "), "{}", h.last(w));
    h.input(w, "//echo -a $version $os $mnick $anick $fullname $usermode $away");
    assert_eq!(h.last(w), format!("AlpIRC {} {} Guiwald Guiwald_ AlpIRC user  $false", alpirc_core::client::VERSION, if cfg!(target_os = "macos") { "macOS" } else { "Linux" }));
    h.input(w, "//echo -a $chan(#lyon).topic| $chan(#lyon).mode $query(0) $notify(0) $ignore(0)");
    assert_eq!(h.last(w), "|  0 0 0");
}

#[test]
fn member_conditions() {
    let (mut h, w) = setup("");
    h.input(w, "//if (Pat isop #lyon) echo -a op | if (Bob isreg #lyon) echo -a reg | if ($me ison #lyon) echo -a on | if (Zed ison #lyon) echo -a zed");
    let l = h.lines(w);
    assert_eq!(&l[l.len() - 3..], &["op", "reg", "on"]);
    h.input(w, "//if (#lyon ischan) echo -a chan");
    assert_eq!(h.last(w), "chan");
}

#[test]
fn isban_isnotify_isignore() {
    let (mut h, w) = setup("");
    h.feed(":Pat!p@h MODE #lyon +b *!*@bad.host");
    h.input(w, "/notify Carl");
    h.input(w, "/ignore Troll");
    h.input(w, "//if (*!*@bad.host isban #lyon) echo -a banned | if (Carl isnotify) echo -a notified | if (Troll isignore) echo -a ignored");
    let l = h.lines(w);
    assert_eq!(&l[l.len() - 3..], &["banned", "notified", "ignored"]);
}

#[test]
fn default_popups() {
    let (mut h, w) = setup("");
    let m = h.c.popup(PopupKind::Nicklist, w, &["Bob".to_string()]);
    assert_eq!(labels(&m), vec!["Info", "Whois", "Query", "-", "Control", "CTCP", "DCC", "-", "Slap!", "-", "Ignore"]);
    assert_eq!(
        labels(&find(&m, "Control").children),
        vec!["Op", "Deop", "Voice", "Devoice", "-", "Kick", "Kick (why)", "Ban", "Ban, Kick", "Ban, Kick (why)"]
    );
    assert_eq!(labels(&find(&m, "CTCP").children), vec!["Ping", "Time", "Version"]);
    let sel = vec!["Bob".to_string()];
    h.c.popup_run(w, &sel, find(&m, "Slap!").action.unwrap(), h.now);
    assert_eq!(h.sends(), vec!["PRIVMSG #lyon :\x01ACTION slaps Bob around a bit with a large trout\x01"]);
    assert_eq!(h.last(w), "* Guiwald slaps Bob around a bit with a large trout");
    let m = h.c.popup(PopupKind::Nicklist, w, &sel);
    h.c.popup_run(w, &sel, find(&m, "Control").children[0].action.unwrap(), h.now);
    assert_eq!(h.sends(), vec!["MODE #lyon +o Bob"]);
    let m = h.c.popup(PopupKind::Nicklist, w, &sel);
    h.c.popup_run(w, &sel, find(&m, "Control").children[8].action.unwrap(), h.now);
    assert_eq!(h.sends(), vec!["MODE #lyon +b *!*@bob.host", "KICK #lyon Bob"]);
    // Plusieurs pseudos : /op $$1- les prend tous.
    let two = vec!["Bob".to_string(), "Pat".to_string()];
    let m = h.c.popup(PopupKind::Nicklist, w, &two);
    h.c.popup_run(w, &two, find(&m, "Control").children[1].action.unwrap(), h.now);
    assert_eq!(h.sends(), vec!["MODE #lyon -oo Bob Pat"]);
}

#[test]
fn status_channel_query_popups() {
    let (mut h, w) = setup("");
    assert_eq!(labels(&h.c.popup(PopupKind::Status, h.status, &[])), vec!["Lusers", "Motd", "Time", "-", "Join channel", "-", "Quit"]);
    let m = h.c.popup(PopupKind::Channel, w, &[]);
    assert_eq!(labels(&m), vec!["Channel Modes", "Channels Folder", "-", "Part"]);
    h.c.popup_run(w, &[], find(&m, "Part").action.unwrap(), h.now);
    assert_eq!(h.sends(), vec!["PART #lyon"]);
    h.feed(":Bob!bob@bob.host PRIVMSG Guiwald :yo");
    let q = h.win("Bob");
    let m = h.c.popup(PopupKind::Query, q, &[]);
    assert_eq!(labels(&m), vec!["Info", "Whois", "Query", "-", "DCC", "-", "Ignore", "Close"]);
    h.c.popup_run(q, &[], find(&m, "Whois").action.unwrap(), h.now);
    assert_eq!(h.sends(), vec!["WHOIS Bob"]);
    let m = h.c.popup(PopupKind::Query, q, &[]);
    h.c.popup_run(q, &[], find(&m, "Close").action.unwrap(), h.now);
    assert!(!h.has_win("Bob"));
}

#[test]
fn on_start_fires_at_startup() {
    let dir = tmpdir();
    std::fs::create_dir_all(dir.join("scripts")).unwrap();
    std::fs::write(dir.join("scripts/start.mrc"), "on *:START:{ set %started oui }\n").unwrap();
    let mut h = H::with_dir(settings(), dir.clone());
    h.input(h.status, "//echo -a %started");
    assert_eq!(h.last(h.status), "oui");
    // Les scripts par défaut ont été écrits à côté.
    assert!(dir.join("scripts/aliases.mrc").is_file());
    assert!(dir.join("scripts/popups.mrc").is_file());
}

#[test]
fn defaults_not_overwritten_and_order_kept() {
    let dir = tmpdir();
    {
        let mut h = H::with_dir(settings(), dir.clone());
        h.c.load_script_text("mine.mrc", ScriptKind::Remote, "alias hello { echo -a hello $1 }", h.now).unwrap();
        assert!(h.effects().contains(&Effect::SaveScripts));
        h.input(h.status, "/set %keep 42");
        h.c.save_scripts().unwrap();
    }
    std::fs::write(dir.join("scripts/aliases.mrc"), "alias j echo -a custom j\n").unwrap();
    let mut h = H::with_dir(settings(), dir.clone());
    h.input(h.status, "/j x");
    assert_eq!(h.last(h.status), "custom j");
    h.input(h.status, "/hello monde");
    assert_eq!(h.last(h.status), "hello monde");
    h.input(h.status, "//echo -a %keep");
    assert_eq!(h.last(h.status), "42");
    let order = std::fs::read_to_string(dir.join("scripts/order.txt")).unwrap();
    assert_eq!(order, "aliases.mrc\npopups.mrc\nmine.mrc\n");
}

#[test]
fn script_errors_in_red() {
    let (mut h, w) = setup("alias bad { msg }");
    h.input(w, "/bad");
    let l = h.c.window(w).unwrap().lines.back().unwrap().clone();
    assert_eq!(l.kind, LineKind::Error);
    assert!(l.text.starts_with("* /msg: insufficient parameters"), "{}", l.text);
}

#[test]
fn timers_run_on_tick() {
    let (mut h, w) = setup("");
    h.input(w, "/timer 1 5 msg #lyon tick");
    let next = h.advance(1000).unwrap();
    assert!(next <= h.now + 4000, "prochaine échéance : le timer");
    assert!(h.sends().is_empty());
    h.advance(4000);
    assert_eq!(h.sends(), vec!["PRIVMSG #lyon :tick"]);
}

#[test]
fn connect_and_disconnect_events() {
    let mut h = H::new();
    h.c.engine_mut()
        .load_text("t.mrc", ScriptKind::Remote, "on *:CONNECT:{ join #auto }\non *:DISCONNECT:{ echo -s perdu $network }\non *:LOGON:*:{ echo -s logon }")
        .unwrap();
    h.connect();
    assert_eq!(h.last(h.status), "logon");
    h.register();
    assert!(h.sends().contains(&"JOIN #auto".to_string()));
    h.c.on_closed(h.conn, Some("reset"), h.now);
    assert!(h.has_line(h.status, "perdu TestNet"));
}

#[test]
fn kick_nick_quit_op_ban_events() {
    let (mut h, w) = setup(
        "on *:KICK:#:{ echo -a kick $knick par $nick ( $+ $1- $+ ) }\non *:NICK:{ echo -s nick $nick -> $newnick }\non *:QUIT:{ echo -s quit $nick $1- }\non *:OP:#:{ echo -a op $opnick par $nick }\non *:BAN:#:{ echo -a ban $banmask $bnick }\non *:RAWMODE:#:{ echo -a raw $1- }\non *:TOPIC:#:{ echo -a topic $1- }\non *:PART:#:{ echo -a part $nick $1- }",
    );
    h.feed(":Pat!p@h KICK #lyon Bob :dehors");
    assert!(h.has_line(w, "kick Bob par Pat (dehors)"));
    h.feed(":Pat!p@h NICK Patrick");
    assert!(h.has_line(h.status, "nick Pat -> Patrick"));
    h.feed(":Patrick!p@h MODE #lyon +o Guiwald");
    assert!(h.has_line(w, "op Guiwald par Patrick"));
    assert!(h.has_line(w, "raw +o Guiwald"));
    h.feed(":Carl!c@carl.host JOIN #lyon");
    h.feed(":Patrick!p@h MODE #lyon +b *!*@carl.host");
    assert!(h.has_line(w, "ban *!*@carl.host Carl"));
    h.feed(":Patrick!p@h TOPIC #lyon :nouveau");
    assert!(h.has_line(w, "topic nouveau"));
    h.feed(":Carl!c@carl.host PART #lyon :salut");
    assert!(h.has_line(w, "part Carl salut"));
    h.feed(":Patrick!p@h QUIT :bye bye");
    assert!(h.has_line(h.status, "quit Patrick bye bye"));
}

#[test]
fn notice_action_open_close_active_events() {
    let (mut h, w) = setup(
        "on *:NOTICE:*:?:{ echo -s notice $nick $1- }\non *:ACTION:*:#:{ echo -a action $nick $1- }\non *:OPEN:?:*:{ echo -s open $nick $1- }\non *:CLOSE:?:{ echo -s close $target }\non *:ACTIVE:*:{ echo -s active $target }",
    );
    h.feed(":Pat!p@h NOTICE Guiwald :coucou");
    assert!(h.has_line(h.status, "notice Pat coucou"));
    h.feed(":Pat!p@h PRIVMSG #lyon :\x01ACTION rit\x01");
    assert!(h.has_line(w, "action Pat rit"));
    h.feed(":Zoe!z@h PRIVMSG Guiwald :salut toi");
    assert!(h.has_line(h.status, "open Zoe salut toi"));
    let q = h.win("Zoe");
    h.c.set_active(q, h.now);
    assert!(h.has_line(h.status, "active Zoe"));
    h.c.close_window(q, h.now);
    assert!(h.has_line(h.status, "close Zoe"));
}

#[test]
fn history_does_not_fire_scripts() {
    let mut h = H::new();
    h.c.engine_mut().load_text("t.mrc", ScriptKind::Remote, "on *:TEXT:*:#:{ inc %n }").unwrap();
    h.up_with_caps("batch server-time draft/chathistory");
    let w = h.join("#lyon", "Guiwald");
    h.feed(":irc.test.net BATCH +x chathistory #lyon");
    h.feed("@batch=x;time=2025-10-09T08:00:00.000Z :Bob!b@h PRIVMSG #lyon :vieux");
    h.feed(":irc.test.net BATCH -x");
    h.input(w, "//echo -a n= $+ %n");
    assert_eq!(h.last(w), "n=");
}

#[test]
fn perform_uses_aliases_and_identifiers() {
    let mut s = settings();
    s.network_mut("TestNet").unwrap().perform = vec!["j #perf".into(), "notice Bob je suis $me sur $network".into()];
    let mut h = H::with(s);
    h.connect();
    h.register();
    let s = h.sends();
    assert!(s.contains(&"JOIN #perf".to_string()), "{s:?}");
    assert!(s.contains(&"NOTICE Bob :je suis Guiwald sur TestNet".to_string()), "{s:?}");
}

#[test]
fn scon_runs_on_every_connection() {
    let mut h = H::new();
    h.up();
    let s2 = h.c.new_status(h.now);
    // `//` évalue la ligne une fois avant /scon : `$!cid` laisse `$cid` pour chaque connexion.
    h.input(h.status, "//scon -a echo -s cid $!cid");
    assert!(h.has_line(h.status, &format!("cid {}", h.conn)), "{:?}", h.status_lines());
    let c2 = h.c.window(s2).unwrap().conn.unwrap();
    assert!(h.lines(s2).contains(&format!("cid {c2}")));
}

#[test]
fn script_dialogs_signal_ui() {
    let (mut h, w) = setup("dialog demo {\n  title \"Démo\"\n  size -1 -1 100 50\n  button \"OK\", 1, 10 10 30 12, ok\n}");
    h.input(w, "/dialog -m demo demo");
    assert!(h.effects().contains(&Effect::ScriptDialogsChanged));
    assert_eq!(h.c.engine().dialogs().len(), 1);
    h.c.dialog_event("demo", alpirc_script::DialogEvent::Click(1), h.now);
    assert!(h.effects().contains(&Effect::ScriptDialogsChanged));
    assert!(h.c.engine().dialogs().is_empty());
}

#[test]
fn scripts_disabled_still_runs_builtins() {
    let mut s = settings();
    s.scripts.enabled = false;
    let mut h = H::with(s);
    h.up();
    let w = h.join("#lyon", "Guiwald");
    h.input(w, "/join #x");
    h.input(w, "/msg Bob hi");
    assert_eq!(h.sends(), vec!["JOIN #x", "PRIVMSG Bob :hi"]);
}

#[test]
fn event_kind_fired_for_usermode_and_snotice() {
    let mut h = H::new();
    h.c.engine_mut()
        .load_text("t.mrc", ScriptKind::Remote, "on *:USERMODE:{ echo -s umode $1- }\non *:SNOTICE:*:{ echo -s snotice $1- }")
        .unwrap();
    h.up();
    h.feed(":Guiwald MODE Guiwald :+i");
    assert!(h.has_line(h.status, "umode +i"));
    h.feed(":irc.test.net NOTICE Guiwald :*** Notice -- hello");
    assert!(h.has_line(h.status, "snotice *** Notice -- hello"));
    let _ = EventKind::Snotice;
}


#[test]
fn exit_event_and_save_scripts_effect() {
    let (mut h, w) = setup("on *:EXIT:{ set %bye 1 }");
    h.input(w, "/set %x 1");
    assert!(h.effects().contains(&Effect::SaveScripts));
    h.input(w, "/echo -a rien");
    assert!(!h.effects().contains(&Effect::SaveScripts), "rien n'a changé");
    h.input(w, "/exit");
    let fx = h.effects();
    assert!(fx.contains(&Effect::Exit));
    assert!(fx.contains(&Effect::SaveScripts), "on EXIT a posé %bye");
}

#[test]
fn load_warning_flow() {
    let mut h = H::new();
    std::fs::write(h.dir.join("scripts/auto.mrc"), "on *:LOAD:{ set %loaded 1 }").unwrap();
    let name = h.c.engine_mut().load_file(std::path::Path::new("auto.mrc"), false).unwrap();
    assert!(h.c.engine().runs_at_startup(&name));
    h.c.run_pending_scripts(h.now);
    h.input(h.status, "//echo -a %loaded");
    assert_eq!(h.last(h.status), "1");
}
