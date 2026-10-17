//! DCC dans le moteur : offres reçues (actives, passives, reprise), nos envois et discussions, réglages
//! d'acceptation, bac à sable des scripts, événements (FILESENT, FILERCVD, SENDFAIL, GETFAIL, CHAT, OPEN),
//! délais, commandes `/dcc`.

mod common;

use std::net::IpAddr;
use std::path::PathBuf;

use alpirc_core::dcc::DccIo;
use alpirc_core::settings::{DccChatAction, DccExists, DccSendAction};
use alpirc_core::{DccAnswer, DccId, DccType, DccUi, Effect, Settings, TransferState, WinKind};
use alpirc_script::ScriptKind;
use common::*;

/// Réglages des essais : dossier de réception à part.
fn dcc_settings(dir: &std::path::Path) -> Settings {
    let mut s = settings();
    s.dcc.folder = Some(dir.join("downloads"));
    s
}

/// Client connecté, adresse locale 192.168.1.10, dossier de réception vide.
fn up_with(f: impl FnOnce(&mut Settings)) -> H {
    let dir = tmpdir();
    let mut s = dcc_settings(&dir);
    f(&mut s);
    let mut h = H::with_dir(s, dir);
    h.up();
    h.c.set_local_ip(h.conn, "192.168.1.10".parse().unwrap());
    h
}

fn up() -> H {
    up_with(|_| {})
}

fn ctcp(from: &str, body: &str) -> String {
    format!(":{from}!u@host.example PRIVMSG Guiwald :\x01DCC {body}\x01")
}

fn prompts(fx: &[Effect]) -> Vec<alpirc_core::DccPrompt> {
    fx.iter().filter_map(|e| if let Effect::DccPrompt(p) = e { Some(p.clone()) } else { None }).collect()
}

fn listens(fx: &[Effect]) -> Vec<(DccId, DccIo)> {
    fx.iter().filter_map(|e| if let Effect::DccListen { id, io, .. } = e { Some((*id, io.clone())) } else { None }).collect()
}

fn connects(fx: &[Effect]) -> Vec<(DccId, IpAddr, u16, DccIo)> {
    fx.iter()
        .filter_map(|e| if let Effect::DccConnect { id, ip, port, io, .. } = e { Some((*id, *ip, *port, io.clone())) } else { None })
        .collect()
}

fn script(h: &mut H, text: &str) {
    h.c.load_script_text("dcc-test.mrc", ScriptKind::Remote, text, h.now).unwrap();
    h.clear();
}

fn downloads(h: &H) -> PathBuf {
    h.c.dcc_folder()
}

#[test]
fn active_offer_prompt_accept_receive() {
    let mut h = up();
    script(&mut h, "on *:FILERCVD:*.zip:echo -s got $nick $nopath($filename)");
    h.feed(&ctcp("Pat", "SEND \"my file.zip\" 3232235777 5000 2048"));
    assert!(h.status_lines().iter().any(|l| l == "* DCC SEND offer from Pat: my file.zip (2.0KB)"));
    let fx = h.effects();
    let p = prompts(&fx);
    assert_eq!(p.len(), 1);
    assert_eq!(
        (p[0].nick.as_str(), p[0].file.as_str(), p[0].size, p[0].chat, p[0].dangerous),
        ("Pat", "my file.zip", Some(2048), false, false)
    );
    assert!(connects(&fx).is_empty(), "rien avant l'accord");
    let id = p[0].id;
    let t = h.c.dcc_entry(id).unwrap();
    assert_eq!((t.kind, t.state, t.incoming), (DccType::Get, TransferState::Offered, true));
    h.c.dcc_accept(id, DccAnswer::Accept, h.now);
    let fx = h.effects();
    let c = connects(&fx);
    assert_eq!(c.len(), 1);
    let path = downloads(&h).join("my file.zip");
    assert_eq!(c[0], (id, "192.168.1.1".parse().unwrap(), 5000, DccIo::Receive { path: path.clone(), offset: 0, size: Some(2048) }));
    assert!(downloads(&h).is_dir(), "dossier créé");
    h.c.on_dcc_connected(id, h.now);
    assert_eq!(h.c.dcc_entry(id).unwrap().state, TransferState::Active);
    h.now += 1000;
    h.c.on_dcc_progress(id, 1024, h.now);
    let t = h.c.dcc_entry(id).unwrap();
    assert_eq!((t.pos, t.speed, t.percent()), (1024, 1024, Some(50)));
    h.now += 1000;
    h.c.on_dcc_ended(id, 2048, None, h.now);
    let t = h.c.dcc_entry(id).unwrap();
    assert_eq!(t.state, TransferState::Done);
    let lines = h.status_lines();
    assert!(lines.iter().any(|l| l.starts_with("* DCC Get of my file.zip from Pat completed (2.0KB, 1.0KB/s)")), "{lines:?}");
    assert!(lines.iter().any(|l| l == "got Pat my file.zip"), "{lines:?}");
    assert_eq!(h.c.transfers().len(), 1);
}

