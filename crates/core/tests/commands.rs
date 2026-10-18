//! Commandes intégrées : /join, /part, /msg, /me, /notice, /query, /server, /op, /kick, /ban, /topic, /away,
//! /quit, /nick, /quote, /echo, /window, /list, /lag, commandes inconnues, WHOIS, complétion Tab.

mod common;

use alpirc_core::{Effect, LineKind, UiDialog, WinKind};
use common::*;

fn chan() -> (H, alpirc_core::WinId) {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "Guiwald @Pat +Vic Bob");
    h.feed(":Bob!bob@bob.host PRIVMSG #lyon :hi all");
    h.clear();
    (h, w)
}

#[test]
fn join_variants() {
    let (mut h, w) = chan();
    h.input(w, "/join #a key");
    h.input(w, "/join lyon2");
    h.input(w, "/join #b,#c");
    h.input(w, "/j #d");
    h.input(w, "/!join #e");
    assert_eq!(h.sends(), vec!["JOIN #a key", "JOIN #lyon2", "JOIN #b,#c", "JOIN #d", "JOIN #e"]);
    // Déjà sur le canal : la fenêtre est montrée.
    h.input(h.status, "/join #lyon");
    assert!(h.sends().is_empty());
    assert!(h.effects().contains(&Effect::Activate(w)));
}

#[test]
fn part_and_hop() {
    let (mut h, w) = chan();
    h.input(w, "/part");
    h.input(w, "/part #lyon bye now");
    h.input(w, "/hop");
    assert_eq!(h.sends(), vec!["PART #lyon", "PART #lyon :bye now", "PART #lyon", "JOIN #lyon"]);
}

#[test]
fn msg_to_nick_and_channel() {
    let (mut h, w) = chan();
    h.input(w, "/msg Bob salut toi");
    assert_eq!(h.sends(), vec!["PRIVMSG Bob :salut toi"]);
    assert_eq!(h.last(w), "-> *Bob* salut toi");
    h.input(h.status, "/msg #lyon depuis Status");
    assert_eq!(h.last(w), "<Guiwald> depuis Status");
    h.input(w, "/msg Bob,Vic hey");
    assert_eq!(h.sends(), vec!["PRIVMSG #lyon :depuis Status", "PRIVMSG Bob :hey", "PRIVMSG Vic :hey"]);
    h.input(w, "/msg Bob");
    assert_eq!(h.last(w), "* /msg: insufficient parameters");
    assert_eq!(h.c.window(w).unwrap().lines.back().unwrap().kind, LineKind::Error);
}

#[test]
fn quiet_msg_has_no_echo() {
    let (mut h, w) = chan();
    h.input(w, "/.msg Bob discret");
    assert_eq!(h.sends(), vec!["PRIVMSG Bob :discret"]);
    assert_eq!(h.last(w), "<Bob> hi all");
}

#[test]
fn me_describe_notice_say() {
    let (mut h, w) = chan();
    h.input(w, "/me danse la java");
    assert_eq!(h.last(w), "* Guiwald danse la java");
    h.input(w, "/describe Bob fait coucou");
    assert_eq!(h.last(w), "-> *Bob* * Guiwald fait coucou");
    h.input(w, "/notice Bob psst");
    assert_eq!(h.last(w), "-> -Bob- psst");
    h.input(w, "/say /pas une commande");
    assert_eq!(h.last(w), "<Guiwald> /pas une commande");
    assert_eq!(
        h.sends(),
        vec![
            "PRIVMSG #lyon :\x01ACTION danse la java\x01",
            "PRIVMSG Bob :\x01ACTION fait coucou\x01",
            "NOTICE Bob :psst",
            "PRIVMSG #lyon :/pas une commande",
        ]
    );
    h.input(h.status, "/me test");
    assert_eq!(h.last(h.status), "* You're not on a channel");
}

#[test]
fn query_opens_and_sends() {
    let (mut h, w) = chan();
    h.input(w, "/query Bob ça va ?");
    let q = h.win("Bob");
    assert_eq!(h.kind_of("Bob"), WinKind::Query);
    assert!(h.effects().contains(&Effect::Activate(q)));
    assert_eq!(h.sends(), vec!["PRIVMSG Bob :ça va ?"]);
    assert_eq!(h.last(q), "<Guiwald> ça va ?");
    assert_eq!(h.c.window(q).unwrap().title, "Bob (bob@bob.host)");
    h.input(q, "réponds");
    assert_eq!(h.sends(), vec!["PRIVMSG Bob :réponds"]);
}

