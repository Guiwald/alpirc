//! Messages : texte, actions, avis, privés, mots surlignés et alertes, server-time, echo-message, CTCP et garde-fou,
//! DCC, découpage des longs messages, absence, URL catcher, historique (chathistory), marqueurs de lecture.

mod common;

use alpirc_core::settings::HighlightWord;
use alpirc_core::timeutil::format_iso8601;
use alpirc_core::{Activity, Effect, LineKind, WinKind};
use common::*;

#[test]
fn channel_text_and_action() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "Guiwald @Pat");
    h.feed(":Pat!p@h PRIVMSG #lyon :salut à tous");
    assert_eq!(h.last(w), "<Pat> salut à tous");
    let l = h.c.window(w).unwrap().lines.back().unwrap().clone();
    assert_eq!(l.kind, LineKind::Normal);
    assert_eq!(l.nick.as_deref(), Some("Pat"));
    h.feed(":Pat!p@h PRIVMSG #lyon :\x01ACTION danse\x01");
    assert_eq!(h.last(w), "* Pat danse");
    assert_eq!(h.c.window(w).unwrap().lines.back().unwrap().kind, LineKind::Action);
}

#[test]
fn mode_prefix_option() {
    let mut s = settings();
    s.irc.show_mode_prefix = true;
    let mut h = H::with(s);
    h.up();
    let w = h.join("#lyon", "Guiwald @Pat +Vic");
    h.feed(":Pat!p@h PRIVMSG #lyon :hi");
    assert_eq!(h.last(w), "<@Pat> hi");
    h.feed(":Vic!v@h PRIVMSG #lyon :yo");
    assert_eq!(h.last(w), "<+Vic> yo");
}

#[test]
fn nick_colors_option() {
    let mut s = settings();
    s.irc.nick_colors = true;
    let mut h = H::with(s);
    h.up();
    let w = h.join("#lyon", "Guiwald Pat");
    h.feed(":Pat!p@h PRIVMSG #lyon :hi");
    let l = h.last(w);
    assert!(l.starts_with("<\x03") && l.ends_with("Pat\x03> hi"), "{l:?}");
}

#[test]
fn strip_codes_option() {
    let mut s = settings();
    s.irc.strip_codes = true;
    let mut h = H::with(s);
    h.up();
    let w = h.join("#lyon", "Guiwald Pat");
    h.feed(":Pat!p@h PRIVMSG #lyon :\x034red\x03 \x02bold\x02");
    assert_eq!(h.last(w), "<Pat> red bold");
}

#[test]
fn own_message_without_echo() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "Guiwald");
    h.input(w, "hello world");
    assert_eq!(h.sends(), vec!["PRIVMSG #lyon :hello world"]);
    assert_eq!(h.last(w), "<Guiwald> hello world");
    assert_eq!(h.c.window(w).unwrap().lines.back().unwrap().kind, LineKind::Own);
}

#[test]
fn own_message_with_echo_message() {
    let mut h = H::new();
    h.up_with_caps("echo-message");
    let w = h.join("#lyon", "Guiwald");
    h.input(w, "hello");
    assert_eq!(h.sends(), vec!["PRIVMSG #lyon :hello"]);
    assert_eq!(h.last(w), "* Now talking in #lyon");
    h.feed(":Guiwald!alpirc@host.test PRIVMSG #lyon :hello");
    assert_eq!(h.last(w), "<Guiwald> hello");
    assert_eq!(h.c.window(w).unwrap().lines.back().unwrap().kind, LineKind::Own);
    // Écho d'un privé sans fenêtre : dans la fenêtre active.
    h.feed(":Guiwald!alpirc@host.test PRIVMSG Bob :psst");
    assert_eq!(h.last(w), "-> *Bob* psst");
    // Écho de notre action.
    h.feed(":Guiwald!alpirc@host.test PRIVMSG #lyon :\x01ACTION rit\x01");
    assert_eq!(h.last(w), "* Guiwald rit");
}

