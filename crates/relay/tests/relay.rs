//! Essais d'intégration du relais, dans le processus : un faux serveur IRC, le relais, et deux appareils
//! (clients ligne à ligne en TLS épinglé) liés au même réseau.

mod common;

use std::time::Duration;

use alpirc_proto::Message;
use common::*;

const CAPS_A: &[&str] = &[
    "server-time",
    "message-tags",
    "echo-message",
    "batch",
    "draft/chathistory",
    "draft/read-marker",
    "soju.im/bouncer-networks",
    "extended-join",
    "multi-prefix",
];
const CAPS_B: &[&str] =
    &["server-time", "message-tags", "batch", "draft/chathistory", "draft/read-marker", "soju.im/bouncer-networks"];

fn joined_burst(lines: &[Message], c: &mut Raw) {
    let _ = lines;
    c.expect("JOIN #test", |m| m.command == "JOIN" && m.param(0) == "#test" && m.nick() == "Gui");
    c.expect("366 #test", |m| m.command == "366" && m.param(1) == "#test");
}

#[test]
fn two_devices_share_one_session() {
    let env = Env::start("two");
    let mut bob = env.bob("Bob");

    // Appareil A : lié par le nom du réseau dans le mot de passe.
    let mut a = env.client();
    let reg = a.register("gui/Fake@mac:secret", "whatever", CAPS_A, None);
    let welcome = reg.iter().find(|m| m.command == "001").expect("001");
    assert_eq!(welcome.param(0), "Gui", "le pseudo du réseau, pas celui demandé");
    assert!(welcome.trailing().contains("Gui!gui@127.0.0.1"), "{}", welcome.to_line());
    let isupport: Vec<String> = reg.iter().filter(|m| m.command == "005").flat_map(|m| m.params.clone()).collect();
    assert!(isupport.contains(&"BOUNCER_NETID=1".to_string()), "{isupport:?}");
    assert!(isupport.contains(&"CHATHISTORY=1000".to_string()));
    assert!(isupport.contains(&"NETWORK=FakeNet".to_string()));
    let join = a.expect("JOIN", |m| m.command == "JOIN");
    assert_eq!(join.params, vec!["#test", "*", "Guillaume"], "JOIN étendu");
    let names = a.expect_cmd("353");
    assert!(names.trailing().contains("Bob") && names.trailing().contains("@Gui"), "{}", names.to_line());
    a.expect("MARKREAD avant 366", |m| m.command == "MARKREAD" && m.param(0) == "#test" && m.param(1) == "*");
    a.expect_cmd("366");

    // Appareil B : sans echo-message, lié par BOUNCER BIND.
    let mut b = env.client();
    let reg_b = b.register("gui:secret", "Gui", CAPS_B, Some("1"));
    joined_burst(&reg_b, &mut b);

    // Message du réseau : les deux appareils le reçoivent, avec msgid et heure.
    bob.send("PRIVMSG #test :bonjour à tous");
    let ma = a.expect("PRIVMSG de Bob (A)", |m| is_privmsg(m, "#test", "bonjour à tous"));
    let mb = b.expect("PRIVMSG de Bob (B)", |m| is_privmsg(m, "#test", "bonjour à tous"));
    assert_eq!(ma.nick(), "Bob");
    let id = ma.tags.get("msgid").expect("msgid").to_string();
    assert_eq!(mb.tags.get("msgid"), Some(id.as_str()), "même msgid sur les deux appareils");
    assert!(ma.tags.get("time").is_some());
    let hello_time = ma.tags.get("time").unwrap().to_string();

    // A écrit : le réseau le reçoit une fois, B le voit venir de nous, A a son écho.
    a.send("@+draft/reply=x PRIVMSG #test :écrit sur le Mac");
    let echo_a = a.expect("écho pour A", |m| is_privmsg(m, "#test", "écrit sur le Mac"));
    assert_eq!(echo_a.nick(), "Gui");
    assert_eq!(echo_a.tags.get("+draft/reply"), Some("x"), "étiquette du client gardée");
    let on_b = b.expect("message de A sur B", |m| is_privmsg(m, "#test", "écrit sur le Mac"));
    assert_eq!(on_b.source.as_ref().unwrap().raw, "Gui!gui@127.0.0.1");
    assert_eq!(on_b.tags.get("msgid"), echo_a.tags.get("msgid"));
    bob.expect("message de A chez Bob", |m| is_privmsg(m, "#test", "écrit sur le Mac"));
    std::thread::sleep(Duration::from_millis(200));
    let sent: Vec<String> = env.ircd.lines_from("Gui").into_iter().filter(|l| l.contains("écrit sur le Mac")).collect();
    assert_eq!(sent, vec!["PRIVMSG #test :écrit sur le Mac"], "une seule fois vers le réseau");

    // B écrit (sans echo-message) : A le voit, B ne reçoit pas d'écho.
    b.send("PRIVMSG #test :écrit sur le téléphone");
    a.expect("message de B sur A", |m| is_privmsg(m, "#test", "écrit sur le téléphone") && m.nick() == "Gui");
    let before_pong = b.until_pong("verif");
    assert!(!before_pong.iter().any(|m| m.command == "PRIVMSG"), "pas d'écho pour B : {before_pong:?}");
    bob.expect("message de B chez Bob", |m| is_privmsg(m, "#test", "écrit sur le téléphone"));

    // Privé : écrit sur A, vu sur B comme venant de nous, adressé à Bob.
    a.send("PRIVMSG Bob :en privé");
    let p = b.expect("privé de A sur B", |m| is_privmsg(m, "Bob", "en privé"));
    assert_eq!(p.nick(), "Gui");
    bob.expect("privé chez Bob", |m| is_privmsg(m, "Bob", "en privé"));
    bob.send("PRIVMSG Gui :réponse privée");
    a.expect("réponse privée sur A", |m| is_privmsg(m, "Gui", "réponse privée") && m.nick() == "Bob");
    b.expect("réponse privée sur B", |m| is_privmsg(m, "Gui", "réponse privée"));

    // Marque de lecture : posée par A, annoncée à B (et à A).
    a.send(&format!("MARKREAD #test timestamp={hello_time}"));
    let mk = b.expect("MARKREAD sur B", |m| m.command == "MARKREAD" && m.param(0) == "#test");
    assert_eq!(mk.param(1), format!("timestamp={hello_time}"));
    a.expect("MARKREAD sur A", |m| m.command == "MARKREAD" && m.param(1).starts_with("timestamp="));
    // Plus ancienne : refusée, la marque connue revient.
    a.send("MARKREAD #test timestamp=2000-01-01T00:00:00.000Z");
    let back = a.expect("MARKREAD inchangé", |m| m.command == "MARKREAD");
    assert_eq!(back.param(1), format!("timestamp={hello_time}"));
    a.send("MARKREAD #test");
    a.expect("MARKREAD (lecture)", |m| m.command == "MARKREAD" && m.param(1) == format!("timestamp={hello_time}"));

    // B se détache (QUIT) : le relais reste sur le réseau.
    b.send("QUIT :à plus");
    b.expect("ERROR", |m| m.command == "ERROR");
    b.wait_closed();
    std::thread::sleep(Duration::from_millis(200));
    assert!(env.ircd.is_connected("Gui"));
    assert!(!env.ircd.lines_from("Gui").iter().any(|l| l.starts_with("QUIT") || l.starts_with("PART")));

    // Pendant ce temps…
    bob.send("PRIVMSG #test :manqué 1");
    bob.send("PRIVMSG #test :manqué 2");
    a.expect("manqué 2 sur A", |m| is_privmsg(m, "#test", "manqué 2"));

    // B revient et rattrape l'historique.
    let mut b = env.client();
    let reg_b = b.register("gui:secret", "Gui", CAPS_B, Some("1"));
    joined_burst(&reg_b, &mut b);
    b.send("CHATHISTORY LATEST #test * 10");
    let start = b.expect("BATCH +", |m| m.command == "BATCH" && m.param(0).starts_with('+'));
    assert_eq!(start.params[1..], ["chathistory", "#test"]);
    assert!(start.tags.has("draft/chathistory-end"));
    let bid = start.param(0)[1..].to_string();
    let mut texts = Vec::new();
    loop {
        let m = b.next(WAIT).expect("historique");
        if m.command == "BATCH" {
            assert_eq!(m.param(0), format!("-{bid}"));
            break;
        }
        assert_eq!(m.tags.get("batch"), Some(bid.as_str()));
        assert!(m.tags.get("msgid").is_some() && m.tags.get("time").is_some());
        texts.push(m.param(1).to_string());
    }
    assert_eq!(texts, vec!["bonjour à tous", "écrit sur le Mac", "écrit sur le téléphone", "manqué 1", "manqué 2"]);

    // Pagination par msgid : avant « manqué 1 ».
    b.send(&format!("CHATHISTORY BEFORE #test msgid={id} 10"));
    b.expect("BATCH vide", |m| m.command == "BATCH" && m.param(0).starts_with('+'));
    let end = b.next(WAIT).unwrap();
    assert_eq!(end.command, "BATCH", "rien avant le premier message : {}", end.to_line());
    b.send(&format!("CHATHISTORY AFTER #test msgid={id} 2"));
    b.expect_cmd("BATCH");
    let m1 = b.expect_cmd("PRIVMSG");
    let m2 = b.expect_cmd("PRIVMSG");
    assert_eq!((m1.param(1), m2.param(1)), ("écrit sur le Mac", "écrit sur le téléphone"));
    b.expect_cmd("BATCH");
    // Le privé, sous le nom de l'autre personne.
    b.send("CHATHISTORY LATEST bob * 10");
    let st = b.expect("BATCH privé", |m| m.command == "BATCH" && m.param(0).starts_with('+'));
    assert_eq!(st.param(2), "Bob", "nom tel qu'affiché");
    let p1 = b.expect_cmd("PRIVMSG");
    let p2 = b.expect_cmd("PRIVMSG");
    assert_eq!((p1.param(1), p2.param(1)), ("en privé", "réponse privée"));
    b.expect_cmd("BATCH");
    // Les cibles récentes.
    b.send("CHATHISTORY TARGETS timestamp=2000-01-01T00:00:00.000Z timestamp=2100-01-01T00:00:00.000Z 10");
    let tb = b.expect("BATCH targets", |m| m.command == "BATCH" && m.param(1) == "draft/chathistory-targets");
    let _ = tb;
    let t1 = b.expect_cmd("CHATHISTORY");
    let t2 = b.expect_cmd("CHATHISTORY");
    let mut names = vec![t1.param(1).to_string(), t2.param(1).to_string()];
    names.sort();
    assert_eq!(names, vec!["#test", "Bob"]);
    // Erreurs.
    b.send("CHATHISTORY LATEST #test * 0");
    b.expect("FAIL INVALID_PARAMS", |m| m.command == "FAIL" && m.param(1) == "INVALID_PARAMS");
    b.send("CHATHISTORY NOPE #test * 10");
    b.expect("FAIL sous-commande", |m| m.command == "FAIL" && m.param(0) == "CHATHISTORY");
    b.send("CHATHISTORY BEFORE #test msgid=inconnu 10");
    b.expect("FAIL msgid", |m| m.command == "FAIL" && m.trailing().contains("message ID"));

    drop(a);
    drop(b);
    drop(bob);
}