#[test]
fn server_command_parsing() {
    let mut h = H::new();
    h.input(h.status, "/server irc.libera.chat +6697");
    assert_eq!(h.connects(), vec![(h.conn, "irc.libera.chat".into(), 6697, true)]);
    assert_eq!(h.c.conn_info(h.conn).unwrap().network, "Libera.Chat");
    h.input(h.status, "/server irc.example.org:7000");
    assert_eq!(h.connects(), vec![(h.conn, "irc.example.org".into(), 7000, false)]);
    h.input(h.status, "/server Undernet");
    assert_eq!(h.connects(), vec![(h.conn, "irc.undernet.org".into(), 6667, false)]);
    h.input(h.status, "/server -m irc.other.org 6668");
    let c = h.connects();
    assert_eq!(c.len(), 1);
    assert_ne!(c[0].0, h.conn);
    assert_eq!((c[0].1.as_str(), c[0].2), ("irc.other.org", 6668));
    assert_eq!(h.c.windows().iter().filter(|w| w.kind == WinKind::Status).count(), 2);
}

#[test]
fn server_with_password_and_undernet_loc_syntax() {
    let mut h = H::new();
    h.input(h.status, "/server irc.undernet.org 6667 +x! guiwald s3cret");
    h.c.on_connected(h.conn, h.now);
    assert_eq!(h.sends()[0], "PASS :+x! guiwald s3cret");
}

#[test]
fn server_switch_quits_current() {
    let mut h = H::new();
    h.up();
    h.input(h.status, "/server irc.other.org");
    let s = h.sends();
    assert_eq!(s, vec!["QUIT :AlpIRC — https://github.com/Guiwald/alpirc"]);
    let fx = h.effects();
    assert!(fx.contains(&Effect::Close { conn: h.conn }));
    assert!(fx.iter().any(|e| matches!(e, Effect::Connect { host, .. } if host == "irc.other.org")));
}

#[test]
fn op_batched_by_modes() {
    let (mut h, w) = chan();
    h.input(w, "/op a b c d");
    h.input(w, "/deop Pat");
    h.input(w, "/voice x y");
    h.input(w, "/devoice #other z");
    assert_eq!(h.sends(), vec!["MODE #lyon +ooo a b c", "MODE #lyon +o d", "MODE #lyon -o Pat", "MODE #lyon +vv x y", "MODE #other -v z"]);
}

#[test]
fn kick_and_invite_and_topic() {
    let (mut h, w) = chan();
    h.input(w, "/kick Bob dehors !");
    h.input(w, "/kick #other Bob");
    h.input(w, "/invite Carl");
    h.input(w, "/topic Nouveau sujet");
    h.input(w, "/topic");
    h.input(w, "/topic #other");
    assert_eq!(
        h.sends(),
        vec!["KICK #lyon Bob :dehors !", "KICK #other Bob", "INVITE Carl #lyon", "TOPIC #lyon :Nouveau sujet", "TOPIC #lyon", "TOPIC #other"]
    );
}

#[test]
fn ban_with_known_address() {
    let (mut h, w) = chan();
    h.input(w, "/ban Bob 2");
    h.input(w, "/ban Bob");
    h.input(w, "/ban -k Bob 2 va-t'en");
    h.input(w, "/ban -r *!*@old.host");
    assert_eq!(
        h.sends(),
        vec![
            "MODE #lyon +b *!*@bob.host",
            "MODE #lyon +b Bob!*@*",
            "MODE #lyon +b *!*@bob.host",
            "KICK #lyon Bob va-t'en",
            "MODE #lyon -b *!*@old.host",
        ]
    );
}

#[test]
fn ban_looks_up_unknown_address() {
    let (mut h, w) = chan();
    h.input(w, "/ban -k Zed 3 bye");
    assert_eq!(h.sends(), vec!["USERHOST Zed"]);
    h.feed(":irc.test.net 302 Guiwald :Zed=+~zed@a.b.zed.org");
    assert_eq!(h.sends(), vec!["MODE #lyon +b *!*zed@*.b.zed.org", "KICK #lyon Zed bye"]);
    assert!(!h.lines(w).iter().any(|l| l.contains("Zed=")), "réponse USERHOST non affichée");
}

#[test]
fn timed_ban() {
    let (mut h, w) = chan();
    h.input(w, "/ban -u10 Bob 2");
    assert_eq!(h.sends(), vec!["MODE #lyon +b *!*@bob.host"]);
    h.advance(9_000);
    assert!(h.sends().is_empty());
    h.advance(1_000);
    assert_eq!(h.sends(), vec!["MODE #lyon -b *!*@bob.host"]);
}

