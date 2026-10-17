//! Les réseaux d'un utilisateur : ceux de `relay.toml` et ceux ajoutés depuis un client
//! (`BOUNCER ADDNETWORK`), avec leur identifiant stable (`netid`), les changements faits depuis un client
//! (`BOUNCER CHANGENETWORK`) et les canaux rejoints, gardés dans `<données>/users/<nom>/networks.json`.
//!
//! Un réseau des réglages garde son nom comme lien avec `relay.toml` ; ses changements faits depuis un client
//! sont rangés à part (`overrides`) et appliqués par-dessus. Un réseau ajouté depuis un client est rangé en
//! entier. Les identifiants sont des nombres (`1`, `2`…) jamais réutilisés.

use std::path::{Path, PathBuf};

use alpirc_core::settings::{Network, Server};
use serde::{Deserialize, Serialize};

use crate::config::NetworkConfig;

/// Un canal rejoint (et sa clé).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedChannel {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
}

/// Changements faits depuis un client sur un réseau de `relay.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Overrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub realname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pass: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct StoredNetwork {
    id: String,
    /// Nom dans `relay.toml` (réseau des réglages) ou nom courant (réseau ajouté).
    name: String,
    #[serde(default)]
    from_config: bool,
    /// Réglages complets d'un réseau ajouté depuis un client.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    settings: Option<Network>,
    #[serde(default)]
    overrides: Overrides,
    #[serde(default)]
    channels: Vec<SavedChannel>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
struct StoreFile {
    #[serde(default)]
    next_id: u64,
    #[serde(default)]
    networks: Vec<StoredNetwork>,
}

/// Un réseau tel que le relais l'utilise.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectiveNetwork {
    pub id: String,
    /// Réglages, changements des clients appliqués.
    pub settings: Network,
    /// Canaux à rejoindre : ceux des réglages puis ceux rejoints depuis les appareils.
    pub channels: Vec<SavedChannel>,
    pub from_config: bool,
    pub enabled: bool,
}

impl EffectiveNetwork {
    pub fn name(&self) -> &str {
        &self.settings.name
    }

    /// Attributs `soju.im/bouncer-networks` (sans `state`, que donne la connexion) ; jamais le mot de passe.
    pub fn attributes(&self, user: &str) -> Vec<(String, String)> {
        let s = &self.settings;
        let mut v = vec![("name".to_string(), s.name.clone())];
        if let Some(srv) = s.servers.first() {
            v.push(("host".into(), srv.host.clone()));
            v.push(("port".into(), srv.port.to_string()));
            v.push(("tls".into(), if srv.tls { "1" } else { "0" }.into()));
        }
        v.push(("nickname".into(), nick_or(s, user)));
        v.push(("username".into(), s.username.clone().filter(|x| !x.is_empty()).unwrap_or_else(|| user.to_string())));
        if let Some(r) = s.realname.clone().filter(|x| !x.is_empty()) {
            v.push(("realname".into(), r));
        }
        v
    }
}

/// Pseudo d'un réseau (le nom de l'utilisateur s'il n'y en a pas).
pub fn nick_or(n: &Network, user: &str) -> String {
    n.nick.clone().filter(|x| !x.trim().is_empty()).unwrap_or_else(|| user.to_string())
}

/// Erreur d'une commande `BOUNCER` (code des réponses normalisées de soju).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BouncerError {
    pub code: &'static str,
    /// Attribut en cause (`INVALID_ATTRIBUTE`, `UNKNOWN_ATTRIBUTE`, `READ_ONLY_ATTRIBUTE`, `NEED_ATTRIBUTE`).
    pub attribute: Option<String>,
    pub description: String,
}

impl BouncerError {
    fn new(code: &'static str, attribute: Option<&str>, description: &str) -> BouncerError {
        BouncerError { code, attribute: attribute.map(str::to_string), description: description.to_string() }
    }
}

