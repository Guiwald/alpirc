//! Canaux : arrivée, NAMES, départs, expulsions, QUIT, NICK, MODE, sujet, préfixes multiples, extended-join,
//! away-notify, account-notify, chghost, userhost-in-names, WHO/WHOX, listes de bans, invitations.

mod common;

use alpirc_core::timeutil;
use alpirc_core::{Activity, Effect, LineKind, WinKind};
use common::*;

#[test]
fn own_join_opens_and_activates_window() {
    let mut h = H::new();
    h.up();
    h.feed(":Guiwald!alpirc@host.test JOIN #lyon");
    let w = h.win("#lyon");
    assert_eq!(h.kind_of("#lyon"), WinKind::Channel);
    assert!(h.c.window(w).unwrap().joined);
    assert!(h.effects().contains(&Effect::Activate(w)));
    assert_eq!(h.c.active(), Some(w));
    assert_eq!(h.last(w), "* Now talking in #lyon");
    assert!(h.sends().contains(&"MODE #lyon".to_string()));
}

#[test]
fn topic_on_join() {
    let mut h = H::new();
    h.up();
    h.feed(":Guiwald!alpirc@host.test JOIN #lyon");
    h.feed(":irc.test.net 332 Guiwald #lyon :Bienvenue à Lyon");
    h.feed(":irc.test.net 333 Guiwald #lyon Pat!p@h 1760000000");
    let w = h.win("#lyon");
    let lines = h.lines(w);
    assert_eq!(lines[1], "* Topic is 'Bienvenue à Lyon'");
    assert_eq!(lines[2], format!("* Set by Pat!p@h on {}", timeutil::asctime(1_760_000_000_000, 0)));
    assert_eq!(h.c.window(w).unwrap().title, "#lyon: Bienvenue à Lyon");
    let st = h.c.channel_state(w).unwrap();
    assert_eq!(st.topic_setter.as_deref(), Some("Pat!p@h"));
    assert_eq!(st.topic_time, Some(1_760_000_000));
}

#[test]
fn names_fill_nicklist_silently() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "@Pat +Bob Guiwald carl");
    assert_eq!(h.nicks(w), vec!["@Pat", "+Bob", "carl", "Guiwald"]);
    assert_eq!(h.lines(w), vec!["* Now talking in #lyon"]);
    // NAMES demandé ensuite : affiché.
    h.feed(":irc.test.net 353 Guiwald = #lyon :@Pat +Bob Guiwald carl Dan");
    h.feed(":irc.test.net 366 Guiwald #lyon :End of /NAMES list.");
    let l = h.lines(w);
    assert!(l.contains(&"* Names #lyon: @Pat +Bob Guiwald carl Dan".to_string()), "{l:?}");
    assert_eq!(h.nicks(w).len(), 5);
}

#[test]
fn others_join_and_part() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "Guiwald");
    h.feed(":Bob!bob@bob.host JOIN #lyon");
    assert_eq!(h.last(w), "* Joins: Bob (bob@bob.host)");
    assert!(h.nicks(w).contains(&"Bob".to_string()));
    h.feed(":Bob!bob@bob.host PART #lyon :see you");
    assert_eq!(h.last(w), "* Parts: Bob (bob@bob.host) (see you)");
    h.feed(":Carl!c@c.host JOIN #lyon");
    h.feed(":Carl!c@c.host PART #lyon");
    assert_eq!(h.last(w), "* Parts: Carl (c@c.host)");
    assert_eq!(h.nicks(w), vec!["Guiwald"]);
    assert_eq!(h.c.window(w).unwrap().lines.back().unwrap().kind, LineKind::Part);
}

#[test]
fn joins_parts_hidden_by_option() {
    let mut s = settings();
    s.irc.show_joins_parts = false;
    let mut h = H::with(s);
    h.up();
    let w = h.join("#lyon", "Guiwald");
    h.feed(":Bob!bob@h JOIN #lyon");
    assert_eq!(h.lines(w), vec!["* Now talking in #lyon"]);
    assert_eq!(h.nicks(w), vec!["Bob", "Guiwald"]);
}