#[test]
fn replies_go_to_the_asking_device_and_local_answers() {
    let env = Env::start("routing");
    let _bob = env.bob("Bob");
    let mut a = env.client();
    let reg = a.register("gui/Fake:secret", "Gui", &["server-time", "multi-prefix", "userhost-in-names"], None);
    joined_burst(&reg, &mut a);
    let mut b = env.client();
    let reg = b.register("gui/Fake:secret", "Gui", &[], None);
    joined_burst(&reg, &mut b);
    // NAMES sans userhost-in-names pour B : pas d'adresse.
    let names_b: Vec<&Message> = b.seen.iter().filter(|m| m.command == "353").collect();
    assert!(names_b.iter().all(|m| !m.trailing().contains('!')), "{names_b:?}");
    let names_a: Vec<&Message> = a.seen.iter().filter(|m| m.command == "353").collect();
    assert!(names_a.iter().any(|m| m.trailing().contains("Bob!bob@127.0.0.1")), "{names_a:?}");

    // WHOIS par A : seul A reçoit la réponse.
    a.send("WHOIS Bob");
    let r = a.expect_cmd("311");
    assert_eq!(r.param(1), "Bob");
    a.expect_cmd("318");
    let on_b = b.until_pong("w");
    assert!(!on_b.iter().any(|m| m.command == "311" || m.command == "318"), "{on_b:?}");

    // Premier WHO : si le relais ne sait pas encore tout de Bob (arrivé après son propre WHO), la question
    // part au réseau (réponse pour B seul) et complète ce que le relais sait ; sinon il répond lui-même.
    b.send("WHO #test");
    b.expect_cmd("352");
    b.expect_cmd("315");
    let on_a = a.until_pong("w1");
    assert!(!on_a.iter().any(|m| m.command == "352" || m.command == "315"), "{on_a:?}");

    // MODE et WHO d'un canal rejoint : répondus par le relais, sans déranger le réseau.
    let before =
        env.ircd.lines_from("Gui").iter().filter(|l| l.starts_with("MODE #test") || l.starts_with("WHO #test")).count();
    b.send("MODE #test");
    let m = b.expect_cmd("324");
    assert_eq!(m.source.as_ref().unwrap().raw, "relay.test");
    assert_eq!(m.param(2), "+nt");
    b.expect_cmd("329");
    b.send("WHO #test");
    let w = b.expect_cmd("352");
    assert!(w.param(6).starts_with('H'));
    let local = b.expect_cmd("315");
    assert_eq!(local.source.as_ref().unwrap().raw, "relay.test", "réponse du relais");
    b.send("WHO #test %tcuhnfar,152");
    let wx = b.expect_cmd("354");
    assert_eq!(wx.param(1), "152");
    assert_eq!(wx.param(2), "#test");
    b.expect_cmd("315");
    b.send("TOPIC #test");
    b.expect_cmd("331");
    let after =
        env.ircd.lines_from("Gui").iter().filter(|l| l.starts_with("MODE #test") || l.starts_with("WHO #test")).count();
    assert_eq!(before, after, "rien demandé au réseau");
    let on_a = a.until_pong("m");
    assert!(!on_a.iter().any(|m| ["324", "352", "354", "315"].contains(&m.command.as_str())), "{on_a:?}");

    // Liste des bans : transmise au réseau, réponse pour B seul.
    b.send("MODE #test +b");
    b.expect_cmd("368");
    let on_a = a.until_pong("b");
    assert!(!on_a.iter().any(|m| m.command == "368"), "{on_a:?}");

    // Un JOIN depuis A : tout le monde voit l'arrivée, le canal est retenu.
    a.send("JOIN #autre");
    a.expect("JOIN #autre (A)", |m| m.command == "JOIN" && m.param(0) == "#autre");
    b.expect("JOIN #autre (B)", |m| m.command == "JOIN" && m.param(0) == "#autre");
    b.expect("NAMES #autre (B)", |m| m.command == "366" && m.param(1) == "#autre");
    let saved = std::fs::read_to_string(env.data.join("users/gui/networks.json")).unwrap();
    assert!(saved.contains("#autre"), "{saved}");
    a.send("PART #autre :bye");
    b.expect("PART #autre (B)", |m| m.command == "PART" && m.param(0) == "#autre");
    std::thread::sleep(Duration::from_millis(100));
    let saved = std::fs::read_to_string(env.data.join("users/gui/networks.json")).unwrap();
    assert!(!saved.contains("#autre"), "{saved}");
}