#[test]
fn passive_offer_listens_and_replies_with_token() {
    let mut h = up();
    h.feed(&ctcp("Pat", "SEND photo.jpg 3232235777 0 9000 777"));
    let id = prompts(&h.effects())[0].id;
    assert!(h.c.dcc_entry(id).unwrap().passive);
    h.c.dcc_accept(id, DccAnswer::Accept, h.now);
    let fx = h.effects();
    let l = listens(&fx);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].1, DccIo::Receive { path: downloads(&h).join("photo.jpg"), offset: 0, size: Some(9000) });
    assert!(h.sends().is_empty(), "la réponse attend le port");
    h.c.on_dcc_listening(id, 40000, h.now);
    assert_eq!(h.sends(), vec!["PRIVMSG Pat :\x01DCC SEND photo.jpg 3232235786 40000 9000 777\x01"]);
    assert_eq!(h.c.dcc_entry(id).unwrap().state, TransferState::Waiting);
}

#[test]
fn our_passive_send_and_reply() {
    let mut h = up();
    script(&mut h, "on *:FILESENT:*:echo -s sent $nopath($filename) to $nick");
    let src = h.dir.join("a b.txt");
    std::fs::write(&src, vec![b'x'; 3000]).unwrap();
    h.input(h.status, &format!("/dcc send Pat {}", src.display()));
    let sends = h.sends();
    assert_eq!(sends.len(), 1);
    let (head, token) = sends[0].trim_end_matches('\x01').rsplit_once(' ').unwrap();
    assert_eq!(head, "PRIVMSG Pat :\x01DCC SEND \"a b.txt\" 3232235786 0 3000");
    assert!(token.parse::<u64>().is_ok());
    assert!(listens(&h.effects()).is_empty(), "passif : pas d'écoute chez nous");
    let id = h.c.transfers()[0].id;
    // L'autre répond avec son adresse, son port et le même jeton.
    h.feed(&ctcp("Pat", &format!("SEND \"a b.txt\" 2130706433 6000 3000 {token}")));
    let c = connects(&h.effects());
    assert_eq!(c, vec![(id, "127.0.0.1".parse().unwrap(), 6000, DccIo::Send { path: src.clone(), offset: 0, size: 3000, packet: 16384 })]);
    assert!(!h.status_lines().iter().any(|l| l.contains("offer from Pat")), "une réponse n'est pas une offre");
    h.c.on_dcc_connected(id, h.now);
    h.c.on_dcc_ended(id, 3000, None, h.now + 10);
    assert!(h.status_lines().iter().any(|l| l == "sent a b.txt to Pat"));
    assert_eq!(h.c.dcc_entry(id).unwrap().state, TransferState::Done);
}

#[test]
fn our_active_send_resume_accept() {
    let mut h = up_with(|s| s.dcc.passive = false);
    let src = h.dir.join("big.bin");
    std::fs::write(&src, vec![0u8; 10_000]).unwrap();
    h.input(h.status, &format!("/dcc send Pat {}", src.display()));
    let fx = h.effects();
    let l = listens(&fx);
    assert_eq!(l.len(), 1);
    let id = l[0].0;
    h.c.on_dcc_listening(id, 5050, h.now);
    assert_eq!(h.sends(), vec!["PRIVMSG Pat :\x01DCC SEND big.bin 3232235786 5050 10000\x01"]);
    // Reprise demandée sur notre port.
    h.feed(&ctcp("Pat", "RESUME big.bin 5050 4000"));
    assert!(h.effects().contains(&Effect::DccResume { id, offset: 4000 }));
    assert_eq!(h.sends(), vec!["PRIVMSG Pat :\x01DCC ACCEPT big.bin 5050 4000\x01"]);
    let t = h.c.dcc_entry(id).unwrap();
    assert_eq!((t.start, t.pos), (4000, 4000));
    // Mauvais port : ignoré.
    h.feed(&ctcp("Pat", "RESUME big.bin 5051 100"));
    assert!(h.sends().is_empty());
}