#[test]
fn away_and_back() {
    let (mut h, w) = chan();
    h.input(w, "/away Parti manger");
    assert_eq!(h.sends(), vec!["AWAY :Parti manger"]);
    h.feed(":irc.test.net 306 Guiwald :You have been marked as being away");
    assert_eq!(h.c.conn_info(h.conn).unwrap().away.as_deref(), Some("Parti manger"));
    assert_eq!(h.last(w), "You have been marked as being away");
    h.input(w, "/back");
    assert_eq!(h.sends(), vec!["AWAY"]);
    h.feed(":irc.test.net 305 Guiwald :You are no longer marked as being away");
    assert_eq!(h.c.conn_info(h.conn).unwrap().away, None);
}

#[test]
fn quit_does_not_reconnect() {
    let (mut h, w) = chan();
    h.input(w, "/quit À plus");
    assert_eq!(h.sends(), vec!["QUIT :À plus"]);
    assert!(h.effects().contains(&Effect::Close { conn: h.conn }));
    assert_eq!(h.c.conn_info(h.conn).unwrap().state, alpirc_core::ConnState::Disconnected);
    assert_eq!(h.last(h.status), "* Disconnected");
    h.c.on_closed(h.conn, None, h.now);
    h.advance(120_000);
    assert!(h.connects().is_empty());
}

#[test]
fn nick_offline_and_online() {
    let mut h = H::new();
    h.input(h.status, "/nick Toto");
    assert_eq!(h.last(h.status), "* Your nick is now Toto");
    assert_eq!(h.c.window(h.status).unwrap().title, "Status: Toto (not connected)");
    h.connect();
    assert!(h.sends().contains(&"NICK Toto".to_string()));
    h.feed(":irc.test.net 001 Toto :Welcome");
    h.input(h.status, "/nick Titi");
    assert_eq!(h.sends(), vec!["NICK Titi"]);
}

#[test]
fn quote_raw_and_unknown_commands() {
    let (mut h, w) = chan();
    h.input(w, "/quote PRIVMSG Bob :raw line");
    h.input(w, "/raw -q MODE Guiwald +i");
    h.input(w, "/knock #secret let me in");
    h.input(w, "/lusers");
    h.input(w, "/whois Bob");
    h.input(w, "/names");
    h.input(w, "/ctcp Bob version");
    h.input(w, "/ctcpreply Bob PING 123");
    h.input(w, "/setname Robert Dupont");
    assert_eq!(
        h.sends(),
        vec![
            "PRIVMSG Bob :raw line",
            "MODE Guiwald +i",
            "KNOCK #secret let me in",
            "LUSERS",
            "WHOIS Bob",
            "NAMES #lyon",
            "PRIVMSG Bob :\x01VERSION\x01",
            "NOTICE Bob :\x01PING 123\x01",
            "SETNAME :Robert Dupont",
        ]
    );
    assert!(h.has_line(h.status, "-> irc.test.net PRIVMSG Bob :raw line"));
    assert!(!h.status_lines().iter().any(|l| l.contains("MODE Guiwald")));
}

#[test]
fn commands_need_connection() {
    let mut h = H::new();
    h.input(h.status, "/msg Bob hi");
    assert_eq!(h.last(h.status), "* Not connected to server");
    h.input(h.status, "/foo bar");
    assert_eq!(h.last(h.status), "* /foo: unknown command");
    h.input(h.status, "hello");
    assert_eq!(h.last(h.status), "* You're not on a channel");
    assert!(h.sends().is_empty());
}

#[test]
fn echo_switches() {
    let (mut h, w) = chan();
    h.input(h.status, "/echo -a dans l'active");
    assert_eq!(h.last(w), "dans l'active");
    h.input(w, "/echo -s dans status");
    assert_eq!(h.last(h.status), "dans status");
    h.input(h.status, "/echo #lyon vers le canal");
    assert_eq!(h.last(w), "vers le canal");
    h.input(w, "/echo 4 -a en rouge");
    let l = h.c.window(w).unwrap().lines.back().unwrap().clone();
    assert_eq!((l.text.as_str(), l.color), ("en rouge", Some(4)));
    h.input(w, "/echo -c info -a info");
    assert_eq!(h.c.window(w).unwrap().lines.back().unwrap().kind, LineKind::Info);
    h.input(w, "//echo -a $calc(6*7) $me");
    assert_eq!(h.last(w), "42 Guiwald");
    h.input(w, "/echo -a $calc(1+1)");
    assert_eq!(h.last(w), "$calc(1+1)", "un seul / : rien n'est évalué");
}