#[test]
fn auto_away_and_ctcp_replies_once() {
    let env = Env::start("away");
    let mut bob = env.bob("Bob");
    // Aucun appareil : le relais se met absent.
    env.ircd.wait_line("Gui", |l| l.starts_with("AWAY :"));
    let mut a = env.client();
    let reg = a.register("gui/Fake:secret", "Gui", &[], None);
    joined_burst(&reg, &mut a);
    env.ircd.wait_line("Gui", |l| l == "AWAY");
    // Le 305 de ce retour n'est pas montré.
    let lines = a.until_pong("x");
    assert!(!lines.iter().any(|m| m.command == "305"), "{lines:?}");
    let mut b = env.client();
    let reg = b.register("gui/Fake:secret", "Gui", &[], None);
    joined_burst(&reg, &mut b);

    // Un CTCP VERSION de Bob : les deux appareils répondent, une seule réponse part.
    bob.send("PRIVMSG Gui :\x01VERSION\x01");
    a.expect("CTCP A", |m| m.command == "PRIVMSG" && m.param(1) == "\x01VERSION\x01");
    b.expect("CTCP B", |m| m.command == "PRIVMSG" && m.param(1) == "\x01VERSION\x01");
    a.send("NOTICE Bob :\x01VERSION AlpIRC A\x01");
    b.send("NOTICE Bob :\x01VERSION AlpIRC B\x01");
    bob.expect("réponse CTCP", |m| m.command == "NOTICE" && m.param(1).contains("VERSION"));
    let more = bob.until_pong("c");
    assert!(!more.iter().any(|m| m.command == "NOTICE"), "une seule réponse : {more:?}");

    // Les deux partent : absent de nouveau.
    drop(a);
    drop(b);
    std::thread::sleep(Duration::from_millis(300));
    let aways = env.ircd.lines_from("Gui").into_iter().filter(|l| l.starts_with("AWAY :")).count();
    assert!(aways >= 2, "{:?}", env.ircd.lines_from("Gui"));
}

