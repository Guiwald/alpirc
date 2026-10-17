//! Réglages du relais (`relay.toml`) : écoutes, certificat, historique, absence automatique, connexions aux
//! réseaux, utilisateurs et leurs réseaux.
//!
//! Les réseaux reprennent les champs de [`alpirc_core::settings::Network`] (serveurs, pseudo, SASL, compte X
//! d'Undernet, NickServ, perform, encodage), avec en plus la liste des canaux à rejoindre. `--add-user` et
//! `--passwd` modifient le fichier en gardant ses commentaires ([`set_user_password`]).

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use alpirc_core::settings::Network;
use serde::{Deserialize, Serialize};

use crate::auth;

/// Modèle commenté des réglages (aussi installé par le paquet `.deb` dans `/etc/alpirc-relay/relay.toml`).
pub const TEMPLATE: &str = include_str!("../../../packaging/linux/alpirc-relay/relay.toml");

/// Nom de serveur par défaut du relais (source de ses propres messages ; un point le fait reconnaître comme
/// serveur par les clients).
pub const DEFAULT_SERVER_NAME: &str = "alpirc.relay";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Nom du relais vu par les clients (source de ses numériques).
    pub server_name: String,
    /// Écoutes (par défaut : `0.0.0.0:6697` en TLS).
    pub listen: Vec<ListenConfig>,
    /// Certificat TLS (PEM) ; relatif au dossier de données ; par défaut `relay.crt`.
    pub cert: Option<PathBuf>,
    /// Clé privée (PEM) ; par défaut `relay.key`.
    pub key: Option<PathBuf>,
    /// Noms (DNS ou adresses IP) mis dans le certificat généré ; le client épingle l'empreinte, les noms
    /// servent aux autres clients.
    pub cert_names: Vec<String>,
    /// Message du jour montré aux appareils (une ligne par ligne du texte).
    pub motd: Option<String>,
    pub history: HistoryConfig,
    pub away: AwayConfig,
    pub upstream: UpstreamConfig,
    pub users: Vec<UserConfig>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            server_name: DEFAULT_SERVER_NAME.into(),
            listen: vec![ListenConfig { address: "0.0.0.0:6697".into(), tls: true }],
            cert: None,
            key: None,
            cert_names: Vec::new(),
            motd: None,
            history: HistoryConfig::default(),
            away: AwayConfig::default(),
            upstream: UpstreamConfig::default(),
            users: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ListenConfig {
    /// `adresse:port` (`0.0.0.0:6697`, `[::]:6697`, `127.0.0.1:6667`).
    pub address: String,
    pub tls: bool,
}