#[test]
fn resume_partial_file_then_accept() {
    let mut h = up();
    std::fs::create_dir_all(downloads(&h)).unwrap();
    std::fs::write(downloads(&h).join("film.mkv"), vec![1u8; 1500]).unwrap();
    h.feed(&ctcp("Pat", "SEND film.mkv 3232235777 5000 4000"));
    let p = prompts(&h.effects()).remove(0);
    assert_eq!(p.existing, Some(1500));
    assert!(p.can_resume());
    h.c.dcc_accept(p.id, DccAnswer::Resume, h.now);
    assert_eq!(h.sends(), vec!["PRIVMSG Pat :\x01DCC RESUME film.mkv 5000 1500\x01"]);
    assert!(connects(&h.effects()).is_empty(), "on attend ACCEPT");
    h.feed(&ctcp("Pat", "ACCEPT film.mkv 5000 1500"));
    let c = connects(&h.effects());
    assert_eq!(c[0].3, DccIo::Receive { path: downloads(&h).join("film.mkv"), offset: 1500, size: Some(4000) });
    assert_eq!(h.c.dcc_entry(p.id).unwrap().start, 1500);
}

#[test]
fn passive_resume_uses_token() {
    let mut h = up();
    std::fs::create_dir_all(downloads(&h)).unwrap();
    std::fs::write(downloads(&h).join("x.iso"), vec![1u8; 100]).unwrap();
    h.feed(&ctcp("Pat", "SEND x.iso 3232235777 0 400 55"));
    let id = prompts(&h.effects())[0].id;
    h.c.dcc_accept(id, DccAnswer::Resume, h.now);
    assert_eq!(h.sends(), vec!["PRIVMSG Pat :\x01DCC RESUME x.iso 0 100 55\x01"]);
    h.feed(&ctcp("Pat", "ACCEPT x.iso 0 100 99"));
    assert!(listens(&h.effects()).is_empty(), "mauvais jeton");
    h.feed(&ctcp("Pat", "ACCEPT x.iso 0 100 55"));
    let l = listens(&h.effects());
    assert_eq!(l[0].1, DccIo::Receive { path: downloads(&h).join("x.iso"), offset: 100, size: Some(400) });
}

#[test]
fn rename_and_overwrite() {
    let mut h = up();
    std::fs::create_dir_all(downloads(&h)).unwrap();
    std::fs::write(downloads(&h).join("a.txt"), b"old").unwrap();
    h.feed(&ctcp("Pat", "SEND a.txt 3232235777 5000 10"));
    let id = prompts(&h.effects())[0].id;
    h.c.dcc_accept(id, DccAnswer::Rename, h.now);
    assert_eq!(connects(&h.effects())[0].3, DccIo::Receive { path: downloads(&h).join("a (1).txt"), offset: 0, size: Some(10) });
    assert_eq!(h.c.dcc_entry(id).unwrap().file, "a (1).txt");
    h.feed(&ctcp("Bob", "SEND a.txt 3232235777 5001 10"));
    let id = prompts(&h.effects())[0].id;
    h.c.dcc_accept(id, DccAnswer::Overwrite, h.now);
    assert_eq!(connects(&h.effects())[0].3, DccIo::Receive { path: downloads(&h).join("a.txt"), offset: 0, size: Some(10) });
}

#[test]
fn names_are_sanitized() {
    let mut h = up();
    h.feed(&ctcp("Pat", "SEND ../../.bashrc 3232235777 5000 10"));
    let p = prompts(&h.effects()).remove(0);
    assert_eq!(p.file, "bashrc");
    assert_eq!(p.path, Some(downloads(&h).join("bashrc")));
    h.feed(&ctcp("Pat", "SEND \"C:\\Windows\\evil.exe\" 3232235777 5000 10"));
    let p = prompts(&h.effects()).remove(0);
    assert_eq!(p.file, "evil.exe");
    assert!(p.dangerous);
}