#[test]
fn private_notice_goes_to_active_window() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "Guiwald");
    h.feed(":Pat!p@h NOTICE Guiwald :coucou");
    assert_eq!(h.last(w), "-Pat- coucou");
    assert_eq!(h.c.window(w).unwrap().lines.back().unwrap().kind, LineKind::Notice);
    let mut s = settings();
    s.irc.notices_in_active = false;
    let mut h = H::with(s);
    h.up();
    h.join("#lyon", "Guiwald");
    h.feed(":Pat!p@h NOTICE Guiwald :coucou");
    assert_eq!(h.last(h.status), "-Pat- coucou");
}

#[test]
fn channel_notice() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "Guiwald Pat");
    h.feed(":Pat!p@h NOTICE #lyon :annonce");
    assert_eq!(h.last(w), "-Pat:#lyon- annonce");
}

#[test]
fn server_notice_in_status() {
    let mut h = H::new();
    h.up();
    h.join("#lyon", "Guiwald");
    h.feed(":irc.test.net NOTICE Guiwald :*** Notice -- server info");
    assert_eq!(h.last(h.status), "-irc.test.net- *** Notice -- server info");
}

#[test]
fn private_message_opens_query_and_notifies() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "Guiwald");
    h.feed(":Pat!p@p.host PRIVMSG Guiwald :psst");
    let q = h.win("Pat");
    assert_eq!(h.kind_of("Pat"), WinKind::Query);
    assert_eq!(h.last(q), "<Pat> psst");
    assert_eq!(h.c.window(q).unwrap().activity, Activity::Message);
    assert_eq!(h.c.window(q).unwrap().title, "Pat (p@p.host)");
    assert_eq!(h.c.active(), Some(w), "un privé reçu ne vole pas le focus");
    let fx = h.effects();
    assert!(fx.contains(&Effect::Notify { win: q, title: "Private message from Pat".into(), body: "psst".into() }), "{fx:?}");
    assert!(!fx.contains(&Effect::Beep));
}

#[test]
fn private_message_without_query_option() {
    let mut s = settings();
    s.irc.query_on_private = false;
    let mut h = H::with(s);
    h.up();
    let w = h.join("#lyon", "Guiwald");
    h.feed(":Pat!p@h PRIVMSG Guiwald :psst");
    assert!(!h.has_win("Pat"));
    assert_eq!(h.last(w), "*Pat* psst");
}

#[test]
fn beep_on_query_option() {
    let mut s = settings();
    s.highlight.beep_on_query = true;
    let mut h = H::with(s);
    h.up();
    h.join("#lyon", "Guiwald");
    h.feed(":Pat!p@h PRIVMSG Guiwald :psst");
    assert!(h.effects().contains(&Effect::Beep));
}

#[test]
fn highlight_own_nick() {
    let mut h = H::new();
    h.up();
    let a = h.join("#a", "Guiwald Pat");
    let _b = h.join("#b", "Guiwald");
    h.feed(":Pat!p@h PRIVMSG #a :Guiwald: tu es là ?");
    let l = h.c.window(a).unwrap().lines.back().unwrap().clone();
    assert!(l.highlight);
    assert_eq!(l.kind, LineKind::Highlight);
    assert_eq!(h.c.window(a).unwrap().activity, Activity::Highlight);
    h.feed(":Pat!p@h PRIVMSG #a :Guiwaldo n'est pas moi");
    assert!(!h.c.window(a).unwrap().lines.back().unwrap().highlight);
}