#[test]
fn authentication() {
    let env = Env::start("auth");
    // Mauvais mot de passe.
    let mut c = env.client();
    c.send("NICK x");
    c.send("PASS gui:mauvais");
    c.send("USER x 0 * :x");
    let e = c.expect("464", |m| m.command == "464");
    assert!(e.trailing().contains("Invalid"));
    c.expect_cmd("ERROR");
    c.wait_closed();
    // Utilisateur inconnu.
    let mut c = env.client();
    c.send("PASS personne:secret");
    c.send("NICK x");
    c.send("USER x 0 * :x");
    c.expect_cmd("464");
    // Réseau inconnu.
    let mut c = env.client();
    c.send("PASS gui/Nulle:secret");
    c.send("NICK x");
    c.send("USER x 0 * :x");
    c.expect("FAIL BOUNCER INVALID_NETID", |m| m.command == "FAIL" && m.param(1) == "INVALID_NETID");
    c.expect_cmd("464");
    // Sans mot de passe.
    let mut c = env.client();
    c.send("NICK x");
    c.send("USER x 0 * :x");
    c.expect_cmd("464");
    // Mot de passe seul, utilisateur dans USER.
    let mut c = env.client();
    c.send("PASS secret");
    c.send("NICK x");
    c.send("USER gui/Fake 0 * :x");
    c.expect_cmd("001");
    c.expect("JOIN #test", |m| m.command == "JOIN");

    // SASL PLAIN, réseau dans l'identité.
    use base64::Engine as _;
    let mut s = env.client();
    s.send("CAP LS 302");
    s.send("NICK x");
    s.send("USER x 0 * :x");
    let ls = s.expect("CAP LS", |m| m.command == "CAP");
    assert!(ls.trailing().contains("sasl=PLAIN"), "{}", ls.to_line());
    assert!(ls.trailing().contains("soju.im/bouncer-networks"));
    s.send("CAP REQ :sasl server-time");
    s.expect("ACK", |m| m.param(1) == "ACK");
    s.send("AUTHENTICATE PLAIN");
    s.expect("AUTHENTICATE +", |m| m.command == "AUTHENTICATE" && m.param(0) == "+");
    let b64 = base64::engine::general_purpose::STANDARD.encode(b"\0gui/Fake@phone\0secret");
    s.send(&format!("AUTHENTICATE {b64}"));
    let ok = s.expect("900", |m| m.command == "900");
    assert_eq!(ok.param(2), "gui");
    s.expect_cmd("903");
    s.send("CAP END");
    let w = s.expect_cmd("001");
    assert_eq!(w.param(0), "Gui");
    s.expect("JOIN #test", |m| m.command == "JOIN" && m.param(0) == "#test");
    // SASL raté.
    let mut f = env.client();
    f.send("CAP LS 302");
    f.send("CAP REQ :sasl");
    f.send("NICK y");
    f.send("USER y 0 * :y");
    f.send("AUTHENTICATE PLAIN");
    f.expect_cmd("AUTHENTICATE");
    f.send(&format!("AUTHENTICATE {}", base64::engine::general_purpose::STANDARD.encode(b"\0gui\0faux")));
    f.expect_cmd("904");
    f.send("CAP END");
    f.expect_cmd("464");
    // Commande avant l'enregistrement.
    let mut g = env.client();
    g.send("PRIVMSG #test :x");
    g.expect_cmd("451");
}