#[test]
fn ipv6_offer() {
    let mut h = up();
    h.feed(&ctcp("Pat", "SEND f.txt 2001:db8::7 5000 10"));
    let id = prompts(&h.effects())[0].id;
    h.c.dcc_accept(id, DccAnswer::Accept, h.now);
    assert_eq!(connects(&h.effects())[0].1, "2001:db8::7".parse::<IpAddr>().unwrap());
}

#[test]
fn auto_get_never_for_executables() {
    let mut h = up_with(|s| s.dcc.on_send = DccSendAction::AutoGet);
    h.feed(&ctcp("Pat", "SEND song.mp3 3232235777 5000 10"));
    let fx = h.effects();
    assert!(prompts(&fx).is_empty());
    assert_eq!(connects(&fx).len(), 1, "accepté tout seul");
    for bad in
        ["setup.exe", "run.bat", "x.cmd", "y.scr", "z.vbs", "a.js", "Mac.app", "i.sh", "p.pkg", "d.dmg", "a.apk", "s.command", "evil.exe."]
    {
        h.feed(&ctcp("Pat", &format!("SEND {bad} 3232235777 5000 10")));
        let fx = h.effects();
        assert!(connects(&fx).is_empty(), "{bad} accepté tout seul");
        assert_eq!(prompts(&fx).len(), 1, "{bad} : question attendue");
    }
    assert!(h.status_lines().iter().any(|l| l.contains("never accepted automatically")));
}

#[test]
fn auto_get_existing_file_follows_setting() {
    let mut h = up_with(|s| {
        s.dcc.on_send = DccSendAction::AutoGet;
        s.dcc.on_exists = DccExists::Resume;
    });
    std::fs::create_dir_all(downloads(&h)).unwrap();
    std::fs::write(downloads(&h).join("p.zip"), vec![0u8; 50]).unwrap();
    h.feed(&ctcp("Pat", "SEND p.zip 3232235777 5000 100"));
    assert!(prompts(&h.effects()).is_empty());
    assert_eq!(h.sends(), vec!["PRIVMSG Pat :\x01DCC RESUME p.zip 5000 50\x01"]);
    // Réglage « demander » : la question revient.
    h.c.settings_mut().dcc.on_exists = DccExists::Ask;
    h.feed(&ctcp("Bob", "SEND p.zip 3232235777 5001 100"));
    assert_eq!(prompts(&h.effects()).len(), 1);
}

#[test]
fn ignore_setting_and_ignore_list() {
    let mut h = up_with(|s| s.dcc.on_send = DccSendAction::Ignore);
    h.feed(&ctcp("Pat", "SEND a.zip 3232235777 5000 10"));
    let fx = h.effects();
    assert!(prompts(&fx).is_empty() && connects(&fx).is_empty());
    assert_eq!(h.c.transfers()[0].state, TransferState::Cancelled);
    assert!(h.status_lines().iter().any(|l| l == "* DCC: offer from Pat ignored (a.zip)"));
    // Liste Ignore, type DCC : rien du tout.
    let mut h = up();
    h.input(h.status, "/ignore -d Pat");
    h.clear();
    h.feed(&ctcp("Pat", "SEND a.zip 3232235777 5000 10"));
    assert!(prompts(&h.effects()).is_empty());
    assert!(h.c.transfers().is_empty());
}

#[test]
fn scripts_cannot_accept_or_send_files() {
    let mut h = up();
    // Un script qui voudrait tout accepter.
    script(&mut h, "ctcp *:DCC SEND*:?:dcc get $nick\non *:CHAT:*:halt");
    h.feed(&ctcp("Pat", "SEND a.zip 3232235777 5000 10"));
    let fx = h.effects();
    assert!(connects(&fx).is_empty(), "un script ne doit pas accepter");
    assert_eq!(prompts(&fx).len(), 1, "l'utilisateur est toujours questionné");
    assert!(h.status_lines().iter().any(|l| l.contains("not allowed from a script")), "{:?}", h.status_lines());
    // Un timer ne vaut pas mieux.
    script(&mut h, "alias later .timer 1 1 dcc get Pat");
    h.input(h.status, "/later");
    h.advance(1500);
    assert!(connects(&h.effects()).is_empty());
    // Ni envoyer un fichier qu'il choisit.
    let secret = h.dir.join("secret.txt");
    std::fs::write(&secret, b"x").unwrap();
    script(&mut h, &format!("on *:TEXT:!steal:?:dcc send $nick {}", secret.display()));
    h.feed(":Pat!u@h PRIVMSG Guiwald :!steal");
    assert!(h.sends().iter().all(|l| !l.contains("DCC SEND")));
    // L'utilisateur, lui, peut.
    h.input(h.status, "/dcc get Pat");
    assert_eq!(connects(&h.effects()).len(), 1);
}