#[test]
fn echo_timestamp_switch() {
    let mut s = settings();
    s.irc.timestamps = true;
    let mut h = H::with(s);
    h.input(h.status, "/echo -s sans heure");
    assert_eq!(h.last(h.status), "sans heure");
    h.input(h.status, "/echo -st avec heure");
    assert_eq!(h.last(h.status), "[08:53] avec heure");
}

#[test]
fn refused_and_unavailable_commands() {
    let (mut h, w) = chan();
    h.input(w, "/run calc.exe");
    assert!(h.last(w).starts_with("* /run"), "{}", h.last(w));
    h.input(w, "/dcc send Bob x.txt");
    assert_eq!(h.last(w), "* /dcc send: no such file: x.txt");
    assert!(h.sends().is_empty());
}

#[test]
fn custom_windows_and_lines() {
    let (mut h, w) = chan();
    h.input(w, "/window -l @test");
    let t = h.c.find_window(None, "@test").unwrap();
    assert_eq!(h.c.window(t).unwrap().kind, WinKind::Custom);
    assert!(h.c.window(t).unwrap().listbox);
    assert!(h.effects().contains(&Effect::Activate(t)));
    h.input(w, "/aline @test un");
    h.input(w, "/aline 4 @test deux");
    h.input(w, "/iline @test 1 zéro");
    h.input(w, "/rline @test 3 trois");
    h.input(w, "/aline -l @test item1");
    h.input(w, "/aline -l @test item2");
    h.input(w, "/sline @test 2");
    assert_eq!(h.lines(t), vec!["zéro", "un", "trois"]);
    h.input(w, "//echo #lyon $line(@test,0) $line(@test,2) $line(@test,1,1) $sline(@test,1) $sline(@test,1).ln");
    assert_eq!(h.last(w), "3 un item1 item2 2");
    h.input(w, "/dline @test 1-2");
    assert_eq!(h.lines(t), vec!["trois"]);
    h.input(w, "//echo #lyon $window(@test) $window(@test).type $window(0)");
    assert_eq!(h.last(w), "@test custom 1");
    h.input(w, "/window -c @test");
    assert!(h.c.find_window(None, "@test").is_none());
    h.input(w, "/aline @nope x");
    assert_eq!(h.last(w), "* /aline: no such window '@nope'");
}

#[test]
fn clear_and_close() {
    let (mut h, w) = chan();
    h.feed(":Bob!bob@bob.host PRIVMSG Guiwald :psst");
    assert!(h.has_win("Bob"));
    h.input(w, "/clear");
    assert!(h.lines(w).is_empty());
    h.input(w, "/close -m Bob");
    assert!(!h.has_win("Bob"));
    h.input(w, "/clearall");
    assert!(h.status_lines().is_empty());
}

#[test]
fn list_window_collects_channels() {
    let (mut h, w) = chan();
    h.input(w, "/list -min 5");
    assert_eq!(h.sends(), vec!["LIST"]);
    let lw = h.c.find_window(Some(h.conn), "Channels List").unwrap();
    assert_eq!(h.c.window(lw).unwrap().kind, WinKind::ChanList);
    assert!(h.effects().contains(&Effect::Activate(lw)));
    h.feed(":irc.test.net 321 Guiwald Channel :Users  Name");
    h.feed(":irc.test.net 322 Guiwald #lyon 42 :[+nt] Bienvenue");
    h.feed(":irc.test.net 322 Guiwald #petit 2 :tout petit");
    h.feed(":irc.test.net 322 Guiwald #grenoble 12 :Grenoble");
    assert!(!h.c.chanlist(lw).unwrap().complete);
    h.feed(":irc.test.net 323 Guiwald :End of /LIST");
    let l = h.c.chanlist(lw).unwrap();
    assert!(l.complete);
    let names: Vec<&str> = l.entries.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, vec!["#lyon", "#grenoble"]);
    assert_eq!(l.entries[0].users, 42);
    assert_eq!(l.entries[0].topic, "[+nt] Bienvenue");
    assert_eq!(h.last(lw), "* Channel list: 2 channels");
    assert_eq!(h.c.window(lw).unwrap().title, "Channels List (2)");
    // Nouvelle liste avec filtre de texte.
    h.input(w, "/list gren");
    h.feed(":irc.test.net 321 Guiwald Channel :Users  Name");
    h.feed(":irc.test.net 322 Guiwald #lyon 42 :x");
    h.feed(":irc.test.net 322 Guiwald #grenoble 12 :y");
    h.feed(":irc.test.net 323 Guiwald :End of /LIST");
    let names: Vec<String> = h.c.chanlist(lw).unwrap().entries.iter().map(|e| e.name.clone()).collect();
    assert_eq!(names, vec!["#grenoble"]);
}

