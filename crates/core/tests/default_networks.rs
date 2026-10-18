//! Réseaux par défaut : TLS là où le réseau l'offre à tous, Undernet en tête ; et un fichier de réglages existant
//! garde ses réseaux (les valeurs par défaut ne servent qu'aux nouvelles installations).

mod common;

use alpirc_core::Settings;
use alpirc_core::settings::{UndernetX, default_networks};
use common::*;

fn server(name: &str) -> (String, u16, bool) {
    let n = default_networks().into_iter().find(|n| n.name == name).unwrap_or_else(|| panic!("{name} absent"));
    let s = &n.servers[0];
    (s.host.clone(), s.port, s.tls)
}

#[test]
fn defaults_use_tls_where_offered() {
    let names: Vec<String> = default_networks().into_iter().map(|n| n.name).collect();
    assert_eq!(names[0], "Undernet", "Undernet en tête (ADR 0010)");
    for (name, host) in [
        ("Libera.Chat", "irc.libera.chat"),
        ("OFTC", "irc.oftc.net"),
        ("EFnet", "irc.efnet.org"),
        ("DALnet", "irc.dal.net"),
        ("Rizon", "irc.rizon.net"),
        ("EpiKnet", "irc.epiknet.org"),
    ] {
        assert_eq!(server(name), (host.to_string(), 6697, true), "{name}");
    }
    // Sans TLS publié pour tous (sources dans settings.rs) : le port en clair, un seul serveur.
    assert_eq!(server("Undernet"), ("irc.undernet.org".into(), 6667, false));
    assert_eq!(server("IRCnet"), ("irc.ircnet.com".into(), 6667, false));
    assert_eq!(server("QuakeNet"), ("irc.quakenet.org".into(), 6667, false));
    for n in default_networks() {
        assert_eq!(n.servers.len(), 1, "{} : pas de serveur de repli en clair à côté de TLS", n.name);
    }
}

#[test]
fn undernet_login_on_connect_still_goes_through_pass() {
    let mut s = settings();
    let u = s.network_mut("Undernet").unwrap();
    u.undernet_x = Some(UndernetX { username: "guiwald".into(), password: "pw".into(), ..UndernetX::default() });
    let mut h = H::with(s);
    h.c.connect(h.status, "Undernet", None, h.now);
    let fx = h.effects();
    assert!(fx.iter().any(
        |e| matches!(e, alpirc_core::Effect::Connect { host, port: 6667, tls: false, .. } if host == "irc.undernet.org")
    ));
    h.c.on_connected(h.conn, h.now);
    assert_eq!(h.sends()[0], "PASS :+x! guiwald pw");
}

#[test]
fn existing_settings_keep_their_networks() {
    let dir = tmpdir();
    let path = dir.join("settings.toml");
    // Un fichier d'une version précédente : IRCnet sur l'ancien hôte, Undernet en TLS choisi à la main.
    let old = r#"version = 1

[[networks]]
name = "IRCnet"

[[networks.servers]]
host = "open.ircnet.net"
port = 6667

[[networks]]
name = "Undernet"

[[networks.servers]]
host = "irc.undernet.org"
port = 7000
tls = true
"#;
    std::fs::write(&path, old).unwrap();
    let s = Settings::load(&path);
    assert_eq!(s.networks.len(), 2, "pas de réseaux par défaut ajoutés");
    assert_eq!(s.networks[0].servers[0].host, "open.ircnet.net");
    assert_eq!((s.networks[1].servers[0].port, s.networks[1].servers[0].tls), (7000, true));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), old, "le fichier n'est pas réécrit à la lecture");
    // Sans liste de réseaux du tout (fichier minimal) : les valeurs par défaut.
    std::fs::write(&path, "version = 1\n").unwrap();
    assert_eq!(Settings::load(&path).networks, default_networks());
    let _ = std::fs::remove_dir_all(&dir);
}