impl Default for ListenConfig {
    fn default() -> ListenConfig {
        ListenConfig { address: "0.0.0.0:6697".into(), tls: true }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HistoryConfig {
    pub enabled: bool,
    /// Jours gardés (0 : sans limite).
    pub retention_days: u32,
    /// Lignes gardées au plus par canal ou privé (0 : sans limite).
    pub max_lines_per_target: usize,
}

impl Default for HistoryConfig {
    fn default() -> HistoryConfig {
        HistoryConfig { enabled: true, retention_days: 365, max_lines_per_target: 100_000 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AwayConfig {
    /// Se mettre absent sur un réseau quand aucun appareil n'y est attaché.
    pub auto_away: bool,
    pub message: String,
}

impl Default for AwayConfig {
    fn default() -> AwayConfig {
        AwayConfig { auto_away: true, message: "Away (no device connected)".into() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UpstreamConfig {
    /// Lignes envoyées d'un coup à un réseau avant de ralentir.
    pub flood_burst: u32,
    /// Ensuite, une ligne toutes les `flood_interval_ms`.
    pub flood_interval_ms: u64,
    /// Premier délai de reconnexion (il double à chaque échec)…
    pub reconnect_min_secs: u64,
    /// … jusqu'à ce plafond.
    pub reconnect_max_secs: u64,
    /// Sans rien recevoir pendant ce temps, le relais envoie un PING…
    pub ping_interval_secs: u64,
    /// … et tient la connexion pour morte au bout de celui-ci.
    pub ping_timeout_secs: u64,
}

impl Default for UpstreamConfig {
    fn default() -> UpstreamConfig {
        UpstreamConfig {
            flood_burst: 10,
            flood_interval_ms: 1500,
            reconnect_min_secs: 10,
            reconnect_max_secs: 300,
            ping_interval_secs: 90,
            ping_timeout_secs: 240,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UserConfig {
    /// Nom de connexion (lettres, chiffres, `.`, `_`, `-`).
    pub name: String,
    /// Empreinte argon2id (`alpirc-relay --add-user` / `--passwd`).
    pub password_hash: String,
    pub networks: Vec<NetworkConfig>,
}

/// Un réseau d'un utilisateur : les champs d'un réseau d'AlpIRC, plus les canaux à rejoindre.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkConfig {
    #[serde(flatten)]
    pub network: Network,
    /// Canaux rejoints à chaque connexion (avec leur clé : `#canal clé`), en plus de ceux que l'utilisateur
    /// a rejoints depuis ses appareils.
    pub channels: Vec<String>,
    /// Faux : le réseau est connu mais le relais ne s'y connecte pas.
    pub enabled: bool,
}

impl Default for NetworkConfig {
    fn default() -> NetworkConfig {
        NetworkConfig { network: Network::default(), channels: Vec::new(), enabled: true }
    }
}

impl Config {
    /// Lit et vérifie des réglages.
    pub fn parse(text: &str) -> Result<Config, String> {
        let cfg: Config = toml::from_str(text).map_err(|e| format!("réglages invalides : {e}"))?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Lit le fichier.
    pub fn load(path: &Path) -> Result<Config, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{} : {e}", path.display()))?;
        Config::parse(&text).map_err(|e| format!("{} : {e}", path.display()))
    }

    /// Vérifications : écoutes lisibles, noms d'utilisateurs valides et uniques, réseaux nommés et uniques.
    pub fn validate(&self) -> Result<(), String> {
        for l in &self.listen {
            parse_listen(&l.address)?;
        }
        let mut seen: Vec<String> = Vec::new();
        for u in &self.users {
            if !auth::valid_user_name(&u.name) {
                return Err(format!("nom d'utilisateur invalide : « {} » (lettres, chiffres, . _ -)", u.name));
            }
            let low = u.name.to_ascii_lowercase();
            if seen.contains(&low) {
                return Err(format!("utilisateur en double : « {} »", u.name));
            }
            seen.push(low);
            if !u.password_hash.is_empty() && !auth::is_valid_hash(&u.password_hash) {
                return Err(format!("empreinte de mot de passe illisible pour « {} »", u.name));
            }
            let mut names: Vec<String> = Vec::new();
            for n in &u.networks {
                let name = n.network.name.trim();
                if name.is_empty() {
                    return Err(format!("un réseau de « {} » n'a pas de nom", u.name));
                }
                let low = name.to_lowercase();
                if names.contains(&low) {
                    return Err(format!("réseau en double pour « {} » : « {name} »", u.name));
                }
                names.push(low);
            }
        }
        Ok(())
    }

    /// Avertissements sans gravité (affichés au démarrage).
    pub fn warnings(&self) -> Vec<String> {
        let mut w = Vec::new();
        if self.users.is_empty() {
            w.push("aucun utilisateur : ajoutez-en un avec « alpirc-relay --add-user <nom> »".to_string());
        }
        for u in &self.users {
            if u.password_hash.is_empty() {
                w.push(format!("« {} » n'a pas de mot de passe : il ne peut pas se connecter (--passwd)", u.name));
            }
            for n in &u.networks {
                if n.network.servers.is_empty() {
                    w.push(format!("le réseau « {} » de « {} » n'a pas de serveur", n.network.name, u.name));
                }
            }
        }
        if self.listen.iter().any(|l| !l.tls && !is_loopback(&l.address)) {
            w.push(
                "une écoute sans TLS est ouverte au-delà de cette machine : les mots de passe y passent en clair"
                    .into(),
            );
        }
        w
    }

    /// Chemin du certificat.
    pub fn cert_path(&self, data: &Path) -> PathBuf {
        resolve(data, self.cert.as_deref(), "relay.crt")
    }

    /// Chemin de la clé privée.
    pub fn key_path(&self, data: &Path) -> PathBuf {
        resolve(data, self.key.as_deref(), "relay.key")
    }

    /// Utilisateur par son nom (sans tenir compte de la casse).
    pub fn user(&self, name: &str) -> Option<&UserConfig> {
        self.users.iter().find(|u| u.name.eq_ignore_ascii_case(name))
    }
}

fn resolve(data: &Path, p: Option<&Path>, default: &str) -> PathBuf {
    match p {
        Some(p) if p.is_absolute() => p.to_path_buf(),
        Some(p) => data.join(p),
        None => data.join(default),
    }
}

/// Lit une adresse d'écoute.
pub fn parse_listen(s: &str) -> Result<SocketAddr, String> {
    s.trim().parse::<SocketAddr>().map_err(|_| format!("adresse d'écoute invalide : « {s} » (attendu : 0.0.0.0:6697)"))
}

fn is_loopback(addr: &str) -> bool {
    parse_listen(addr).map(|a| a.ip().is_loopback()).unwrap_or(false)
}

/// Écrit le modèle commenté si le fichier n'existe pas. Rend vrai s'il a été écrit.
pub fn write_template_if_missing(path: &Path) -> Result<bool, String> {
    if path.exists() {
        return Ok(false);
    }
    if let Some(dir) = path.parent()
        && !dir.as_os_str().is_empty()
    {
        std::fs::create_dir_all(dir).map_err(|e| format!("{} : {e}", dir.display()))?;
    }
    write_private(path, TEMPLATE).map_err(|e| format!("{} : {e}", path.display()))?;
    Ok(true)
}

/// Ajoute un utilisateur (`create`) ou change son mot de passe, en gardant les commentaires et la mise en
/// forme du fichier, son propriétaire et ses droits.
pub fn set_user_password(path: &Path, name: &str, hash: &str, create: bool) -> Result<(), String> {
    if !auth::valid_user_name(name) {
        return Err(format!("nom d'utilisateur invalide : « {name} » (lettres, chiffres, . _ -)"));
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("{} : {e}", path.display()))?;
    let new_text = edit_user_password(&text, name, hash, create)?;
    // Le résultat doit rester lisible.
    Config::parse(&new_text)?;
    write_preserving(path, &new_text).map_err(|e| format!("{} : {e}", path.display()))
}

/// La modification de [`set_user_password`], sur le texte.
pub fn edit_user_password(text: &str, name: &str, hash: &str, create: bool) -> Result<String, String> {
    use toml_edit::{ArrayOfTables, DocumentMut, Item, value};
    let mut doc: DocumentMut = text.parse().map_err(|e| format!("réglages illisibles : {e}"))?;
    let item = doc.entry("users").or_insert(Item::ArrayOfTables(ArrayOfTables::new()));
    let users = item.as_array_of_tables_mut().ok_or("« users » doit être une liste de tables [[users]]")?;
    let found =
        users.iter_mut().find(|t| t.get("name").and_then(|v| v.as_str()).is_some_and(|n| n.eq_ignore_ascii_case(name)));
    match (found, create) {
        (Some(_), true) => {
            Err(format!("l'utilisateur « {name} » existe déjà (changer son mot de passe : --passwd {name})"))
        }
        (Some(t), false) => {
            t["password_hash"] = value(hash);
            Ok(doc.to_string())
        }
        (None, true) => {
            // Ajouté au bout du texte (après les commentaires de fin, que toml_edit garderait en dernier) : le
            // nom (vérifié) et l'empreinte PHC n'ont ni guillemet ni barre oblique inverse.
            if !auth::valid_user_name(name) || hash.contains(['"', '\\', '\n']) {
                return Err("nom ou empreinte invalide".into());
            }
            let mut out = text.to_string();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&format!("\n[[users]]\nname = \"{name}\"\npassword_hash = \"{hash}\"\n"));
            Ok(out)
        }
        (None, false) => Err(format!("utilisateur inconnu : « {name} » (l'ajouter : --add-user {name})")),
    }
}

/// Écrit un fichier lisible par son seul propriétaire (0600 sous Unix).
pub fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(path)?;
    f.write_all(text.as_bytes())?;
    f.sync_all()
}

/// Remplace un fichier d'un coup (temporaire puis renommage) en gardant ses droits et, si possible, son
/// propriétaire et son groupe (`/etc/alpirc-relay/relay.toml` est à root, groupe alpirc-relay).
pub fn write_preserving(path: &Path, text: &str) -> std::io::Result<()> {
    let meta = std::fs::metadata(path).ok();
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    write_private(&tmp, text)?;
    if let Some(m) = &meta {
        std::fs::set_permissions(&tmp, m.permissions())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let _ = std::os::unix::fs::chown(&tmp, Some(m.uid()), Some(m.gid()));
        }
    }
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_parses_to_defaults() {
        let c = Config::parse(TEMPLATE).expect("le modèle doit être valide");
        assert_eq!(c.listen, vec![ListenConfig { address: "0.0.0.0:6697".into(), tls: true }]);
        assert!(c.users.is_empty());
        assert!(c.history.enabled);
        assert_eq!(c.server_name, DEFAULT_SERVER_NAME);
    }

    #[test]
    fn full_config() {
        let text = r##"
server_name = "relais.example.org"
motd = "Bonjour\nDeuxième ligne"

[[listen]]
address = "0.0.0.0:6697"
tls = true

[[listen]]
address = "127.0.0.1:6667"
tls = false

[history]
retention_days = 30
max_lines_per_target = 5000

[away]
auto_away = false

[upstream]
flood_burst = 4

[[users]]
name = "guillaume"
password_hash = "$argon2id$v=19$m=19456,t=2,p=1$c2FsdHNhbHRzYWx0$J+6wQu1PqJgRvZb0j4Pl9eK8V9eM3m7l5nB4w2y0ZlU"

[[users.networks]]
name = "Undernet"
nick = "Guiwald"
alt_nick = "Guiwald_"
encoding = "latin1"
channels = ["#lyon", "#secret clé"]
perform = ["MODE Guiwald +x"]

[users.networks.undernet_x]
username = "guiwald"
password = "pw"

[[users.networks.servers]]
host = "irc.undernet.org"
port = 6667

[[users.networks]]
name = "Libera"
enabled = false

[users.networks.sasl]
account = "gui"
password = "pw"

[[users.networks.servers]]
host = "irc.libera.chat"
port = 6697
tls = true
"##;
        let c = Config::parse(text).unwrap();
        assert_eq!(c.server_name, "relais.example.org");
        assert_eq!(c.listen.len(), 2);
        assert!(!c.listen[1].tls);
        assert_eq!(c.history.retention_days, 30);
        assert_eq!(c.history.max_lines_per_target, 5000);
        assert!(!c.away.auto_away);
        assert_eq!(c.upstream.flood_burst, 4);
        assert_eq!(c.upstream.flood_interval_ms, 1500, "valeur par défaut gardée");
        let u = c.user("Guillaume").unwrap();
        assert_eq!(u.networks.len(), 2);
        let n = &u.networks[0];
        assert_eq!(n.network.name, "Undernet");
        assert_eq!(n.network.nick.as_deref(), Some("Guiwald"));
        assert_eq!(n.network.encoding, "latin1");
        assert_eq!(n.channels, vec!["#lyon", "#secret clé"]);
        assert_eq!(n.network.perform, vec!["MODE Guiwald +x"]);
        assert_eq!(n.network.undernet_x.as_ref().unwrap().username, "guiwald");
        assert!(n.network.undernet_x.as_ref().unwrap().require_x, "valeur par défaut d'AlpIRC");
        assert_eq!(n.network.servers[0].host, "irc.undernet.org");
        assert!(!n.network.servers[0].tls);
        assert!(n.enabled);
        let l = &u.networks[1];
        assert!(!l.enabled);
        assert_eq!(l.network.sasl.as_ref().unwrap().mechanism, "PLAIN");
        assert!(l.network.servers[0].tls);
        assert!(!c.warnings().iter().any(|w| w.contains("sans TLS")), "6667 est sur 127.0.0.1");
    }

    #[test]
    fn invalid_configs() {
        assert!(Config::parse("[[listen]]\naddress = \"pas une adresse\"\n").is_err());
        assert!(Config::parse("[[users]]\nname = \"a b\"\n").is_err());
        assert!(Config::parse("[[users]]\nname = \"a\"\n[[users]]\nname = \"A\"\n").is_err());
        assert!(Config::parse("[[users]]\nname = \"a\"\npassword_hash = \"x\"\n").is_err());
        assert!(
            Config::parse(
                "[[users]]\nname = \"a\"\n[[users.networks]]\nname = \"N\"\n[[users.networks]]\nname = \"n\"\n"
            )
            .is_err()
        );
        assert!(Config::parse("[[users]]\nname = \"a\"\n[[users.networks]]\nnick = \"x\"\n").is_err());
        assert!(Config::parse("listen = 3").is_err());
        let open = Config::parse("[[listen]]\naddress = \"0.0.0.0:6667\"\ntls = false\n").unwrap();
        assert!(open.warnings().iter().any(|w| w.contains("sans TLS")));
    }

    #[test]
    fn paths() {
        let c = Config::default();
        assert_eq!(c.cert_path(Path::new("/var/lib/r")), PathBuf::from("/var/lib/r/relay.crt"));
        let c = Config { key: Some("/etc/k.pem".into()), cert: Some("c.pem".into()), ..Config::default() };
        assert_eq!(c.key_path(Path::new("/d")), PathBuf::from("/etc/k.pem"));
        assert_eq!(c.cert_path(Path::new("/d")), PathBuf::from("/d/c.pem"));
    }

    #[test]
    fn edit_users_keeps_comments() {
        let text = "# en tête\nserver_name = \"x.y\" # nom\n\n[[listen]]\naddress = \"127.0.0.1:0\"\n";
        let h1 = auth::hash_password("un").unwrap();
        let t = edit_user_password(text, "gui", &h1, true).unwrap();
        assert!(t.starts_with("# en tête\nserver_name = \"x.y\" # nom\n"), "{t}");
        let c = Config::parse(&t).unwrap();
        assert!(auth::verify_password(&c.user("gui").unwrap().password_hash, "un"));
        assert!(edit_user_password(&t, "gui", &h1, true).is_err(), "déjà là");
        assert!(edit_user_password(&t, "autre", &h1, false).is_err(), "inconnu");
        let h2 = auth::hash_password("deux").unwrap();
        let t2 = edit_user_password(&t, "GUI", &h2, false).unwrap();
        let c = Config::parse(&t2).unwrap();
        assert_eq!(c.users.len(), 1);
        assert!(auth::verify_password(&c.user("gui").unwrap().password_hash, "deux"));
        // Le modèle livré accepte un utilisateur.
        let t3 = edit_user_password(TEMPLATE, "guillaume", &h1, true).unwrap();
        assert!(t3.starts_with(TEMPLATE), "commentaires gardés, utilisateur au bout");
        assert!(t3.trim_end().ends_with(&format!("password_hash = \"{h1}\"")));
        assert_eq!(Config::parse(&t3).unwrap().users.len(), 1);
    }

    #[test]
    fn file_edit_preserves_mode() {
        let dir = std::env::temp_dir().join(format!("alpirc-relay-cfg-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("relay.toml");
        assert!(write_template_if_missing(&p).unwrap());
        assert!(!write_template_if_missing(&p).unwrap());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o640)).unwrap();
        }
        let h = auth::hash_password("pw").unwrap();
        set_user_password(&p, "gui", &h, true).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o640);
        }
        assert_eq!(Config::load(&p).unwrap().users.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