#[test]
fn script_halt_drops_offer() {
    let mut h = up();
    script(&mut h, "ctcp *:DCC*:*:halt");
    h.feed(&ctcp("Pat", "SEND a.zip 3232235777 5000 10"));
    assert!(prompts(&h.effects()).is_empty());
    assert!(h.c.transfers().is_empty());
}

#[test]
fn chat_offer_accept_lines_events() {
    let mut h = up();
    script(&mut h, "on *:OPEN:=:*:echo -s open $target\non *:CHAT:*:echo -s chat $nick $1-");
    h.feed(&ctcp("Pat", "CHAT chat 2130706433 4000"));
    assert!(h.status_lines().iter().any(|l| l == "* DCC CHAT offer from Pat"));
    let p = prompts(&h.effects()).remove(0);
    assert!(p.chat);
    h.c.dcc_accept(p.id, DccAnswer::Accept, h.now);
    let c = connects(&h.effects());
    assert_eq!(c, vec![(p.id, "127.0.0.1".parse().unwrap(), 4000, DccIo::Chat)]);
    let w = h.win("=Pat");
    assert_eq!(h.c.window(w).unwrap().kind, WinKind::DccChat);
    assert!(h.status_lines().iter().any(|l| l == "open =Pat"));
    h.c.on_dcc_connected(p.id, h.now);
    assert!(h.has_line(w, "* DCC Chat connection established with Pat"));
    h.c.on_dcc_chat_line(p.id, "salut à toi".as_bytes(), h.now);
    assert!(h.has_line(w, "<Pat> salut à toi"));
    assert!(h.status_lines().iter().any(|l| l == "chat Pat salut à toi"));
    h.c.on_dcc_chat_line(p.id, b"\x01ACTION waves\x01", h.now);
    assert_eq!(h.last(w), "* Pat waves");
    // Latin-1 accepté.
    h.c.on_dcc_chat_line(p.id, b"caf\xe9", h.now);
    assert_eq!(h.last(w), "<Pat> café");
    // Ce qu'on tape part dans la discussion.
    h.clear();
    h.input(w, "bonjour");
    assert!(h.effects().contains(&Effect::DccChatSend { id: p.id, line: "bonjour".into() }));
    assert_eq!(h.last(w), "<Guiwald> bonjour");
    h.input(w, "/me danse");
    assert!(h.effects().contains(&Effect::DccChatSend { id: p.id, line: "\x01ACTION danse\x01".into() }));
    assert_eq!(h.last(w), "* Guiwald danse");
    h.input(h.status, "/msg =Pat par msg");
    assert!(h.effects().contains(&Effect::DccChatSend { id: p.id, line: "par msg".into() }));
    assert!(h.sends().is_empty(), "rien par le serveur");
    // Fermer la fenêtre ferme la discussion.
    h.c.close_window(w, h.now);
    assert!(h.effects().contains(&Effect::DccClose { id: p.id }));
    assert_eq!(h.c.dcc_entry(p.id).unwrap().state, TransferState::Cancelled);
    h.c.on_dcc_ended(p.id, 0, None, h.now);
}

#[test]
fn chat_closed_by_peer() {
    let mut h = up_with(|s| s.dcc.on_chat = DccChatAction::AutoAccept);
    h.feed(&ctcp("Pat", "CHAT chat 2130706433 4000"));
    let id = connects(&h.effects())[0].0;
    let w = h.win("=Pat");
    h.c.on_dcc_connected(id, h.now);
    h.c.on_dcc_ended(id, 0, None, h.now);
    assert_eq!(h.last(w), "* DCC Chat session with Pat closed");
    h.input(w, "encore là ?");
    assert_eq!(h.last(w), "* DCC Chat with Pat is not connected");
    assert!(!h.effects().iter().any(|e| matches!(e, Effect::DccChatSend { .. })));
}