#[test]
fn bouncer_networks_management() {
    let env = Env::start("networks");
    let mut u = env.client();
    let caps = ["batch", "soju.im/bouncer-networks", "soju.im/bouncer-networks-notify"];
    let reg = u.register("gui:secret", "Gui", &caps, None);
    assert!(reg.iter().any(|m| m.command == "001" && m.trailing().contains("AlpIRC relay")));
    let isupport: Vec<String> = reg.iter().filter(|m| m.command == "005").flat_map(|m| m.params.clone()).collect();
    assert!(!isupport.iter().any(|t| t.starts_with("BOUNCER_NETID")), "non lié");
    // Liste initiale (notify) puis LISTNETWORKS.
    u.expect("BATCH initial", |m| m.command == "BATCH" && m.param(1) == "soju.im/bouncer-networks");
    let n = u.expect_cmd("BOUNCER");
    assert_eq!(n.param(0), "NETWORK");
    assert_eq!(n.param(1), "1");
    let attrs = alpirc_core::session::parse_attrs(n.param(2));
    let get = |k: &str| attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
    assert_eq!(get("name").as_deref(), Some("Fake"));
    assert_eq!(get("state").as_deref(), Some("connected"));
    assert_eq!(get("nickname").as_deref(), Some("Gui"));
    assert_eq!(get("tls").as_deref(), Some("0"));
    assert!(get("pass").is_none());
    u.expect_cmd("BATCH");
    u.send("BOUNCER LISTNETWORKS");
    u.expect("BATCH +", |m| m.command == "BATCH" && m.param(0).starts_with('+'));
    u.expect("NETWORK 1", |m| m.command == "BOUNCER" && m.param(1) == "1");
    u.expect("BATCH -", |m| m.command == "BATCH" && m.param(0).starts_with('-'));
    // Le non-lié ne parle pas aux canaux.
    u.send("JOIN #x");
    u.expect_cmd("403");
    u.send("ISON Bob");
    u.expect_cmd("303");

    // Ajout.
    u.send("BOUNCER ADDNETWORK name=Seconde");
    u.expect("NEED_ATTRIBUTE", |m| m.command == "FAIL" && m.param(1) == "NEED_ATTRIBUTE" && m.param(3) == "host");
    u.send("BOUNCER ADDNETWORK host=x;state=connected");
    u.expect("READ_ONLY", |m| {
        m.command == "FAIL" && m.param(1) == "READ_ONLY_ATTRIBUTE" && m.param(3) == "*" && m.param(4) == "state"
    });
    let port = env.ircd.addr.port();
    u.send(&format!("BOUNCER ADDNETWORK name=Seconde;host=127.0.0.1;port={port};tls=0;nickname=Gui2"));
    let note = u.expect("NETWORK 2", |m| m.command == "BOUNCER" && m.param(0) == "NETWORK" && m.param(1) == "2");
    assert!(note.param(2).contains("name=Seconde"));
    u.expect("ADDNETWORK 2", |m| m.command == "BOUNCER" && m.param(0) == "ADDNETWORK" && m.param(1) == "2");
    u.expect("connecté", |m| m.command == "BOUNCER" && m.param(1) == "2" && m.param(2).contains("state=connected"));
    assert!(env.ircd.is_connected("Gui2"));
    let saved = std::fs::read_to_string(env.data.join("users/gui/networks.json")).unwrap();
    assert!(saved.contains("Seconde"));

    // Un appareil lié à ce réseau.
    let mut c = env.client();
    let reg = c.register("gui/2:secret", "x", &["server-time"], None);
    assert_eq!(reg.iter().find(|m| m.command == "001").unwrap().param(0), "Gui2");

    // Changement du pseudo : NICK sur le réseau, notification.
    u.send("BOUNCER CHANGENETWORK 2 nickname=Gui3");
    u.expect("notif nickname", |m| m.command == "BOUNCER" && m.param(1) == "2" && m.param(2).contains("nickname=Gui3"));
    u.expect("CHANGENETWORK", |m| m.command == "BOUNCER" && m.param(0) == "CHANGENETWORK");
    env.ircd.wait_line("Gui2", |l| l == "NICK Gui3");
    c.expect("NICK", |m| m.command == "NICK" && m.param(0) == "Gui3");
    u.send("BOUNCER CHANGENETWORK 9 name=x");
    u.expect("INVALID_NETID", |m| m.command == "FAIL" && m.param(1) == "INVALID_NETID" && m.param(3) == "9");
    u.send("BOUNCER DELNETWORK 1");
    u.expect("config", |m| m.command == "FAIL" && m.param(1) == "NETWORK_IN_CONFIG");

    // Suppression : le réseau est quitté, l'appareil lié est fermé.
    u.send("BOUNCER DELNETWORK 2");
    u.expect("NETWORK 2 *", |m| m.command == "BOUNCER" && m.param(1) == "2" && m.param(2) == "*");
    u.expect("DELNETWORK", |m| m.command == "BOUNCER" && m.param(0) == "DELNETWORK");
    c.expect_cmd("ERROR");
    env.ircd.wait_line("Gui3", |l| l.starts_with("QUIT"));
    u.send("BOUNCER NOPE");
    u.expect("UNKNOWN_COMMAND", |m| m.command == "FAIL" && m.param(1) == "UNKNOWN_COMMAND");
    u.send("BOUNCER BIND 1");
    u.expect("REGISTRATION_IS_COMPLETED", |m| m.command == "FAIL" && m.param(1) == "REGISTRATION_IS_COMPLETED");
}