#[test]
fn each_new_list_has_a_new_generation() {
    let (mut h, w) = chan();
    h.input(w, "/list");
    let lw = h.c.find_window(Some(h.conn), "Channels List").unwrap();
    let generation = |h: &H| h.c.chanlist(lw).unwrap().generation;
    let g0 = generation(&h);
    h.feed(":irc.test.net 321 Guiwald Channel :Users  Name");
    let g1 = generation(&h);
    assert!(g1 > g0, "le serveur recommence la liste");
    h.feed(":irc.test.net 322 Guiwald #lyon 42 :Lyon");
    h.feed(":irc.test.net 323 Guiwald :End of /LIST");
    assert_eq!(generation(&h), g1, "les entrées s'ajoutent sans changer de liste");
    // Un `/list` tapé à nouveau, même s'il rend autant de canaux.
    h.input(w, "/list");
    assert!(generation(&h) > g1);
    assert!(h.c.chanlist(lw).unwrap().entries.is_empty());
}

#[test]
fn lag_command() {
    let (mut h, w) = chan();
    h.input(w, "/lag");
    assert_eq!(h.sends(), vec![format!("PING :ALPIRC{}", h.now)]);
    let t = h.now;
    h.now += 120;
    h.feed(&format!(":irc.test.net PONG irc.test.net :ALPIRC{t}"));
    assert_eq!(h.last(w), "* Lag: 120ms");
    assert_eq!(h.c.conn_info(h.conn).unwrap().lag_ms, Some(120));
}

#[test]
fn whois_block_in_status() {
    let (mut h, w) = chan();
    h.input(w, "/whois Bob");
    h.feed(":irc.test.net 311 Guiwald Bob bob bob.host * :Bob Real");
    h.feed(":irc.test.net 319 Guiwald Bob :@#lyon +#chambery");
    h.feed(":irc.test.net 312 Guiwald Bob irc.test.net :Test server");
    h.feed(":irc.test.net 301 Guiwald Bob :au café");
    h.feed(":irc.test.net 317 Guiwald Bob 303 1760000000 :seconds idle, signon time");
    h.feed(":irc.test.net 330 Guiwald Bob bobacc :is logged in as");
    h.feed(":irc.test.net 671 Guiwald Bob :is using a secure connection");
    h.feed(":irc.test.net 318 Guiwald Bob :End of /WHOIS list.");
    let l = h.status_lines();
    let n = l.len();
    assert_eq!(
        &l[n - 8..],
        &[
            "Bob is bob@bob.host * Bob Real",
            "Bob on @#lyon +#chambery",
            "Bob using irc.test.net Test server",
            "Bob is away: au café",
            "Bob has been idle 5mins 3secs, signed on Thu Oct 09 08:53:20 2025",
            "Bob is logged in as bobacc",
            "Bob is using a secure connection",
            "Bob End of /WHOIS list.",
        ]
    );
    assert_eq!(h.c.window(h.status).unwrap().lines.back().unwrap().kind, LineKind::Whois);
}

#[test]
fn whois_in_active_option() {
    let mut s = settings();
    s.irc.whois_in_active = true;
    let mut h = H::with(s);
    h.up();
    let w = h.join("#lyon", "Guiwald");
    h.input(w, "/whois Bob");
    h.feed(":irc.test.net 311 Guiwald Bob bob bob.host * :Bob Real");
    h.feed(":irc.test.net 318 Guiwald Bob :End of /WHOIS list.");
    assert_eq!(h.last(w), "Bob End of /WHOIS list.");
}

#[test]
fn amsg_and_ame() {
    let mut h = H::new();
    h.up();
    h.join("#a", "Guiwald");
    let b = h.join("#b", "Guiwald");
    h.input(b, "/amsg salut");
    h.input(b, "/ame part");
    let mut s = h.sends();
    s.sort();
    assert_eq!(s, vec!["PRIVMSG #a :\x01ACTION part\x01", "PRIVMSG #a :salut", "PRIVMSG #b :\x01ACTION part\x01", "PRIVMSG #b :salut"]);
}