#[test]
fn highlight_effects_only_when_unseen() {
    let mut s = settings();
    s.highlight.beep = true;
    let mut h = H::with(s);
    h.up();
    let a = h.join("#a", "Guiwald Pat");
    h.feed(":Pat!p@h PRIVMSG #a :hey guiwald");
    let fx = h.effects();
    assert!(!fx.iter().any(|e| matches!(e, Effect::Beep | Effect::Notify { .. } | Effect::Flash { .. })), "{fx:?}");
    // Autre fenêtre active : bip, clignotement et notification.
    h.join("#b", "Guiwald");
    h.feed(":Pat!p@h PRIVMSG #a :hey guiwald");
    let fx = h.effects();
    assert!(fx.contains(&Effect::Beep), "{fx:?}");
    assert!(fx.contains(&Effect::Flash { win: a }));
    assert!(fx.contains(&Effect::Notify { win: a, title: "Pat on #a".into(), body: "hey guiwald".into() }));
}

#[test]
fn highlight_when_app_unfocused() {
    let mut h = H::new();
    h.up();
    let a = h.join("#a", "Guiwald Pat");
    h.c.set_app_focused(false);
    h.feed(":Pat!p@h PRIVMSG #a :Guiwald!");
    assert!(h.effects().iter().any(|e| matches!(e, Effect::Notify { win, .. } if *win == a)));
}

#[test]
fn notifications_disabled() {
    let mut s = settings();
    s.notifications.enabled = false;
    let mut h = H::with(s);
    h.up();
    h.join("#a", "Guiwald Pat");
    h.join("#b", "Guiwald");
    h.feed(":Pat!p@h PRIVMSG #a :Guiwald!");
    assert!(!h.effects().iter().any(|e| matches!(e, Effect::Notify { .. })));
}

#[test]
fn highlight_words_with_color() {
    let mut s = settings();
    s.highlight.words.push(HighlightWord { text: "alpirc".into(), color: Some(7), beep: true, flash: false, notify: false });
    let mut h = H::with(s);
    h.up();
    let a = h.join("#a", "Guiwald Pat");
    h.join("#b", "Guiwald");
    h.feed(":Pat!p@h PRIVMSG #a :j'aime AlpIRC.");
    let l = h.c.window(a).unwrap().lines.back().unwrap().clone();
    assert!(l.highlight);
    assert_eq!(l.color, Some(7));
    assert!(h.effects().contains(&Effect::Beep));
}

#[test]
fn highlight_disabled() {
    let mut s = settings();
    s.highlight.enabled = false;
    let mut h = H::with(s);
    h.up();
    let a = h.join("#a", "Guiwald Pat");
    h.feed(":Pat!p@h PRIVMSG #a :Guiwald!");
    assert!(!h.c.window(a).unwrap().lines.back().unwrap().highlight);
}

#[test]
fn set_active_clears_activity_and_marker() {
    let mut h = H::new();
    h.up();
    let a = h.join("#a", "Guiwald Pat");
    let b = h.join("#b", "Guiwald");
    h.feed(":Pat!p@h PRIVMSG #a :un");
    h.feed(":Pat!p@h PRIVMSG #a :deux");
    let wa = h.c.window(a).unwrap();
    let first_unread = wa.lines.iter().find(|l| l.text == "<Pat> un").unwrap().seq;
    assert_eq!(wa.unread_marker, Some(first_unread));
    h.c.set_active(a, h.now);
    assert_eq!(h.c.window(a).unwrap().activity, Activity::None);
    assert_eq!(h.c.window(a).unwrap().unread_marker, Some(first_unread), "la barre reste tant qu'on la regarde");
    h.c.set_active(b, h.now);
    assert_eq!(h.c.window(a).unwrap().unread_marker, None);
}

#[test]
fn server_time_tag() {
    let mut h = H::new();
    h.up_with_caps("server-time");
    let w = h.join("#lyon", "Guiwald Pat");
    h.feed(&format!("@time={} :Pat!p@h PRIVMSG #lyon :x", format_iso8601(T0 - 3_600_000)));
    assert_eq!(h.c.window(w).unwrap().lines.back().unwrap().time_ms, T0 - 3_600_000);
}