#[test]
fn upstream_loss_and_relay_restart() {
    let mut env = Env::start("restart");
    let bob = env.bob("Bob");
    let mut a = env.client();
    let reg = a.register(
        "gui/Fake:secret",
        "Gui",
        &["server-time", "soju.im/bouncer-networks", "soju.im/bouncer-networks-notify"],
        None,
    );
    joined_burst(&reg, &mut a);
    a.send("JOIN #garde");
    a.expect("JOIN #garde", |m| m.command == "JOIN" && m.param(0) == "#garde");
    bob.send("PRIVMSG #test :avant la coupure");
    a.expect("avant", |m| is_privmsg(m, "#test", "avant la coupure"));

    // Le réseau coupe : avis et notification (pas de PART : l'appareil garde ses canaux), reconnexion, retour
    // sur les canaux.
    env.ircd.kill("Gui");
    let notice =
        a.expect("avis de coupure", |m| m.command == "NOTICE" && m.trailing().contains("Disconnected from Fake"));
    assert!(notice.trailing().contains("reconnecting"));
    a.expect("déconnecté", |m| m.command == "BOUNCER" && m.param(2).contains("state=disconnected"));
    a.expect("reconnecté", |m| m.command == "BOUNCER" && m.param(2).contains("state=connected"));
    a.expect("JOIN #test", |m| m.command == "JOIN" && m.param(0) == "#test");
    a.expect("JOIN #garde", |m| m.command == "JOIN" && m.param(0) == "#garde");
    drop(a);

    // Le relais redémarre : historique et canaux retrouvés.
    env.stop_relay();
    std::thread::sleep(Duration::from_millis(300));
    env.boot();
    env.ircd.wait_member("#garde", "Gui");
    let mut a = env.client();
    let reg = a.register("gui/Fake:secret", "Gui", &["server-time", "batch", "draft/chathistory"], None);
    joined_burst(&reg, &mut a);
    a.send("CHATHISTORY LATEST #test * 5");
    a.expect_cmd("BATCH");
    let m = a.expect_cmd("PRIVMSG");
    assert_eq!(m.param(1), "avant la coupure");
    a.expect_cmd("BATCH");
}

