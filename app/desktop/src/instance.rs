//! Linux : une seule instance d'AlpIRC par dossier de données.
//!
//! Le bureau lance `alpirc irc://…` pour chaque lien cliqué (gestionnaire `x-scheme-handler/irc` du fichier
//! `.desktop`). Si AlpIRC tourne déjà, le nouveau processus lui passe le lien par une prise Unix et s'arrête, au
//! lieu d'ouvrir un second client ; lancé sans lien, il demande seulement à la fenêtre existante de revenir devant.
//!
//! - L'instance principale tient un verrou (`alpirc-<empreinte>.lock`, `flock`) pendant toute sa vie : le système le
//!   relâche à sa mort, même brutale, et une prise restée d'un plantage est alors simplement remplacée.
//! - Prise et verrou vont dans `$XDG_RUNTIME_DIR` (propre à l'utilisateur, droits 0700), sinon dans le dossier des
//!   données ; l'empreinte du dossier des données sépare les installations (`ALPIRC_HOME` pour les essais).
//! - Protocole : une ligne `open <lien>` ou `raise`, réponse `ok`.

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use winit::event_loop::EventLoopProxy;

use crate::UserEvent;

/// Longueur maximale d'un chemin de prise Unix (108 octets sous Linux, zéro final compris).
const SUN_PATH_MAX: usize = 107;
/// Délai pour joindre l'instance principale qui démarre (elle tient le verrou mais n'écoute pas encore).
const CONNECT_PATIENCE: Duration = Duration::from_secs(3);
const IO_TIMEOUT: Duration = Duration::from_secs(3);
/// Une demande ne dépasse pas cette taille (un lien irc:// est court).
const LINE_MAX: usize = 4096;

/// Issue de la recherche d'une instance déjà ouverte.
pub enum Claim {
    /// Nous sommes l'instance principale : la prise écoute (à servir une fois la boucle créée).
    Primary(Primary),
    /// Une instance tourne déjà et a pris la demande : il faut s'arrêter.
    Handed,
    /// Pas d'instance unique possible (dossier inaccessible, instance muette…) : on démarre quand même.
    Alone,
}

pub struct Primary {
    listener: UnixListener,
    /// Gardé ouvert (verrou) jusqu'à la fin du processus.
    _lock: File,
}

/// Empreinte FNV-1a (64 bits) : stable d'une version de Rust à l'autre, contrairement au hachage de std.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ *b as u64).wrapping_mul(0x0100_0000_01b3))
}

/// Chemins (prise, verrou) pour un dossier de données.
fn paths(data_dir: &Path, runtime_dir: Option<&Path>) -> (PathBuf, PathBuf) {
    use std::os::unix::ffi::OsStrExt;
    let canon = std::fs::canonicalize(data_dir).unwrap_or_else(|_| data_dir.to_path_buf());
    let tag = format!("{:016x}", fnv1a(canon.as_os_str().as_bytes()));
    let dir = match runtime_dir.filter(|d| d.is_absolute() && d.is_dir()) {
        Some(d) => d.to_path_buf(),
        None => data_dir.to_path_buf(),
    };
    let mut sock = dir.join(format!("alpirc-{tag}.sock"));
    if sock.as_os_str().len() > SUN_PATH_MAX {
        // Chemin trop long pour une prise : dossier temporaire, au nom de l'utilisateur.
        use std::os::unix::fs::MetadataExt;
        let uid = std::fs::metadata(data_dir).map(|m| m.uid()).unwrap_or(0);
        sock = std::env::temp_dir().join(format!("alpirc-{uid}-{tag}.sock"));
    }
    let lock = sock.with_extension("lock");
    (sock, lock)
}

/// La demande pour l'instance déjà ouverte.
fn request_line(url: Option<&str>) -> String {
    match url {
        Some(u) => format!("open {u}\n"),
        None => "raise\n".to_string(),
    }
}

/// Ce qu'une demande reçue fait faire à la boucle (`None` : demande invalide).
fn parse_request(line: &str) -> Option<UserEvent> {
    let line = line.trim_end_matches(['\r', '\n']);
    if line == "raise" {
        return Some(UserEvent::Raise);
    }
    let url = line.strip_prefix("open ")?.trim();
    crate::is_irc_url(url).then(|| UserEvent::OpenUrl(url.to_string()))
}

/// Cherche l'instance déjà ouverte ; lui passe `url` (ou la demande de revenir devant) si elle existe.
pub fn claim(data_dir: &Path, url: Option<&str>) -> Claim {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
    let (sock, lock_path) = paths(data_dir, runtime.as_deref());
    claim_at(&sock, &lock_path, url)
}

fn claim_at(sock: &Path, lock_path: &Path, url: Option<&str>) -> Claim {
    let lock = match std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(lock_path) {
        Ok(f) => f,
        Err(e) => {
            log::warn!("instance unique : verrou {} impossible ({e})", lock_path.display());
            return Claim::Alone;
        }
    };
    match lock.try_lock() {
        Ok(()) => {
            // Une prise qui traîne vient d'une instance morte : le verrou le prouve.
            let _ = std::fs::remove_file(sock);
            match UnixListener::bind(sock) {
                Ok(listener) => {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(sock, std::fs::Permissions::from_mode(0o600));
                    Claim::Primary(Primary { listener, _lock: lock })
                }
                Err(e) => {
                    log::warn!("instance unique : prise {} impossible ({e})", sock.display());
                    Claim::Alone
                }
            }
        }
        Err(std::fs::TryLockError::WouldBlock) => match hand_over(sock, url) {
            Ok(()) => Claim::Handed,
            Err(e) => {
                log::warn!("instance unique : AlpIRC tourne déjà mais ne répond pas ({e}) ; nouvelle fenêtre");
                Claim::Alone
            }
        },
        Err(std::fs::TryLockError::Error(e)) => {
            log::warn!("instance unique : verrou impossible ({e})");
            Claim::Alone
        }
    }
}