#[test]
fn timestamps_prefix() {
    let mut s = settings();
    s.irc.timestamps = true;
    let mut h = H::with(s);
    h.up();
    let w = h.join("#lyon", "Guiwald Pat");
    h.feed(":Pat!p@h PRIVMSG #lyon :hi");
    // T0 = 2025-10-09 08:53:20 UTC.
    assert_eq!(h.last(w), "[08:53] <Pat> hi");
    h.c.settings_mut().irc.timestamp_format = "[HH:nn:ss]".into();
    h.feed(":Pat!p@h PRIVMSG #lyon :hi");
    assert_eq!(h.last(w), "[08:53:20] <Pat> hi");
    h.c.set_utc_offset(Some(7200));
    h.feed(":Pat!p@h PRIVMSG #lyon :hi");
    assert_eq!(h.last(w), "[10:53:20] <Pat> hi");
}

#[test]
fn ctcp_version_reply() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "Guiwald");
    h.feed(":Pat!p@h PRIVMSG Guiwald :\x01VERSION\x01");
    assert_eq!(h.sends(), vec![format!("NOTICE Pat :\x01VERSION AlpIRC {} (Rust) https://github.com/Guiwald/alpirc\x01", alpirc_core::client::VERSION)]);
    assert_eq!(h.last(w), "[Pat VERSION]");
    assert_eq!(h.c.window(w).unwrap().lines.back().unwrap().kind, LineKind::Ctcp);
    h.feed(":Pat!p@h PRIVMSG #lyon :\x01TIME\x01");
    assert_eq!(h.last(w), "[Pat:#lyon TIME]");
    assert!(h.sends()[0].starts_with("NOTICE Pat :\x01TIME Thu Oct 09 08:53:20 2025"));
}

#[test]
fn ctcp_replies_ping_clientinfo_source_custom_version() {
    let mut s = settings();
    s.ctcp.version_reply = Some("mIRC v6.17 Khaled Mardam-Bey".into());
    s.ctcp.reply_finger = false;
    let mut h = H::with(s);
    h.up();
    h.feed(":Pat!p@h PRIVMSG Guiwald :\x01PING 12345 678\x01");
    h.feed(":Pat!p@h PRIVMSG Guiwald :\x01CLIENTINFO\x01");
    h.advance(20_000);
    h.feed(":Pat!p@h PRIVMSG Guiwald :\x01SOURCE\x01");
    h.feed(":Pat!p@h PRIVMSG Guiwald :\x01VERSION\x01");
    h.feed(":Pat!p@h PRIVMSG Guiwald :\x01FINGER\x01");
    let s = h.sends();
    let s: Vec<&String> = s.iter().filter(|l| l.starts_with("NOTICE")).collect();
    assert_eq!(s[0], "NOTICE Pat :\x01PING 12345 678\x01");
    assert!(s[1].contains("CLIENTINFO ACTION CLIENTINFO DCC"));
    assert_eq!(s[2], "NOTICE Pat :\x01SOURCE https://github.com/Guiwald/alpirc\x01");
    assert_eq!(s[3], "NOTICE Pat :\x01VERSION mIRC v6.17 Khaled Mardam-Bey\x01");
    assert_eq!(s.len(), 4, "FINGER désactivé");
}

#[test]
fn ctcp_flood_guard() {
    let mut h = H::new();
    h.up();
    for _ in 0..6 {
        h.feed(":Pat!p@h PRIVMSG Guiwald :\x01VERSION\x01");
    }
    assert_eq!(h.sends().len(), 3);
    h.advance(10_000);
    h.sends();
    h.feed(":Pat!p@h PRIVMSG Guiwald :\x01VERSION\x01");
    assert_eq!(h.sends().len(), 1);
}