#[test]
fn our_chat_active_and_passive() {
    let mut h = up();
    h.input(h.status, "/dcc chat Pat");
    let l = listens(&h.effects());
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].1, DccIo::Chat);
    let w = h.win("=Pat");
    h.c.on_dcc_listening(l[0].0, 4444, h.now);
    assert_eq!(h.sends(), vec!["PRIVMSG Pat :\x01DCC CHAT chat 3232235786 4444\x01"]);
    assert!(h.has_line(w, "* DCC Chat: waiting for Pat to accept…"));
    // Passive : jeton, puis on se connecte à la réponse.
    let mut h = up_with(|s| s.dcc.passive_chat = true);
    h.input(h.status, "/chat Bob");
    let s = h.sends();
    let token = s[0].trim_end_matches('\x01').rsplit(' ').next().unwrap().to_string();
    assert!(s[0].starts_with("PRIVMSG Bob :\x01DCC CHAT chat 3232235786 0 "), "{s:?}");
    h.feed(&ctcp("Bob", &format!("CHAT chat 2130706433 5555 {token}")));
    assert_eq!(connects(&h.effects())[0].2, 5555);
    assert!(prompts(&h.effects()).is_empty());
}

#[test]
fn incoming_passive_chat() {
    let mut h = up();
    h.feed(&ctcp("Pat", "CHAT chat 0 0 4242"));
    let id = prompts(&h.effects())[0].id;
    // `/dcc chat Pat` tapé : accepte son offre.
    h.input(h.status, "/dcc chat Pat");
    assert_eq!(listens(&h.effects())[0], (id, DccIo::Chat));
    h.c.on_dcc_listening(id, 6000, h.now);
    assert_eq!(h.sends(), vec!["PRIVMSG Pat :\x01DCC CHAT chat 3232235786 6000 4242\x01"]);
}

#[test]
fn offers_expire_and_waits_fail() {
    let mut h = up_with(|s| s.dcc.timeout_secs = 30);
    script(&mut h, "on *:SENDFAIL:*:echo -s fail $nick $nopath($filename)");
    h.feed(&ctcp("Pat", "SEND a.zip 3232235777 5000 10"));
    let id = prompts(&h.effects())[0].id;
    let src = h.dir.join("b.txt");
    std::fs::write(&src, b"hello").unwrap();
    h.input(h.status, &format!("/dcc send Bob {}", src.display()));
    let next = h.advance(1000).unwrap();
    assert!(next <= h.now + 30_000);
    h.advance(30_000);
    assert_eq!(h.c.dcc_entry(id).unwrap().state, TransferState::Cancelled);
    let sent = h.c.transfers().into_iter().find(|t| t.kind == DccType::Send).unwrap();
    assert_eq!(sent.state, TransferState::Failed);
    let lines = h.status_lines();
    assert!(lines.iter().any(|l| l == "* DCC: offer from Pat expired (a.zip)"), "{lines:?}");
    assert!(lines.iter().any(|l| l == "fail Bob b.txt"), "{lines:?}");
    // Accepter trop tard : rien.
    h.c.dcc_accept(id, DccAnswer::Accept, h.now);
    assert!(connects(&h.effects()).is_empty());
}

#[test]
fn close_and_list_commands() {
    let mut h = up();
    script(&mut h, "on *:GETFAIL:*:echo -s getfail $nick");
    h.feed(&ctcp("Pat", "SEND a.zip 3232235777 5000 100"));
    let id = prompts(&h.effects())[0].id;
    h.c.dcc_accept(id, DccAnswer::Accept, h.now);
    h.c.on_dcc_connected(id, h.now);
    h.c.on_dcc_progress(id, 25, h.now + 500);
    h.clear();
    h.input(h.status, "/dcc list");
    let lines = h.status_lines();
    assert!(lines.iter().any(|l| l.contains("Get Pat a.zip 25B/100B 25%")), "{lines:?}");
    h.input(h.status, "/dcc close get Pat");
    assert!(h.effects().contains(&Effect::DccClose { id }));
    assert_eq!(h.c.dcc_entry(id).unwrap().state, TransferState::Cancelled);
    assert!(h.status_lines().iter().any(|l| l == "getfail Pat"));
    // La fin annoncée ensuite par l'hôte est ignorée.
    h.c.on_dcc_ended(id, 25, Some("annulé"), h.now);
    assert_eq!(h.c.dcc_entry(id).unwrap().state, TransferState::Cancelled);
    h.input(h.status, "/dcc close send Nobody");
    assert!(h.last(h.status).contains("no such DCC send with Nobody"));
    h.c.dcc_clear_finished();
    assert!(h.c.transfers().is_empty());
}