#[test]
fn sounds_beep_flash_url() {
    let (mut h, w) = chan();
    h.input(w, "/splay ding.wav");
    assert_eq!(h.last(w), "* /splay: file not found 'ding.wav'");
    std::fs::create_dir_all(h.dir.join("sounds")).unwrap();
    std::fs::write(h.dir.join("sounds/ding.wav"), b"RIFF").unwrap();
    h.input(w, "/splay ding.wav");
    h.input(w, "/splay ../../etc/passwd");
    h.input(w, "/beep 2");
    h.input(w, "/flash");
    h.input(w, "/url https://alpirc.org");
    let fx = h.effects();
    assert!(fx.contains(&Effect::PlaySound(h.dir.join("sounds/ding.wav"))));
    assert_eq!(fx.iter().filter(|e| matches!(e, Effect::PlaySound(_))).count(), 1);
    assert_eq!(fx.iter().filter(|e| **e == Effect::Beep).count(), 2);
    assert!(fx.contains(&Effect::Flash { win: w }));
    assert!(fx.contains(&Effect::OpenUrl("https://alpirc.org".into())));
}

#[test]
fn splay_stop() {
    let (mut h, w) = chan();
    h.effects();
    h.input(w, "/splay stop");
    h.input(w, "/splay -w STOP ");
    let fx = h.effects();
    assert_eq!(fx.iter().filter(|e| **e == Effect::StopSound).count(), 2);
    assert!(!fx.iter().any(|e| matches!(e, Effect::PlaySound(_))));
    assert!(!h.last(w).contains("not found"), "« stop » n'est pas un fichier");
}

#[test]
fn dialogs_and_exit() {
    let (mut h, w) = chan();
    h.input(w, "/channel");
    let fx = h.effects();
    assert!(fx.contains(&Effect::OpenDialog(UiDialog::ChannelCentral(w))));
    assert!(h.sends().contains(&"MODE #lyon +b".to_string()));
    h.input(w, "/options");
    h.input(w, "/abook");
    h.input(w, "/folder");
    h.input(w, "/font");
    h.input(w, "/scripts");
    let fx = h.effects();
    assert_eq!(
        fx,
        vec![
            Effect::OpenDialog(UiDialog::Options(None)),
            Effect::OpenDialog(UiDialog::AddressBook),
            Effect::OpenDialog(UiDialog::ChannelsFolder),
            Effect::OpenDialog(UiDialog::Options(Some("display".into()))),
            Effect::OpenDialog(UiDialog::ScriptEditor),
        ]
    );
    h.input(w, "/exit");
    assert!(h.sends()[0].starts_with("QUIT"));
    assert!(h.effects().contains(&Effect::Exit));
}

#[test]
fn help_and_linesep() {
    let (mut h, w) = chan();
    h.input(w, "/help");
    assert_eq!(h.last(w), "* Full guide: https://github.com/Guiwald/alpirc#readme");
    assert!(h.lines(w).iter().any(|l| l == "* AlpIRC commands (type /help command for details):"));
    assert!(h.lines(w).iter().any(|l| l.contains(" /join ") && l.starts_with("* /")));
    h.input(w, "/help join");
    assert_eq!(h.last(w), "* /join [-i] #channel[,#other] [key] — join channels (-i: where you were last invited)");
    h.input(w, "/help nope");
    assert_eq!(h.last(w), "* No help for /nope");
    h.input(w, "/linesep");
    assert_eq!(h.last(w), "-");
}

#[test]
fn tab_completion() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "Guiwald Pat Paul Bob");
    h.feed(":Paul!p@h PRIVMSG #lyon :un");
    h.now += 1000;
    h.feed(":Pat!p@h PRIVMSG #lyon :deux");
    assert_eq!(h.c.tab_complete(w, "pa", 0).as_deref(), Some("Pat"));
    assert_eq!(h.c.tab_complete(w, "pa", 1).as_deref(), Some("Paul"));
    assert_eq!(h.c.tab_complete(w, "pa", 2).as_deref(), Some("Pat"));
    assert_eq!(h.c.tab_complete(w, "b", 0).as_deref(), Some("Bob"));
    assert_eq!(h.c.tab_complete(w, "zz", 0), None);
    assert_eq!(h.c.tab_complete(w, "#ly", 0).as_deref(), Some("#lyon"));
    assert_eq!(h.c.tab_complete(w, "/jo", 0).as_deref(), Some("/join"));
    h.feed(":Zoe!z@h PRIVMSG Guiwald :hi");
    let q = h.win("Zoe");
    assert_eq!(h.c.tab_complete(q, "z", 0).as_deref(), Some("Zoe"));
}

#[test]
fn command_api_runs_like_input() {
    let (mut h, w) = chan();
    h.cmd(w, "join #x");
    h.cmd(w, "/part #x");
    assert_eq!(h.sends(), vec!["JOIN #x", "PART #x"]);
}