/// Ce qu'un changement modifie.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Change {
    /// Attributs changés (pour la notification `BOUNCER NETWORK`).
    pub attributes: Vec<(String, String)>,
    /// La connexion doit être refaite (serveur, identité, mot de passe).
    pub reconnect: bool,
    /// Seul le pseudo change : un `NICK` suffit.
    pub nick: Option<String>,
}

const WRITABLE: &[&str] = &["name", "host", "port", "tls", "nickname", "username", "realname", "pass"];
const READ_ONLY: &[&str] = &["state", "error"];

/// Les réseaux d'un utilisateur.
#[derive(Debug)]
pub struct NetworkStore {
    path: PathBuf,
    user: String,
    config: Vec<NetworkConfig>,
    file: StoreFile,
}

impl NetworkStore {
    /// Lit `networks.json` (ou part de rien) et y range les réseaux des réglages : un identifiant pour chaque
    /// nouveau, oubli de ceux qui ont quitté `relay.toml`.
    pub fn open(path: PathBuf, user: &str, config: Vec<NetworkConfig>) -> NetworkStore {
        let file = match std::fs::read_to_string(&path) {
            Ok(t) => serde_json::from_str::<StoreFile>(&t).unwrap_or_else(|e| {
                log::warn!("{} illisible ({e}) : copie de secours .bak, réseaux ajoutés oubliés", path.display());
                let mut bak = path.as_os_str().to_owned();
                bak.push(".bak");
                let _ = std::fs::write(PathBuf::from(bak), &t);
                StoreFile::default()
            }),
            Err(_) => StoreFile::default(),
        };
        let mut s = NetworkStore { path, user: user.to_string(), config, file };
        let before = s.file.clone();
        s.sync_config();
        if s.file != before {
            s.save();
        }
        s
    }

    /// Store en mémoire seulement (essais).
    pub fn in_memory(user: &str, config: Vec<NetworkConfig>) -> NetworkStore {
        let mut s = NetworkStore { path: PathBuf::new(), user: user.to_string(), config, file: StoreFile::default() };
        s.sync_config();
        s
    }

    fn sync_config(&mut self) {
        let names: Vec<String> = self.config.iter().map(|c| c.network.name.to_lowercase()).collect();
        self.file.networks.retain(|n| !n.from_config || names.contains(&n.name.to_lowercase()));
        let max = self.file.networks.iter().filter_map(|n| n.id.parse::<u64>().ok()).max().unwrap_or(0);
        self.file.next_id = self.file.next_id.max(max + 1);
        for c in &self.config {
            let known =
                self.file.networks.iter().any(|n| n.from_config && n.name.eq_ignore_ascii_case(&c.network.name));
            if !known {
                let id = self.file.next_id.to_string();
                self.file.next_id += 1;
                self.file.networks.push(StoredNetwork {
                    id,
                    name: c.network.name.clone(),
                    from_config: true,
                    settings: None,
                    overrides: Overrides::default(),
                    channels: Vec::new(),
                });
            }
        }
    }