#[test]
fn incomplete_get_fails() {
    let mut h = up();
    h.feed(&ctcp("Pat", "SEND a.zip 3232235777 5000 100"));
    let id = prompts(&h.effects())[0].id;
    h.c.dcc_accept(id, DccAnswer::Accept, h.now);
    h.c.on_dcc_connected(id, h.now);
    h.c.on_dcc_ended(id, 60, None, h.now);
    let t = h.c.dcc_entry(id).unwrap();
    assert_eq!(t.state, TransferState::Failed);
    assert!(t.error.unwrap().contains("60"));
}

#[test]
fn send_without_file_asks_ui_and_errors() {
    let mut h = up();
    h.input(h.status, "/dcc send Pat");
    assert!(h.effects().contains(&Effect::DccPickFile { conn: h.conn, nick: "Pat".into() }));
    h.input(h.status, "/dcc send");
    assert!(h.effects().contains(&Effect::DccWindow(DccUi::SendDialog)));
    h.input(h.status, "/dcc");
    assert!(h.effects().contains(&Effect::DccWindow(DccUi::Transfers)));
    h.input(h.status, "/dcc send Pat /nonexistent/file.bin");
    assert_eq!(h.last(h.status), "* /dcc send: no such file: /nonexistent/file.bin");
    h.input(h.status, "/dcc send Pat /nonexistent/my file.bin");
    assert_eq!(h.last(h.status), "* /dcc send: no such file: /nonexistent/my file.bin");
    // Choisi dans l'interface.
    let src = h.dir.join("c.txt");
    std::fs::write(&src, b"abc").unwrap();
    let id = h.c.dcc_send_file(h.conn, "Pat", &src, h.now).unwrap();
    assert_eq!(h.c.dcc_entry(id).unwrap().size, Some(3));
    assert!(h.sends()[0].starts_with("PRIVMSG Pat :\x01DCC SEND c.txt 3232235786 0 3 "));
    // Renvoi après échec.
    h.c.on_dcc_ended(id, 0, Some("connexion refusée"), h.now);
    h.c.dcc_resend(id, h.now).unwrap();
    assert_eq!(h.c.transfers().len(), 2);
}

#[test]
fn own_ip_setting_and_public_server_ip() {
    let mut h = up_with(|s| {
        s.dcc.passive = false;
        s.dcc.own_ip = Some("203.0.113.5".into());
    });
    let src = h.dir.join("d.txt");
    std::fs::write(&src, b"abc").unwrap();
    let id = h.c.dcc_send_file(h.conn, "Pat", &src, h.now).unwrap();
    h.c.on_dcc_listening(id, 7000, h.now);
    assert_eq!(
        h.sends(),
        vec![format!("PRIVMSG Pat :\x01DCC SEND d.txt {} 7000 3\x01", u32::from("203.0.113.5".parse::<std::net::Ipv4Addr>().unwrap()))]
    );
}

#[test]
fn reject_offer() {
    let mut h = up();
    h.feed(&ctcp("Pat", "SEND a.zip 3232235777 5000 10"));
    let id = prompts(&h.effects())[0].id;
    assert!(h.c.dcc_prompt(id).is_some());
    h.c.dcc_reject(id, h.now);
    assert!(h.c.dcc_prompt(id).is_none());
    assert_eq!(h.c.dcc_entry(id).unwrap().state, TransferState::Cancelled);
    assert!(h.status_lines().iter().any(|l| l == "* DCC: offer from Pat refused (a.zip)"));
}

#[test]
fn settings_round_trip() {
    let mut s = Settings::default();
    assert_eq!(s.dcc.on_send, DccSendAction::Ask);
    assert!(s.dcc.passive);
    s.dcc.on_send = DccSendAction::AutoGet;
    s.dcc.on_exists = DccExists::Rename;
    s.dcc.port_min = 5000;
    s.dcc.port_max = 5010;
    let t = s.to_toml().unwrap();
    assert!(t.contains("on_send = \"auto-get\""), "{t}");
    let back = Settings::from_toml(&t).unwrap();
    assert_eq!(back.dcc, s.dcc);
    // Un ancien fichier sans section [dcc] garde les valeurs par défaut.
    let old = Settings::from_toml("version = 1\n").unwrap();
    assert_eq!(old.dcc, Settings::default().dcc);
}