#[test]
fn partall_and_disconnect() {
    let mut h = H::new();
    h.up();
    h.join("#a", "Guiwald");
    let b = h.join("#b", "Guiwald");
    h.input(b, "/partall");
    let mut s = h.sends();
    s.sort();
    assert_eq!(s, vec!["PART #a", "PART #b"]);
    h.input(b, "/disconnect");
    assert_eq!(h.sends(), vec!["QUIT :AlpIRC — https://github.com/Guiwald/alpirc"]);
    assert!(h.effects().contains(&Effect::Close { conn: h.conn }));
}

#[test]
fn mirc_identity_and_option_commands() {
    let mut h = H::new();
    h.up();
    let st = h.status;
    h.input(st, "/anick Guiwald_2");
    assert_eq!(h.last(st), "* Alternate nickname: Guiwald_2");
    h.input(st, "/mnick Guillaume");
    assert_eq!(h.last(st), "* Main nickname: Guillaume");
    assert!(h.sends().is_empty(), "/mnick ne change pas le pseudo du moment");
    h.input(st, "/fullname Guillaume de Lyon");
    assert_eq!(h.last(st), "* Full name: Guillaume de Lyon");
    let id = &h.c.settings().identity;
    assert_eq!((id.alt_nick.as_str(), id.nick.as_str(), id.realname.as_str()), ("Guiwald_2", "Guillaume", "Guillaume de Lyon"));
    assert!(h.effects().contains(&Effect::SaveSettings));
    // /tnick : comme /nick (temporaire).
    h.input(st, "/tnick Gui");
    assert_eq!(h.sends(), vec!["NICK Gui"]);
    // Interrupteurs.
    h.input(st, "/ajinvite on");
    assert!(h.c.settings().irc.join_on_invite);
    assert_eq!(h.last(st), "* Auto-join on invite is on");
    h.input(st, "/creq +m auto");
    assert_eq!(h.c.settings().dcc.on_chat, alpirc_core::settings::DccChatAction::AutoAccept);
    h.input(st, "/sreq ignore");
    assert_eq!(h.c.settings().dcc.on_send, alpirc_core::settings::DccSendAction::Ignore);
    assert_eq!(h.last(st), "* DCC Send: ignore requests");
    h.input(st, "/switchbar off");
    h.input(st, "/toolbar off");
    assert_eq!((h.c.settings().display.switchbar.as_str(), h.c.settings().display.toolbar), ("off", false));
    h.input(st, "/switchbar on");
    assert_eq!(h.c.settings().display.switchbar, "top");
    h.input(st, "/remote off");
    assert!(!h.c.settings().scripts.enabled);
    // Messages à tous les privés de la connexion.
    h.feed(":Bob!b@h PRIVMSG Guiwald :salut");
    h.feed(":Pat!p@h PRIVMSG Guiwald :yo");
    h.clear();
    h.input(st, "/qmsg je reviens");
    let mut sent = h.sends();
    sent.sort();
    assert_eq!(sent, vec!["PRIVMSG Bob :je reviens", "PRIVMSG Pat :je reviens"]);
    // Presse-papiers.
    h.input(st, "/clipboard -a coucou");
    assert!(h.effects().contains(&Effect::Clipboard { text: "coucou".into(), append: true }));
}

#[test]
fn mirc_control_list_commands() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "@Guiwald Bob");
    h.feed(":irc.test.net 352 Guiwald #lyon bob bob.host irc.test.net Bob H :0 Bob");
    h.input(w, "/aop Bob #lyon,#grenoble 2");
    assert_eq!(h.last(w), "* Added *!*@bob.host to the auto-op list");
    let e = h.c.settings().address_book.control[0].clone();
    assert_eq!((e.mask.as_str(), e.channels.clone(), e.network.as_deref()), ("*!*@bob.host", vec!["#lyon".to_string(), "#grenoble".to_string()], Some("TestNet")));
    h.input(w, "/avoice -w Pat");
    assert_eq!(h.c.settings().address_book.control[1].network, None, "-w : tous les réseaux");
    h.input(w, "/aop -l");
    assert!(h.lines(w).iter().any(|l| l.contains("*!*@bob.host #lyon,#grenoble TestNet")), "{:?}", h.lines(w));
    // Éteinte : plus d'auto-op.
    h.input(w, "/aop off");
    assert_eq!(h.last(w), "* auto-op is off");
    assert!(!h.c.settings().address_book.autoop_on);
    h.clear();
    h.feed(":Bob!bob@bob.host JOIN #lyon");
    h.advance(5_000);
    assert!(!h.sends().iter().any(|l| l.starts_with("MODE #lyon +o")));
    h.input(w, "/aop on");
    h.input(w, "/aop -r *!*@bob.host");
    assert_eq!(h.last(w), "* Removed *!*@bob.host from the auto-op list");
    h.input(w, "/aop -r Personne");
    assert_eq!(h.last(w), "* Personne is not in the auto-op list");
}