#[test]
fn own_part_closes_window() {
    let mut h = H::new();
    h.up();
    h.join("#lyon", "Guiwald");
    h.feed(":Guiwald!alpirc@host.test PART #lyon :bye");
    assert!(!h.has_win("#lyon"));
    assert!(h.c.session(h.conn).unwrap().channel("#lyon").is_none());
}

#[test]
fn kick_other_and_self() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "@Pat Bob Guiwald");
    h.feed(":Pat!p@h KICK #lyon Bob :out");
    assert_eq!(h.last(w), "* Bob was kicked by Pat (out)");
    assert_eq!(h.nicks(w), vec!["@Pat", "Guiwald"]);
    h.feed(":Pat!p@h KICK #lyon Guiwald :you too");
    assert_eq!(h.last(w), "* You were kicked from #lyon by Pat (you too)");
    assert!(!h.c.window(w).unwrap().joined);
    assert!(h.sends().iter().all(|l| !l.starts_with("JOIN")));
}

#[test]
fn rejoin_on_kick_option() {
    let mut s = settings();
    s.irc.rejoin_on_kick = true;
    let mut h = H::with(s);
    h.up();
    h.join("#lyon", "@Pat Guiwald");
    h.feed(":Pat!p@h KICK #lyon Guiwald :bye");
    assert!(h.sends().contains(&"JOIN #lyon".to_string()));
}

#[test]
fn kick_closes_window_without_keep_option() {
    let mut s = settings();
    s.irc.keep_channels_open = false;
    let mut h = H::with(s);
    h.up();
    h.join("#lyon", "@Pat Guiwald");
    h.feed(":Pat!p@h KICK #lyon Guiwald :bye");
    assert!(!h.has_win("#lyon"));
    assert_eq!(h.last(h.status), "* You were kicked from #lyon by Pat (bye)");
}

#[test]
fn quit_in_every_shared_window() {
    let mut h = H::new();
    h.up();
    let a = h.join("#a", "Guiwald Bob");
    let b = h.join("#b", "Guiwald Bob");
    let c = h.join("#c", "Guiwald");
    h.feed(":Bob!bob@h PRIVMSG Guiwald :hi");
    let q = h.win("Bob");
    h.feed(":Bob!bob@h QUIT :Leaving");
    for w in [a, b, q] {
        assert_eq!(h.last(w), "* Quits: Bob (bob@h) (Leaving)");
    }
    assert_eq!(h.last(c), "* Now talking in #c");
    assert_eq!(h.nicks(a), vec!["Guiwald"]);
    assert!(h.c.session(h.conn).unwrap().user_info("Bob").is_none());
}

#[test]
fn nick_change_everywhere_and_query_renamed() {
    let mut h = H::new();
    h.up();
    let a = h.join("#a", "Guiwald @Bob");
    h.feed(":Bob!bob@h PRIVMSG Guiwald :hi");
    let q = h.win("Bob");
    h.feed(":Bob!bob@h NICK :Robert");
    assert_eq!(h.last(a), "* Bob is now known as Robert");
    assert_eq!(h.last(q), "* Bob is now known as Robert");
    assert_eq!(h.c.window(q).unwrap().name, "Robert");
    assert_eq!(h.nicks(a), vec!["@Robert", "Guiwald"]);
}

#[test]
fn own_nick_change() {
    let mut h = H::new();
    h.up();
    let a = h.join("#a", "Guiwald");
    h.feed(":Guiwald!alpirc@host.test NICK Gui2");
    assert_eq!(h.last(a), "* Your nick is now Gui2");
    assert_eq!(h.last(h.status), "* Your nick is now Gui2");
    assert_eq!(h.c.conn_info(h.conn).unwrap().nick, "Gui2");
}

#[test]
fn nick_changes_hidden_by_option() {
    let mut s = settings();
    s.irc.show_nick_changes = false;
    let mut h = H::with(s);
    h.up();
    let a = h.join("#a", "Guiwald Bob");
    h.feed(":Bob!b@h NICK Robert");
    assert_eq!(h.last(a), "* Now talking in #a");
}