#[test]
fn malformed_input_is_harmless() {
    let env = Env::start("malformed");
    // Avant l'enregistrement.
    let mut c = env.client();
    for l in [
        "",
        ":",
        "@",
        "@a=b",
        ":x",
        "CAP",
        "CAP REQ",
        "CAP FOO",
        "AUTHENTICATE",
        "BOUNCER",
        "BOUNCER BIND",
        "USER",
        "NICK",
        "PASS",
        "\u{1}\u{2}\u{3}",
        "CHATHISTORY LATEST #test * 10",
    ] {
        c.send(l);
    }
    c.send(&format!("PRIVMSG #test :{}", "x".repeat(600)));
    c.until_pong("vivant");
    // Une fois lié.
    let mut a = env.client();
    let reg = a.register(
        "gui/Fake:secret",
        "Gui",
        &["batch", "draft/chathistory", "draft/read-marker", "message-tags"],
        None,
    );
    joined_burst(&reg, &mut a);
    for l in [
        "PRIVMSG",
        "PRIVMSG #test",
        "NOTICE",
        "TAGMSG",
        "CHATHISTORY",
        "CHATHISTORY LATEST",
        "CHATHISTORY LATEST #test",
        "CHATHISTORY LATEST #test * x",
        "CHATHISTORY BETWEEN #test timestamp=x msgid= 10",
        "CHATHISTORY TARGETS * * 10",
        "CHATHISTORY AROUND #test timestamp=2026-13-45T99:99:99Z 10",
        "MARKREAD",
        "MARKREAD #test timestamp=nope",
        "MARKREAD #test *",
        "BOUNCER CHANGENETWORK",
        "BOUNCER ADDNETWORK ;;;===",
        "BOUNCER DELNETWORK",
        "CAP REQ :inconnue",
        "MODE",
        "WHO",
        "NAMES",
        "TOPIC",
        "JOIN",
        "AUTHENTICATE PLAIN",
        "USER a b c d",
    ] {
        a.send(l);
    }
    a.send(&format!("PRIVMSG #test :{}", "é".repeat(400)));
    let lines = a.until_pong("toujours vivant");
    assert!(lines.iter().any(|m| m.command == "417"), "ligne trop longue refusée : {lines:?}");
    assert!(lines.iter().any(|m| m.command == "FAIL" && m.param(0) == "MARKREAD"));
    assert!(lines.iter().any(|m| m.command == "FAIL" && m.param(0) == "CHATHISTORY"));
    // Toujours utilisable.
    a.send("PRIVMSG #test :encore là");
    env.ircd.wait_line("Gui", |l| l == "PRIVMSG #test :encore là");
}