#[test]
fn savebuf_writes_in_the_scripts_folder() {
    let dir = tmpdir();
    let mut h = H::with_dir(settings(), dir.clone());
    h.up();
    let w = h.join("#lyon", "Guiwald Bob");
    h.feed(":Bob!b@h PRIVMSG #lyon :\u{3}4rouge\u{3} et noir");
    h.input(w, "/savebuf 1 #lyon lyon.txt");
    assert_eq!(h.last(w), "* Saved 1 lines to lyon.txt");
    let text = std::fs::read_to_string(dir.join("scripts/lyon.txt")).unwrap();
    assert!(text.ends_with("<Bob> rouge et noir\n"), "{text:?}");
    h.input(w, "/savebuf -a #lyon lyon.txt");
    assert!(std::fs::read_to_string(dir.join("scripts/lyon.txt")).unwrap().len() > text.len());
    // Hors du dossier des scripts : refusé.
    h.input(w, "/savebuf #lyon ../evasion.txt");
    assert!(h.last(w).contains("unable to write"), "{}", h.last(w));
    assert!(!dir.join("evasion.txt").exists());
}

#[test]
fn dns_host_nick_and_ip() {
    let mut h = H::new();
    h.up();
    let st = h.status;
    h.input(st, "/dns irc.undernet.org");
    assert_eq!(h.last(st), "* Dns resolving irc.undernet.org");
    let id = h.effects().iter().find_map(|e| match e {
        Effect::Resolve { id, host } if host == "irc.undernet.org" => Some(*id),
        _ => None,
    });
    let id = id.expect("recherche demandée à l'hôte");
    let addrs: Vec<std::net::IpAddr> = vec!["192.0.2.1".parse().unwrap(), "2001:db8::1".parse().unwrap()];
    h.c.on_resolved(id, Ok(&addrs), h.now);
    assert_eq!(h.last(st), "* Dns resolved irc.undernet.org to 192.0.2.1, 2001:db8::1");
    // Un pseudo dont l'adresse n'est pas connue : USERHOST d'abord, sans rien afficher de la réponse.
    h.input(st, "/dns Bob");
    assert_eq!(h.sends(), vec!["USERHOST Bob"]);
    h.feed(":irc.test.net 302 Guiwald :Bob=+bob@bob.example.org");
    assert_eq!(h.last(st), "* Dns resolving bob.example.org");
    let id = h.effects().iter().find_map(|e| match e {
        Effect::Resolve { id, .. } => Some(*id),
        _ => None,
    });
    h.c.on_resolved(id.unwrap(), Err(&alpirc_core::NetError::NoAddress { host: "bob.example.org".into() }), h.now);
    assert_eq!(h.last(st), "* Dns unable to resolve bob.example.org");
    // Une adresse IP : pas de recherche inverse.
    h.input(st, "/dns 192.0.2.7");
    assert!(h.last(st).contains("192.0.2.7") && h.last(st).contains("not available"));
}

#[test]
fn debug_window_shows_both_directions() {
    let mut h = H::new();
    h.up();
    let st = h.status;
    h.input(st, "/debug @raw");
    assert_eq!(h.last(st), "* Debug output to @raw");
    let raw = h.win("@raw");
    h.input(st, "/msg Bob coucou");
    h.pull();
    h.feed(":Bob!b@h PRIVMSG Guiwald :salut");
    let l = h.lines(raw);
    assert!(l.iter().any(|x| x.ends_with("-> PRIVMSG Bob :coucou")), "{l:?}");
    assert!(l.iter().any(|x| x.ends_with("<- :Bob!b@h PRIVMSG Guiwald :salut")), "{l:?}");
    h.input(st, "/debug off");
    assert_eq!(h.last(st), "* Debug output is off");
    h.feed(":Bob!b@h PRIVMSG Guiwald :encore");
    assert_eq!(h.lines(raw).len(), l.len());
}

#[test]
fn join_last_invite() {
    let mut h = H::new();
    h.up();
    let st = h.status;
    h.feed(":Kaelis!k@h INVITE Guiwald :#secret");
    h.clear();
    h.input(st, "/join -i");
    assert_eq!(h.sends(), vec!["JOIN #secret"]);
}