#[test]
fn ctcp_reply_display_and_ping_time() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "Guiwald");
    h.feed(":Pat!p@h NOTICE Guiwald :\x01VERSION xchat 2.8\x01");
    assert_eq!(h.last(w), "[Pat VERSION reply]: xchat 2.8");
    h.input(w, "/ctcp Pat ping");
    let sent = h.sends();
    assert_eq!(sent, vec![format!("PRIVMSG Pat :\x01PING {}\x01", h.now)]);
    assert_eq!(h.last(w), format!("-> [Pat] PING {}", h.now));
    let t = h.now;
    h.now += 1500;
    h.feed(&format!(":Pat!p@h NOTICE Guiwald :\x01PING {t}\x01"));
    assert_eq!(h.last(w), "[Pat PING reply]: 1.500secs");
}

#[test]
fn dcc_offer_shown_not_accepted() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "Guiwald");
    h.feed(":Pat!p@h PRIVMSG Guiwald :\x01DCC SEND \"my file.txt\" 3232235777 5000 2048\x01");
    assert_eq!(h.last(w), "* DCC SEND offer from Pat: my file.txt (2.0KB)");
    h.feed(":Pat!p@h PRIVMSG Guiwald :\x01DCC CHAT chat 3232235777 5001\x01");
    assert_eq!(h.last(w), "* DCC CHAT offer from Pat");
    assert!(h.sends().is_empty());
    assert!(h.connects().is_empty());
}

#[test]
fn long_message_split() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "Guiwald");
    let word = "abcdefghij ";
    let text: String = word.repeat(120).trim_end().to_string();
    h.input(w, &text);
    let s = h.sends();
    assert!(s.len() >= 3, "{}", s.len());
    let mut rebuilt = Vec::new();
    for l in &s {
        let relayed = format!(":Guiwald!alpirc@host.test {l}\r\n");
        assert!(relayed.len() <= 512, "{}", relayed.len());
        rebuilt.push(l.strip_prefix("PRIVMSG #lyon :").unwrap().to_string());
    }
    assert_eq!(rebuilt.join(" "), text);
    assert_eq!(h.lines(w).len(), 1 + s.len());
}

#[test]
fn multiline_input_sends_each_line() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "Guiwald");
    h.input(w, "un\r\ndeux\n\ntrois");
    assert_eq!(h.sends(), vec!["PRIVMSG #lyon :un", "PRIVMSG #lyon :deux", "PRIVMSG #lyon :trois"]);
    assert_eq!(h.c.window(w).unwrap().history, vec!["un", "deux", "trois"]);
}

#[test]
fn away_message_shown_once_in_query() {
    let mut h = H::new();
    h.up();
    h.feed(":Bob!b@h PRIVMSG Guiwald :hi");
    let q = h.win("Bob");
    h.input(q, "ça va ?");
    h.feed(":irc.test.net 301 Guiwald Bob :au déjeuner");
    assert_eq!(h.last(q), "Bob is away: au déjeuner");
    h.input(q, "allô ?");
    h.feed(":irc.test.net 301 Guiwald Bob :au déjeuner");
    assert_eq!(h.last(q), "<Guiwald> allô ?");
    h.feed(":irc.test.net 301 Guiwald Bob :parti");
    assert_eq!(h.last(q), "Bob is away: parti");
}

#[test]
fn url_catcher() {
    let mut h = H::new();
    h.up();
    h.join("#lyon", "Guiwald Pat");
    h.feed(":Pat!p@h PRIVMSG #lyon :regarde https://alpirc.org/docs. et www.example.fr");
    let u = h.c.urls();
    assert_eq!(u.len(), 2);
    assert_eq!(u[0].url, "https://alpirc.org/docs");
    assert_eq!(u[0].nick, "Pat");
    assert_eq!(u[0].window, "#lyon");
    assert_eq!(u[1].url, "www.example.fr");
    // Doublon : remonté en dernier, pas répété.
    h.feed(":Bob!b@h PRIVMSG #lyon :https://alpirc.org/docs");
    assert_eq!(h.c.urls().len(), 2);
    assert_eq!(h.c.urls()[1].nick, "Bob");
}