#[test]
fn mode_display_and_prefix_state() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "@Pat Bob Guiwald");
    h.feed(":Pat!p@h MODE #lyon +ov Bob Guiwald");
    assert_eq!(h.last(w), "* Pat sets mode: +ov Bob Guiwald");
    assert_eq!(h.nicks(w), vec!["@Bob", "@Pat", "+Guiwald"]);
    h.feed(":Pat!p@h MODE #lyon -o Bob");
    assert_eq!(h.nicks(w), vec!["@Pat", "+Guiwald", "Bob"]);
    assert_eq!(h.c.window(w).unwrap().lines.back().unwrap().kind, LineKind::Mode);
}

#[test]
fn channel_key_limit_and_title() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "@Guiwald");
    h.feed(":Guiwald!alpirc@host.test MODE #lyon +ntkl secret 10");
    let st = h.c.channel_state(w).unwrap();
    assert_eq!(st.key(), Some("secret"));
    assert_eq!(st.limit(), Some(10));
    assert_eq!(h.c.window(w).unwrap().title, "[+ntkl secret 10] #lyon");
    h.feed(":Guiwald!alpirc@host.test MODE #lyon -k+m secret");
    let st = h.c.channel_state(w).unwrap();
    assert_eq!(st.key(), None);
    assert_eq!(st.mode_string(false), "+ntlm");
}

#[test]
fn auto_mode_reply_hidden_user_mode_shown() {
    let mut h = H::new();
    h.up();
    h.feed(":Guiwald!alpirc@host.test JOIN #lyon");
    h.feed(":irc.test.net 324 Guiwald #lyon +nt");
    h.feed(":irc.test.net 329 Guiwald #lyon 1700000000");
    let w = h.win("#lyon");
    assert_eq!(h.lines(w), vec!["* Now talking in #lyon"]);
    assert_eq!(h.c.channel_state(w).unwrap().mode_string(false), "+nt");
    assert_eq!(h.c.channel_state(w).unwrap().created, Some(1_700_000_000));
    h.clear();
    h.input(w, "/mode");
    assert_eq!(h.sends(), vec!["MODE #lyon"]);
    h.feed(":irc.test.net 324 Guiwald #lyon +ntk key");
    assert_eq!(h.last(w), "* #lyon modes: +ntk key");
}

#[test]
fn topic_change() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "Guiwald Pat");
    h.feed(":Pat!p@h TOPIC #lyon :Nouveau sujet");
    assert_eq!(h.last(w), "* Pat changes topic to 'Nouveau sujet'");
    assert_eq!(h.c.channel_state(w).unwrap().topic.as_deref(), Some("Nouveau sujet"));
    assert_eq!(h.c.window(w).unwrap().title, "#lyon: Nouveau sujet");
}

#[test]
fn multi_prefix_names() {
    let mut h = H::new();
    h.up_with_caps("multi-prefix");
    let w = h.join("#lyon", "@+Bob +Carl Guiwald");
    let e = h.c.nicklist(w);
    assert_eq!(e[0].nick, "Bob");
    assert_eq!(e[0].prefixes, "@+");
    h.feed(":X!x@h MODE #lyon -o Bob");
    assert_eq!(h.c.nicklist(w)[0].prefixes, "+");
}

#[test]
fn extended_join_account_and_realname() {
    let mut h = H::new();
    h.up_with_caps("extended-join account-notify");
    let w = h.join("#lyon", "Guiwald");
    h.feed(":Bob!bob@h JOIN #lyon bobacc :Bob Real");
    let e = h.c.nicklist(w).into_iter().find(|e| e.nick == "Bob").unwrap();
    assert_eq!(e.account.as_deref(), Some("bobacc"));
    let u = h.c.session(h.conn).unwrap().user_info("bob").unwrap().clone();
    assert_eq!(u.realname.as_deref(), Some("Bob Real"));
    h.feed(":Bob!bob@h ACCOUNT newacc");
    assert_eq!(h.c.session(h.conn).unwrap().user_info("Bob").unwrap().account.as_deref(), Some("newacc"));
    h.feed(":Bob!bob@h ACCOUNT *");
    assert_eq!(h.c.session(h.conn).unwrap().user_info("Bob").unwrap().account, None);
    h.feed(":Carl!c@h JOIN #lyon * :Carl");
    assert_eq!(h.c.session(h.conn).unwrap().user_info("Carl").unwrap().account, None);
}