#[test]
fn slow_device_is_disconnected() {
    use std::io::{Read, Write};
    let env = Env::start_with("slow", |c| {
        c.listen.push(alpirc_relay::config::ListenConfig { address: "127.0.0.1:0".into(), tls: false });
        c.clients.max_send_queue_kib = 64;
    });
    let plain = env.relay.as_ref().unwrap().listeners()[1].0;
    let bob = env.bob("Bob");
    let mut a = env.client();
    let reg = a.register("gui/Fake:secret", "Gui", &["server-time"], None);
    joined_burst(&reg, &mut a);
    // De quoi remplir l'historique : 300 longues lignes.
    let long = "x".repeat(400);
    for i in 0..300 {
        bob.send(&format!("PRIVMSG #test :{i} {long}"));
    }
    bob.send("PRIVMSG #test :fin");
    a.expect("fin", |m| is_privmsg(m, "#test", "fin"));

    // Un appareil qui demande beaucoup d'historique et ne lit jamais.
    let mut slow = std::net::TcpStream::connect(plain).unwrap();
    let mut out = String::from("PASS gui/Fake:secret\r\nNICK lent\r\nUSER lent 0 * :lent\r\n");
    for _ in 0..200 {
        out.push_str("CHATHISTORY LATEST #test * 1000\r\n");
    }
    slow.write_all(out.as_bytes()).unwrap();
    // Le relais le coupe (ERROR puis fermeture, 2 s au plus pour vider) : on lit ce qui est arrivé, puis la fin.
    std::thread::sleep(std::time::Duration::from_secs(4));
    slow.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
    let mut buf = vec![0u8; 256 * 1024];
    let mut total = 0usize;
    let ended = loop {
        match slow.read(&mut buf) {
            Ok(0) => break true,
            Ok(n) => total += n,
            Err(e) if e.kind() == std::io::ErrorKind::ConnectionReset => break true,
            Err(_) => break false,
        }
    };
    assert!(ended, "le relais doit fermer l'appareil trop lent (lu {total} octets)");
    assert!(total < 200 * 300 * 400, "tout n'a pas été envoyé : {total} octets");
    // Les autres appareils continuent.
    bob.send("PRIVMSG #test :toujours là");
    a.expect("toujours là", |m| is_privmsg(m, "#test", "toujours là"));
}