#[test]
fn url_catcher_limit_and_disable() {
    let mut s = settings();
    s.catcher.max = 2;
    let mut h = H::with(s);
    h.up();
    h.join("#lyon", "Guiwald Pat");
    for i in 0..4 {
        h.feed(&format!(":Pat!p@h PRIVMSG #lyon :http://u{i}.org"));
    }
    let urls: Vec<String> = h.c.urls().iter().map(|u| u.url.clone()).collect();
    assert_eq!(urls, vec!["http://u2.org", "http://u3.org"]);
    let mut s = settings();
    s.catcher.enabled = false;
    let mut h = H::with(s);
    h.up();
    h.join("#lyon", "Guiwald Pat");
    h.feed(":Pat!p@h PRIVMSG #lyon :http://x.org");
    assert!(h.c.urls().is_empty());
}

#[test]
fn chathistory_batch_inserted_before_live_lines() {
    let mut s = settings();
    s.highlight.beep = true;
    let mut h = H::with(s);
    h.up_with_caps("batch server-time draft/chathistory message-tags");
    h.feed(":Guiwald!alpirc@host.test JOIN #lyon");
    assert!(h.sends().contains(&"CHATHISTORY LATEST #lyon * 100".to_string()));
    let w = h.win("#lyon");
    let other = h.join("#other", "Guiwald");
    h.clear();
    h.feed(&format!("@time={} :Pat!p@h PRIVMSG #lyon :live", format_iso8601(T0 + 5_000)));
    h.feed(":irc.test.net BATCH +hist chathistory #lyon");
    h.feed(&format!("@batch=hist;time={};msgid=m1 :Bob!b@h PRIVMSG #lyon :old Guiwald", format_iso8601(T0 - 60_000)));
    h.feed(&format!("@batch=hist;time={};msgid=m2 :Bob!b@h PRIVMSG #lyon :\x01ACTION older\x01", format_iso8601(T0 - 30_000)));
    h.feed(&format!("@batch=hist;time={} :Carl!c@h JOIN #lyon", format_iso8601(T0 - 20_000)));
    assert_eq!(h.lines(w).len(), 2, "rien avant la fin du lot");
    h.feed(":irc.test.net BATCH -hist");
    let lines = h.lines(w);
    assert_eq!(lines, vec!["<Bob> old Guiwald", "* Bob older", "* Joins: Carl (c@h)", "* Now talking in #lyon", "<Pat> live"]);
    let first = h.c.window(w).unwrap().lines[0].clone();
    assert!(first.from_history && first.highlight);
    assert_eq!(first.msgid.as_deref(), Some("m1"));
    let fx = h.effects();
    assert!(!fx.iter().any(|e| matches!(e, Effect::Beep | Effect::Notify { .. })), "{fx:?}");
    // L'historique ne change pas l'état : Carl n'est pas dans la liste.
    assert!(!h.nicks(w).contains(&"Carl".to_string()));
    // Un second lot avec les mêmes msgid ne double rien.
    h.feed(":irc.test.net BATCH +h2 chathistory #lyon");
    h.feed(&format!("@batch=h2;time={};msgid=m1 :Bob!b@h PRIVMSG #lyon :old Guiwald", format_iso8601(T0 - 60_000)));
    h.feed(":irc.test.net BATCH -h2");
    assert_eq!(h.lines(w).len(), 5);
    let _ = other;
}