#[test]
fn away_notify_updates_nicklist() {
    let mut h = H::new();
    h.up_with_caps("away-notify");
    h.feed(":Guiwald!alpirc@host.test JOIN #lyon");
    // away-notify : WHO à l'arrivée, réponse non affichée.
    assert!(h.sends().contains(&"WHO #lyon".to_string()));
    h.feed(":irc.test.net 353 Guiwald = #lyon :Guiwald Bob");
    h.feed(":irc.test.net 366 Guiwald #lyon :End");
    h.feed(":irc.test.net 352 Guiwald #lyon bob bob.host irc.test.net Bob G :0 Bob Real");
    h.feed(":irc.test.net 315 Guiwald #lyon :End of /WHO list.");
    let w = h.win("#lyon");
    assert_eq!(h.lines(w), vec!["* Now talking in #lyon"]);
    assert!(h.c.nicklist(w).iter().find(|e| e.nick == "Bob").unwrap().away);
    h.feed(":Bob!bob@bob.host AWAY");
    assert!(!h.c.nicklist(w).iter().find(|e| e.nick == "Bob").unwrap().away);
    h.feed(":Bob!bob@bob.host AWAY :lunch");
    let u = h.c.session(h.conn).unwrap().user_info("Bob").unwrap().clone();
    assert!(u.away);
    assert_eq!(u.away_message.as_deref(), Some("lunch"));
    assert_eq!(u.address().as_deref(), Some("bob@bob.host"));
}

#[test]
fn whox_reply_fills_ial() {
    let mut h = H::new();
    h.connect();
    h.register_with("PREFIX=(ov)@+ CHANTYPES=# WHOX");
    h.c.settings_mut().irc.who_on_join = true;
    h.c.settings_changed(h.now);
    h.clear();
    // who_on_join est lu à la connexion : on passe par /who.
    let w = h.join("#lyon", "Guiwald Bob");
    h.input(w, "/who #lyon %tcuhnfar,152");
    h.sends();
    h.feed(":irc.test.net 354 Guiwald 152 #lyon bob h.example Bob H@ bobacc :Bob Real");
    let u = h.c.session(h.conn).unwrap().user_info("Bob").unwrap().clone();
    assert_eq!(u.host.as_deref(), Some("h.example"));
    assert_eq!(u.account.as_deref(), Some("bobacc"));
    assert_eq!(h.nicks(w), vec!["@Bob", "Guiwald"]);
}

#[test]
fn chghost_and_setname() {
    let mut h = H::new();
    h.up_with_caps("chghost setname");
    h.join("#lyon", "Guiwald Bob");
    h.feed(":Bob!bob@old.host PRIVMSG #lyon :x");
    h.feed(":Bob!bob@old.host CHGHOST newuser new.host");
    let u = h.c.session(h.conn).unwrap().user_info("Bob").unwrap().clone();
    assert_eq!(u.address().as_deref(), Some("newuser@new.host"));
    h.feed(":Bob!newuser@new.host SETNAME :Robert Dupont");
    assert_eq!(h.c.session(h.conn).unwrap().user_info("Bob").unwrap().realname.as_deref(), Some("Robert Dupont"));
    h.feed(":Guiwald!alpirc@host.test CHGHOST gui cloak.test");
    assert_eq!(h.c.session(h.conn).unwrap().host(), Some("cloak.test"));
}

#[test]
fn userhost_in_names_fills_ial() {
    let mut h = H::new();
    h.up_with_caps("userhost-in-names");
    let w = h.join("#lyon", "@Bob!bob@bob.host Guiwald!alpirc@host.test");
    assert_eq!(h.nicks(w), vec!["@Bob", "Guiwald"]);
    assert_eq!(h.c.session(h.conn).unwrap().user_info("Bob").unwrap().address().as_deref(), Some("bob@bob.host"));
}