    fn save(&self) {
        if self.path.as_os_str().is_empty() {
            return;
        }
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let text = match serde_json::to_string_pretty(&self.file) {
            Ok(t) => t,
            Err(e) => {
                log::error!("réseaux de {} : {e}", self.user);
                return;
            }
        };
        let mut tmp = self.path.as_os_str().to_owned();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        if let Err(e) = crate::config::write_private(&tmp, &text).and_then(|_| std::fs::rename(&tmp, &self.path)) {
            log::error!("{} : {e}", self.path.display());
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn effective(&self, n: &StoredNetwork) -> Option<EffectiveNetwork> {
        if n.from_config {
            let c = self.config.iter().find(|c| c.network.name.eq_ignore_ascii_case(&n.name))?;
            let mut settings = c.network.clone();
            apply_overrides(&mut settings, &n.overrides);
            let mut channels: Vec<SavedChannel> = c.channels.iter().filter_map(|s| parse_channel_spec(s)).collect();
            for ch in &n.channels {
                if !channels.iter().any(|x| x.name.eq_ignore_ascii_case(&ch.name)) {
                    channels.push(ch.clone());
                }
            }
            Some(EffectiveNetwork { id: n.id.clone(), settings, channels, from_config: true, enabled: c.enabled })
        } else {
            let settings = n.settings.clone()?;
            Some(EffectiveNetwork {
                id: n.id.clone(),
                settings,
                channels: n.channels.clone(),
                from_config: false,
                enabled: true,
            })
        }
    }

    /// Tous les réseaux, par identifiant croissant.
    pub fn list(&self) -> Vec<EffectiveNetwork> {
        let mut v: Vec<EffectiveNetwork> = self.file.networks.iter().filter_map(|n| self.effective(n)).collect();
        v.sort_by_key(|n| (n.id.parse::<u64>().unwrap_or(u64::MAX), n.id.clone()));
        v
    }

    pub fn get(&self, id: &str) -> Option<EffectiveNetwork> {
        self.file.networks.iter().find(|n| n.id == id).and_then(|n| self.effective(n))
    }

    /// Identifiant d'un réseau donné par son identifiant ou son nom (casse indifférente).
    pub fn find(&self, id_or_name: &str) -> Option<String> {
        let list = self.list();
        list.iter()
            .find(|n| n.id == id_or_name)
            .or_else(|| list.iter().find(|n| n.settings.name.eq_ignore_ascii_case(id_or_name)))
            .map(|n| n.id.clone())
    }

    fn name_taken(&self, name: &str, except: Option<&str>) -> bool {
        self.list().iter().any(|n| Some(n.id.as_str()) != except && n.settings.name.eq_ignore_ascii_case(name))
    }

    /// `BOUNCER ADDNETWORK` : rend l'identifiant du nouveau réseau.
    pub fn add(&mut self, attrs: &[(String, String)]) -> Result<String, BouncerError> {
        check_attributes(attrs)?;
        let get = |k: &str| attrs.iter().rev().find(|(a, _)| a == k).map(|(_, v)| v.clone());
        let host = get("host")
            .filter(|h| !h.is_empty())
            .ok_or_else(|| BouncerError::new("NEED_ATTRIBUTE", Some("host"), "Missing required attribute"))?;
        validate_host(&host)?;
        let tls = match get("tls") {
            Some(v) => parse_bool(&v)
                .ok_or_else(|| BouncerError::new("INVALID_ATTRIBUTE", Some("tls"), "Invalid attribute value"))?,
            None => true,
        };
        let port = match get("port") {
            Some(v) => parse_port(&v)?,
            None => {
                if tls {
                    6697
                } else {
                    6667
                }
            }
        };
        let name = get("name").filter(|n| !n.trim().is_empty()).unwrap_or_else(|| host.clone());
        validate_name(&name)?;
        if self.name_taken(&name, None) {
            return Err(BouncerError::new(
                "INVALID_ATTRIBUTE",
                Some("name"),
                "A network with this name already exists",
            ));
        }
        let mut server = Server::new(&host, port, tls);
        server.password = get("pass").filter(|p| !p.is_empty());
        let mut net = Network::new(&name, vec![server]);
        net.nick = get("nickname").filter(|x| !x.is_empty());
        net.username = get("username").filter(|x| !x.is_empty());
        net.realname = get("realname").filter(|x| !x.is_empty());
        net.bouncer = false;
        let id = self.file.next_id.max(1).to_string();
        self.file.next_id = self.file.next_id.max(1) + 1;
        self.file.networks.push(StoredNetwork {
            id: id.clone(),
            name: name.clone(),
            from_config: false,
            settings: Some(net),
            overrides: Overrides::default(),
            channels: Vec::new(),
        });
        self.save();
        Ok(id)
    }

    /// `BOUNCER CHANGENETWORK`.
    pub fn change(&mut self, id: &str, attrs: &[(String, String)]) -> Result<Change, BouncerError> {
        check_attributes(attrs)?;
        if attrs.is_empty() {
            return Err(BouncerError::new("NEED_ATTRIBUTE", None, "No attribute to change"));
        }
        let Some(before) = self.get(id) else {
            return Err(BouncerError::new("INVALID_NETID", None, "Network not found"));
        };
        // Vérifications d'abord : rien n'est changé si un attribut est refusé.
        for (k, v) in attrs {
            match k.as_str() {
                "name" => {
                    validate_name(v)?;
                    if self.name_taken(v, Some(id)) {
                        return Err(BouncerError::new(
                            "INVALID_ATTRIBUTE",
                            Some("name"),
                            "A network with this name already exists",
                        ));
                    }
                }
                "host" => validate_host(v)?,
                "port" => {
                    parse_port(v)?;
                }
                "tls" => {
                    parse_bool(v).ok_or_else(|| {
                        BouncerError::new("INVALID_ATTRIBUTE", Some("tls"), "Invalid attribute value")
                    })?;
                }
                "nickname" if v.contains(' ') => {
                    return Err(BouncerError::new("INVALID_ATTRIBUTE", Some("nickname"), "Invalid attribute value"));
                }
                _ => {}
            }
        }
        let Some(stored) = self.file.networks.iter_mut().find(|n| n.id == id) else {
            return Err(BouncerError::new("INVALID_NETID", None, "Network not found"));
        };
        let opt = |v: &str| (!v.is_empty()).then(|| v.to_string());
        if stored.from_config {
            let o = &mut stored.overrides;
            for (k, v) in attrs {
                match k.as_str() {
                    "name" => o.name = Some(v.clone()),
                    "host" => o.host = Some(v.clone()),
                    "port" => o.port = parse_port(v).ok(),
                    "tls" => o.tls = parse_bool(v),
                    "nickname" => o.nickname = opt(v),
                    "username" => o.username = opt(v),
                    "realname" => o.realname = opt(v),
                    "pass" => o.pass = opt(v),
                    _ => {}
                }
            }
        } else if let Some(net) = stored.settings.as_mut() {
            let mut o = Overrides::default();
            for (k, v) in attrs {
                match k.as_str() {
                    "name" => {
                        net.name = v.clone();
                        stored.name = v.clone();
                    }
                    "host" => o.host = Some(v.clone()),
                    "port" => o.port = parse_port(v).ok(),
                    "tls" => o.tls = parse_bool(v),
                    "nickname" => net.nick = opt(v),
                    "username" => net.username = opt(v),
                    "realname" => net.realname = opt(v),
                    "pass" => o.pass = Some(v.clone()),
                    _ => {}
                }
            }
            apply_overrides(net, &o);
        }
        self.save();
        let after = self.get(id).ok_or_else(|| BouncerError::new("INVALID_NETID", None, "Network not found"))?;
        let user = self.user.clone();
        let (a, b) = (before.attributes(&user), after.attributes(&user));
        let mut change = Change::default();
        for (k, v) in &b {
            if a.iter().find(|(x, _)| x == k).map(|(_, y)| y) != Some(v) {
                change.attributes.push((k.clone(), v.clone()));
            }
        }
        for (k, _) in &a {
            if !b.iter().any(|(x, _)| x == k) {
                change.attributes.push((k.clone(), String::new()));
            }
        }
        let (s0, s1) = (&before.settings, &after.settings);
        let server_changed = s0.servers != s1.servers;
        let identity_changed = s0.username != s1.username || s0.realname != s1.realname;
        let nick_changed = nick_or(s0, &user) != nick_or(s1, &user);
        change.reconnect = server_changed || identity_changed;
        if nick_changed && !change.reconnect {
            change.nick = Some(nick_or(s1, &user));
        }
        Ok(change)
    }

    /// `BOUNCER DELNETWORK` (seulement pour un réseau ajouté depuis un client).
    pub fn delete(&mut self, id: &str) -> Result<(), BouncerError> {
        let Some(i) = self.file.networks.iter().position(|n| n.id == id && (n.from_config || n.settings.is_some()))
        else {
            return Err(BouncerError::new("INVALID_NETID", None, "Network not found"));
        };
        if self.file.networks[i].from_config {
            return Err(BouncerError::new(
                "NETWORK_IN_CONFIG",
                None,
                "This network is defined in the relay configuration (relay.toml): remove it there",
            ));
        }
        self.file.networks.remove(i);
        self.save();
        Ok(())
    }

    /// Retient un canal rejoint (rend vrai s'il est nouveau).
    pub fn add_channel(&mut self, id: &str, name: &str, key: Option<&str>) -> bool {
        let in_config = |n: &StoredNetwork| {
            n.from_config
                && self.config.iter().any(|c| {
                    c.network.name.eq_ignore_ascii_case(&n.name)
                        && c.channels
                            .iter()
                            .filter_map(|s| parse_channel_spec(s))
                            .any(|ch| ch.name.eq_ignore_ascii_case(name))
                })
        };
        if self.file.networks.iter().find(|n| n.id == id).is_some_and(in_config) {
            return false;
        }
        let Some(n) = self.file.networks.iter_mut().find(|n| n.id == id) else { return false };
        let key = key.filter(|k| !k.is_empty()).map(str::to_string);
        match n.channels.iter_mut().find(|c| c.name.eq_ignore_ascii_case(name)) {
            Some(c) => {
                if key.is_some() && c.key != key {
                    c.key = key;
                    self.save();
                }
                false
            }
            None => {
                n.channels.push(SavedChannel { name: name.to_string(), key });
                self.save();
                true
            }
        }
    }

    /// Oublie un canal quitté.
    pub fn remove_channel(&mut self, id: &str, name: &str) -> bool {
        let Some(n) = self.file.networks.iter_mut().find(|n| n.id == id) else { return false };
        let before = n.channels.len();
        n.channels.retain(|c| !c.name.eq_ignore_ascii_case(name));
        let changed = n.channels.len() != before;
        if changed {
            self.save();
        }
        changed
    }

    /// Retient le pseudo choisi depuis un client (`NICK` réussi).
    pub fn set_nick(&mut self, id: &str, nick: &str) {
        let Some(n) = self.file.networks.iter_mut().find(|n| n.id == id) else { return };
        if n.from_config {
            n.overrides.nickname = Some(nick.to_string());
        } else if let Some(s) = n.settings.as_mut() {
            s.nick = Some(nick.to_string());
        }
        self.save();
    }
}

/// `#canal` ou `#canal clé`.
pub fn parse_channel_spec(s: &str) -> Option<SavedChannel> {
    let mut it = s.split_whitespace();
    let name = it.next()?.to_string();
    let key = it.next().map(str::to_string);
    Some(SavedChannel { name, key })
}

fn apply_overrides(net: &mut Network, o: &Overrides) {
    if let Some(n) = &o.name {
        net.name = n.clone();
    }
    if o.host.is_some() || o.port.is_some() || o.tls.is_some() {
        let mut s = net.servers.first().cloned().unwrap_or_else(|| Server::new("", 6697, true));
        if let Some(h) = &o.host {
            s.host = h.clone();
        }
        if let Some(t) = o.tls {
            s.tls = t;
        }
        if let Some(p) = o.port {
            s.port = p;
        }
        net.servers = vec![s];
    }
    if let Some(p) = &o.pass {
        for s in &mut net.servers {
            s.password = (!p.is_empty()).then(|| p.clone());
        }
    }
    if let Some(n) = &o.nickname {
        net.nick = Some(n.clone());
    }
    if let Some(u) = &o.username {
        net.username = Some(u.clone());
    }
    if let Some(r) = &o.realname {
        net.realname = Some(r.clone());
    }
}

fn check_attributes(attrs: &[(String, String)]) -> Result<(), BouncerError> {
    for (k, _) in attrs {
        if READ_ONLY.contains(&k.as_str()) {
            return Err(BouncerError::new("READ_ONLY_ATTRIBUTE", Some(k), "Read-only attribute"));
        }
        if !WRITABLE.contains(&k.as_str()) {
            return Err(BouncerError::new("UNKNOWN_ATTRIBUTE", Some(k), "Unknown attribute"));
        }
    }
    Ok(())
}

fn parse_bool(v: &str) -> Option<bool> {
    match v {
        "1" | "true" => Some(true),
        "0" | "false" => Some(false),
        _ => None,
    }
}

fn parse_port(v: &str) -> Result<u16, BouncerError> {
    v.parse::<u16>()
        .ok()
        .filter(|p| *p > 0)
        .ok_or_else(|| BouncerError::new("INVALID_ATTRIBUTE", Some("port"), "Invalid attribute value"))
}

fn validate_host(h: &str) -> Result<(), BouncerError> {
    if h.is_empty() || h.len() > 255 || h.contains(char::is_whitespace) {
        return Err(BouncerError::new("INVALID_ATTRIBUTE", Some("host"), "Invalid attribute value"));
    }
    Ok(())
}

fn validate_name(n: &str) -> Result<(), BouncerError> {
    if n.trim().is_empty() || n.len() > 100 || n.contains(['\r', '\n', '\0']) {
        return Err(BouncerError::new("INVALID_ATTRIBUTE", Some("name"), "Invalid attribute value"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg_net(name: &str, host: &str) -> NetworkConfig {
        NetworkConfig {
            network: Network::new(name, vec![Server::new(host, 6697, true)]),
            channels: vec!["#a".into(), "#k clé".into()],
            enabled: true,
        }
    }

    fn attrs(s: &str) -> Vec<(String, String)> {
        alpirc_core::session::parse_attrs(s)
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("alpirc-relay-nets-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d.join("networks.json")
    }

    #[test]
    fn ids_are_stable_across_restarts() {
        let p = tmp("stable");
        let s = NetworkStore::open(
            p.clone(),
            "gui",
            vec![cfg_net("Undernet", "irc.undernet.org"), cfg_net("Libera", "irc.libera.chat")],
        );
        let ids: Vec<String> = s.list().iter().map(|n| n.id.clone()).collect();
        assert_eq!(ids, vec!["1", "2"]);
        // Ordre inversé et un réseau de plus : les anciens gardent leur identifiant.
        let s = NetworkStore::open(
            p.clone(),
            "gui",
            vec![cfg_net("OFTC", "irc.oftc.net"), cfg_net("libera", "irc.libera.chat"), cfg_net("Undernet", "x")],
        );
        assert_eq!(s.find("undernet").as_deref(), Some("1"));
        assert_eq!(s.find("Libera").as_deref(), Some("2"));
        assert_eq!(s.find("OFTC").as_deref(), Some("3"));
        assert_eq!(s.find("3").as_deref(), Some("3"));
        assert_eq!(s.find("nope"), None);
        // Un réseau retiré des réglages disparaît ; son identifiant n'est pas réutilisé.
        let mut s = NetworkStore::open(p.clone(), "gui", vec![cfg_net("Undernet", "x")]);
        assert_eq!(s.list().len(), 1);
        let id = s.add(&attrs("host=irc.example.org")).unwrap();
        assert_eq!(id, "4");
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn config_channels_and_saved_channels() {
        let mut s = NetworkStore::in_memory("gui", vec![cfg_net("N", "h")]);
        let n = s.get("1").unwrap();
        assert_eq!(
            n.channels,
            vec![
                SavedChannel { name: "#a".into(), key: None },
                SavedChannel { name: "#k".into(), key: Some("clé".into()) }
            ]
        );
        assert!(s.add_channel("1", "#b", Some("k")));
        assert!(!s.add_channel("1", "#B", None));
        assert!(!s.add_channel("1", "#a", None), "déjà dans les réglages : pas en double");
        let n = s.get("1").unwrap();
        assert_eq!(n.channels.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["#a", "#k", "#b"]);
        assert!(s.remove_channel("1", "#b"));
        assert!(!s.remove_channel("1", "#b"));
    }

    #[test]
    fn add_change_delete() {
        let p = tmp("acd");
        let mut s = NetworkStore::open(p.clone(), "gui", vec![cfg_net("Conf", "irc.conf.org")]);
        assert_eq!(s.add(&attrs("name=X")).unwrap_err().code, "NEED_ATTRIBUTE");
        assert_eq!(s.add(&attrs("host=h;state=connected")).unwrap_err().code, "READ_ONLY_ATTRIBUTE");
        assert_eq!(s.add(&attrs("host=h;color=blue")).unwrap_err().code, "UNKNOWN_ATTRIBUTE");
        assert_eq!(s.add(&attrs("host=h;port=x")).unwrap_err().attribute.as_deref(), Some("port"));
        assert_eq!(s.add(&attrs("host=h;name=conf")).unwrap_err().attribute.as_deref(), Some("name"));
        let id = s.add(&attrs("name=OFTC;host=irc.oftc.net;nickname=Gui;pass=sec")).unwrap();
        let n = s.get(&id).unwrap();
        assert_eq!(n.settings.servers[0].port, 6697);
        assert!(n.settings.servers[0].tls);
        assert_eq!(n.settings.servers[0].password.as_deref(), Some("sec"));
        let a = n.attributes("gui");
        assert!(a.contains(&("name".into(), "OFTC".into())));
        assert!(a.contains(&("nickname".into(), "Gui".into())));
        assert!(a.contains(&("username".into(), "gui".into())));
        assert!(!a.iter().any(|(k, _)| k == "pass"), "jamais le mot de passe");
        // Changement du pseudo seul : un NICK suffit.
        let c = s.change(&id, &attrs("nickname=Gui2")).unwrap();
        assert_eq!(c.nick.as_deref(), Some("Gui2"));
        assert!(!c.reconnect);
        assert_eq!(c.attributes, vec![("nickname".to_string(), "Gui2".to_string())]);
        // Changement de serveur : reconnexion.
        let c = s.change(&id, &attrs("port=7000;tls=0")).unwrap();
        assert!(c.reconnect);
        assert_eq!(s.get(&id).unwrap().settings.servers[0].port, 7000);
        assert!(!s.get(&id).unwrap().settings.servers[0].tls);
        // Réseau des réglages : changements rangés à part.
        let c = s.change("1", &attrs("name=Renamed;realname=Guillaume")).unwrap();
        assert!(c.reconnect);
        assert_eq!(s.get("1").unwrap().settings.name, "Renamed");
        assert_eq!(s.find("renamed").as_deref(), Some("1"));
        assert_eq!(s.change("9", &attrs("name=x")).unwrap_err().code, "INVALID_NETID");
        assert_eq!(s.change(&id, &attrs("state=x")).unwrap_err().code, "READ_ONLY_ATTRIBUTE");
        assert_eq!(s.delete("1").unwrap_err().code, "NETWORK_IN_CONFIG");
        // Tout est relu au redémarrage.
        let s2 = NetworkStore::open(p.clone(), "gui", vec![cfg_net("Conf", "irc.conf.org")]);
        assert_eq!(s2.get("1").unwrap().settings.name, "Renamed");
        assert_eq!(s2.get(&id).unwrap().settings.servers[0].port, 7000);
        assert_eq!(s2.get(&id).unwrap().settings.nick.as_deref(), Some("Gui2"));
        let mut s = s2;
        s.delete(&id).unwrap();
        assert!(s.get(&id).is_none());
        assert_eq!(s.delete(&id).unwrap_err().code, "INVALID_NETID");
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn nick_persistence() {
        let mut s = NetworkStore::in_memory("gui", vec![cfg_net("N", "h")]);
        assert_eq!(nick_or(&s.get("1").unwrap().settings, "gui"), "gui");
        s.set_nick("1", "Guiwald");
        assert_eq!(nick_or(&s.get("1").unwrap().settings, "gui"), "Guiwald");
    }
}