/// Passe la demande à l'instance principale et attend sa réponse.
fn hand_over(sock: &Path, url: Option<&str>) -> std::io::Result<()> {
    let start = Instant::now();
    let mut stream = loop {
        match UnixStream::connect(sock) {
            Ok(s) => break s,
            Err(e) if start.elapsed() >= CONNECT_PATIENCE => return Err(e),
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    };
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    stream.write_all(request_line(url).as_bytes())?;
    let mut reply = String::new();
    BufReader::new(stream).read_line(&mut reply)?;
    if reply.trim() == "ok" {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("réponse inattendue : {reply:?}")))
    }
}

impl Primary {
    /// Sert les demandes des instances suivantes (fil `alpirc-instance`), en les passant à la boucle.
    pub fn serve(self, proxy: EventLoopProxy<UserEvent>) {
        let Primary { listener, _lock } = self;
        let spawned = std::thread::Builder::new().name("alpirc-instance".into()).spawn(move || {
            let _lock = _lock;
            for conn in listener.incoming() {
                let Ok(stream) = conn else { continue };
                // Boucle d'événements fermée : l'application s'arrête.
                if let Some(ev) = answer(stream)
                    && proxy.send_event(ev).is_err()
                {
                    return;
                }
            }
        });
        if let Err(e) = spawned {
            log::warn!("instance unique : fil impossible ({e})");
        }
    }
}

/// Lit une demande, répond, et rend l'événement correspondant.
fn answer(stream: UnixStream) -> Option<UserEvent> {
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
    let mut line = String::new();
    let mut reader = BufReader::new(std::io::Read::take(&stream, LINE_MAX as u64));
    reader.read_line(&mut line).ok()?;
    let ev = parse_request(&line);
    let mut w = &stream;
    let _ = w.write_all(if ev.is_some() { b"ok\n" } else { b"?\n" });
    ev
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("alpirc-instance-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn demandes() {
        assert_eq!(request_line(Some("irc://irc.libera.chat/#rust")), "open irc://irc.libera.chat/#rust\n");
        assert_eq!(request_line(None), "raise\n");
        assert!(matches!(parse_request("raise\n"), Some(UserEvent::Raise)));
        let ev = parse_request("open ircs://h:6697/#a\r\n");
        assert!(matches!(ev, Some(UserEvent::OpenUrl(u)) if u == "ircs://h:6697/#a"));
        // Rien d'autre que des liens irc:// ou ircs://.
        assert!(parse_request("open https://exemple.org\n").is_none());
        assert!(parse_request("open file:///etc/passwd\n").is_none());
        assert!(parse_request("quit\n").is_none());
        assert!(parse_request("").is_none());
    }

    #[test]
    fn chemins() {
        let d = tmp("chemins");
        let (s, l) = paths(&d, Some(Path::new("/nexiste/pas")));
        assert_eq!(s.parent(), Some(d.as_path()), "pas de XDG_RUNTIME_DIR utilisable : le dossier des données");
        assert_eq!(l, s.with_extension("lock"));
        let (s2, _) = paths(&d, Some(&std::env::temp_dir()));
        assert_eq!(s2.parent(), Some(std::env::temp_dir().as_path()));
        assert_eq!(s.file_name(), s2.file_name(), "même empreinte pour le même dossier");
        let other = tmp("autre");
        assert_ne!(paths(&other, None).0.file_name(), s.file_name());
        // Un chemin trop long pour une prise passe dans le dossier temporaire.
        let long = d.join("x".repeat(120));
        std::fs::create_dir_all(&long).unwrap();
        let (s3, _) = paths(&long, None);
        assert!(s3.as_os_str().len() <= SUN_PATH_MAX && s3.starts_with(std::env::temp_dir()));
        assert_eq!(fnv1a(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a(b"a"), 0xaf63_dc4c_8601_ec8c);
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::remove_dir_all(&other);
    }

    #[test]
    fn la_seconde_instance_passe_le_lien() {
        let d = tmp("passe");
        let (sock, lock) = (d.join("a.sock"), d.join("a.lock"));
        // Une prise restée d'un plantage n'empêche pas de démarrer.
        drop(UnixListener::bind(&sock).unwrap());
        let Claim::Primary(primary) = claim_at(&sock, &lock, None) else { panic!("instance principale attendue") };
        let (tx, rx) = std::sync::mpsc::channel();
        let Primary { listener, _lock } = primary;
        std::thread::spawn(move || {
            for conn in listener.incoming().take(3) {
                tx.send(answer(conn.unwrap())).unwrap();
            }
        });
        assert!(matches!(claim_at(&sock, &lock, Some("irc://127.0.0.1:16667/#lane-d1")), Claim::Handed));
        assert!(matches!(rx.recv().unwrap(), Some(UserEvent::OpenUrl(u)) if u == "irc://127.0.0.1:16667/#lane-d1"));
        assert!(matches!(claim_at(&sock, &lock, None), Claim::Handed));
        assert!(matches!(rx.recv().unwrap(), Some(UserEvent::Raise)));
        // Une demande invalide est refusée : la seconde instance démarre alors seule.
        assert!(matches!(claim_at(&sock, &lock, Some("http://exemple.org")), Claim::Alone));
        assert!(rx.recv().unwrap().is_none());
        drop(_lock);
        // Le verrou relâché, la suivante devient principale.
        assert!(matches!(claim_at(&sock, &lock, None), Claim::Primary(_)));
        let _ = std::fs::remove_dir_all(&d);
    }
}