#[test]
fn ban_lists_for_channel_central() {
    let mut h = H::new();
    h.connect();
    h.register_with("PREFIX=(ov)@+ CHANTYPES=# EXCEPTS INVEX");
    h.clear();
    let w = h.join("#lyon", "@Guiwald");
    h.c.request_lists(w);
    assert_eq!(h.sends(), vec!["MODE #lyon +b", "MODE #lyon +e", "MODE #lyon +I"]);
    h.feed(":irc.test.net 367 Guiwald #lyon *!*@bad.host Pat 1700000000");
    h.feed(":irc.test.net 367 Guiwald #lyon *!*@worse.host Pat 1700000001");
    h.feed(":irc.test.net 368 Guiwald #lyon :End of Channel Ban List");
    h.feed(":irc.test.net 349 Guiwald #lyon :End of Channel Exception List");
    h.feed(":irc.test.net 346 Guiwald #lyon *!*@friend Pat 1700000002");
    h.feed(":irc.test.net 347 Guiwald #lyon :End of Channel Invite List");
    let st = h.c.channel_state(w).unwrap();
    assert_eq!(st.bans.len(), 2);
    assert_eq!(st.bans[0].setter.as_deref(), Some("Pat"));
    assert_eq!(st.bans[1].time, Some(1_700_000_001));
    assert!(st.excepts.is_empty());
    assert_eq!(st.invites[0].mask, "*!*@friend");
    assert_eq!(h.lines(w), vec!["* Now talking in #lyon"], "listes demandées sans affichage");
    // Les changements de mode tiennent la liste à jour.
    h.feed(":Guiwald!alpirc@host.test MODE #lyon -b+b *!*@bad.host *!*@new.host");
    let masks: Vec<String> = h.c.channel_state(w).unwrap().bans.iter().map(|b| b.mask.clone()).collect();
    assert_eq!(masks, vec!["*!*@worse.host", "*!*@new.host"]);
}

#[test]
fn ban_list_shown_when_typed() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "@Guiwald");
    h.input(w, "/mode #lyon +b");
    h.feed(":irc.test.net 367 Guiwald #lyon *!*@bad.host Pat 1700000000");
    h.feed(":irc.test.net 368 Guiwald #lyon :End of Channel Ban List");
    let l = h.lines(w);
    assert_eq!(l[l.len() - 2], format!("#lyon *!*@bad.host set by Pat on {}", timeutil::asctime(1_700_000_000_000, 0)));
    assert_eq!(l[l.len() - 1], "#lyon End of Channel Ban List");
}

#[test]
fn invite_received() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "Guiwald");
    h.feed(":Pat!p@p.host INVITE Guiwald :#secret");
    assert_eq!(h.last(w), "* Pat (p@p.host) invites you to join #secret");
    assert_eq!(h.c.window(w).unwrap().lines.back().unwrap().kind, LineKind::Invite);
    assert!(h.sends().is_empty());
}

#[test]
fn join_on_invite_option() {
    let mut s = settings();
    s.irc.join_on_invite = true;
    let mut h = H::with(s);
    h.up();
    h.feed(":Pat!p@p.host INVITE Guiwald :#secret");
    assert_eq!(h.sends(), vec!["JOIN #secret"]);
}

#[test]
fn invite_notify_for_others() {
    let mut h = H::new();
    h.up_with_caps("invite-notify");
    let w = h.join("#lyon", "@Pat Guiwald");
    h.feed(":Pat!p@h INVITE Bob #lyon");
    assert_eq!(h.last(w), "* Pat invited Bob into #lyon");
}

#[test]
fn channel_state_only_for_channels() {
    let mut h = H::new();
    h.up();
    assert!(h.c.channel_state(h.status).is_none());
}

#[test]
fn event_activity_on_inactive_channel() {
    let mut h = H::new();
    h.up();
    let a = h.join("#a", "Guiwald");
    let b = h.join("#b", "Guiwald");
    assert_eq!(h.c.active(), Some(b));
    h.feed(":Bob!b@h JOIN #a");
    assert_eq!(h.c.window(a).unwrap().activity, Activity::Event);
    h.feed(":Bob!b@h PRIVMSG #a :hello");
    assert_eq!(h.c.window(a).unwrap().activity, Activity::Message);
    h.feed(":Carl!c@h JOIN #a");
    assert_eq!(h.c.window(a).unwrap().activity, Activity::Message, "un événement ne fait pas redescendre");
    assert_eq!(h.c.window(b).unwrap().activity, Activity::None);
}

#[test]
fn version_bumps_on_nicklist_change() {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "Guiwald Bob");
    let v = h.c.window(w).unwrap().version;
    h.feed(":Bob!b@h AWAY :x");
    assert!(h.c.window(w).unwrap().version > v);
}