#[test]
fn read_markers_sync() {
    let mut h = H::new();
    h.up_with_caps("draft/read-marker server-time");
    h.feed(":Guiwald!alpirc@host.test JOIN #a");
    assert!(h.sends().contains(&"MARKREAD #a".to_string()));
    let a = h.win("#a");
    h.join("#b", "Guiwald");
    h.feed(":Pat!p@h PRIVMSG #a :lu ailleurs");
    assert_eq!(h.c.window(a).unwrap().activity, Activity::Message);
    // Lu sur un autre appareil.
    h.feed(&format!(":irc.test.net MARKREAD #a timestamp={}", format_iso8601(T0 + 1)));
    assert_eq!(h.c.window(a).unwrap().activity, Activity::None);
    assert_eq!(h.c.window(a).unwrap().read_marker_ms, Some(T0 + 1));
    // Un marqueur plus ancien que la dernière ligne ne l'efface pas.
    h.now += 10_000;
    h.feed(":Pat!p@h PRIVMSG #a :nouveau");
    h.feed(&format!(":irc.test.net MARKREAD #a timestamp={}", format_iso8601(T0 + 2)));
    assert_eq!(h.c.window(a).unwrap().activity, Activity::Message);
    h.clear();
    h.c.set_active(a, h.now);
    assert_eq!(h.sends(), vec![format!("MARKREAD #a timestamp={}", format_iso8601(h.now))]);
}

#[test]
fn statusmsg_messages() {
    let mut h = H::new();
    h.connect();
    h.register_with("PREFIX=(ov)@+ CHANTYPES=# STATUSMSG=@+");
    h.clear();
    let w = h.join("#lyon", "@Guiwald @Pat");
    h.feed(":Pat!p@h PRIVMSG @#lyon :ops only");
    assert_eq!(h.last(w), "<Pat:@#lyon> ops only");
    h.input(w, "/omsg salut les ops");
    assert_eq!(h.sends(), vec!["PRIVMSG @#lyon :salut les ops"]);
    assert_eq!(h.last(w), "<Guiwald:@#lyon> salut les ops");
    h.input(w, "/onotice #lyon avis");
    assert_eq!(h.sends(), vec!["NOTICE @#lyon :avis"]);
}

#[test]
fn wallops_in_status() {
    let mut h = H::new();
    h.up();
    h.feed(":oper!o@h WALLOPS :maintenance");
    assert_eq!(h.last(h.status), "!oper! maintenance");
}

#[test]
fn search_lines() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "Guiwald Pat");
    h.feed(":Pat!p@h PRIVMSG #lyon :Hello \x02World\x02");
    h.feed(":Pat!p@h PRIVMSG #lyon :autre");
    assert_eq!(h.c.search(w, "hello world"), vec![1]);
    assert!(h.c.search(w, "absent").is_empty());
}

#[test]
fn max_lines_trimmed() {
    let mut s = settings();
    s.irc.max_lines = 10;
    let mut h = H::with(s);
    h.up();
    let w = h.join("#lyon", "Guiwald Pat");
    for i in 0..30 {
        h.feed(&format!(":Pat!p@h PRIVMSG #lyon :{i}"));
    }
    let l = h.lines(w);
    assert_eq!(l.len(), 10);
    assert_eq!(l[9], "<Pat> 29");
}

#[test]
fn incoming_query_requests_history() {
    let mut h = H::new();
    h.up_with_caps("batch server-time draft/chathistory message-tags");
    h.feed("@msgid=live1 :Pat!p@h PRIVMSG Guiwald :salut");
    assert_eq!(h.sends(), vec!["CHATHISTORY LATEST Pat * 100"]);
    let q = h.win("Pat");
    h.feed(":irc.test.net BATCH +q chathistory Pat");
    h.feed(&format!("@batch=q;time={};msgid=old :Pat!p@h PRIVMSG Guiwald :hier", format_iso8601(T0 - 86_400_000)));
    h.feed(&format!("@batch=q;time={};msgid=live1 :Pat!p@h PRIVMSG Guiwald :salut", format_iso8601(T0)));
    h.feed(":irc.test.net BATCH -q");
    assert_eq!(h.lines(q), vec!["<Pat> hier", "<Pat> salut"]);
}
