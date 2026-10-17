//! `alpirc-relay` : le programme du relais d'AlpIRC.
//!
//! ```text
//! alpirc-relay [--config <relay.toml>] [--data <dossier>]      lance le relais
//! alpirc-relay --init                                          réglages commentés, certificat, empreinte
//! alpirc-relay --fingerprint                                   empreinte SHA-256 du certificat
//! alpirc-relay --add-user <nom> | --passwd <nom>               mot de passe lu sur l'entrée standard
//! alpirc-relay --version
//! ```
//!
//! Par défaut : `/etc/alpirc-relay/relay.toml` et `/var/lib/alpirc-relay` (le service systemd du paquet les
//! donne explicitement). Journal : variable `RUST_LOG` (`info` par défaut).

use std::path::PathBuf;
use std::process::ExitCode;

use alpirc_relay::{Config, auth, config, ensure_certificate, fingerprint_colons, fingerprint_of_pem};

const USAGE: &str = "\
alpirc-relay — AlpIRC relay (IRC bouncer) / relais d'AlpIRC

Usage:
  alpirc-relay [--config FILE] [--data DIR]   run the relay / lance le relais
  alpirc-relay [...] --init                   write a commented config if missing, create the TLS
                                              certificate, print its fingerprint
  alpirc-relay [...] --fingerprint            print the certificate's SHA-256 fingerprint
  alpirc-relay [...] --add-user NAME          add a user (password read on stdin)
  alpirc-relay [...] --passwd NAME            change a user's password (read on stdin)
  alpirc-relay --version

Defaults: --config /etc/alpirc-relay/relay.toml --data /var/lib/alpirc-relay
Log level: RUST_LOG (info by default).
";

enum Action {
    Run,
    Init,
    Fingerprint,
    AddUser(String),
    Passwd(String),
    Version,
    Help,
}

struct Args {
    config: PathBuf,
    data: PathBuf,
    action: Action,
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut a = Args {
        config: "/etc/alpirc-relay/relay.toml".into(),
        data: "/var/lib/alpirc-relay".into(),
        action: Action::Run,
    };
    let mut it = args.peekable();
    let set = |a: &mut Args, act: Action| -> Result<(), String> {
        if !matches!(a.action, Action::Run) {
            return Err("une seule action à la fois (--init, --fingerprint, --add-user, --passwd)".into());
        }
        a.action = act;
        Ok(())
    };
    while let Some(arg) = it.next() {
        let (name, inline) = match arg.split_once('=') {
            Some((n, v)) if n.starts_with("--") => (n.to_string(), Some(v.to_string())),
            _ => (arg.clone(), None),
        };
        let mut value = |what: &str| -> Result<String, String> {
            inline.clone().or_else(|| it.next()).ok_or_else(|| format!("{name} : {what} attendu"))
        };
        match name.as_str() {
            "--config" | "-c" => a.config = value("fichier")?.into(),
            "--data" | "-d" => a.data = value("dossier")?.into(),
            "--init" => set(&mut a, Action::Init)?,
            "--fingerprint" => set(&mut a, Action::Fingerprint)?,
            "--add-user" => {
                let v = value("nom")?;
                set(&mut a, Action::AddUser(v))?
            }
            "--passwd" => {
                let v = value("nom")?;
                set(&mut a, Action::Passwd(v))?
            }
            "--version" | "-V" => a.action = Action::Version,
            "--help" | "-h" => a.action = Action::Help,
            other => return Err(format!("option inconnue : {other} (voir --help)")),
        }
    }
    Ok(a)
}

/// Coupe ou remet l'écho du terminal (pour taper le mot de passe sans le voir).
fn set_echo(on: bool) -> bool {
    std::process::Command::new("stty")
        .arg(if on { "echo" } else { "-echo" })
        .stdin(std::process::Stdio::inherit())
        .status()
        .is_ok_and(|s| s.success())
}

fn read_line() -> Result<String, String> {
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).map_err(|e| format!("lecture du mot de passe : {e}"))?;
    Ok(s.trim_end_matches(['\r', '\n']).to_string())
}

fn read_password() -> Result<String, String> {
    use std::io::IsTerminal;
    let pw = if std::io::stdin().is_terminal() {
        let ask = |prompt: &str| -> Result<String, String> {
            eprint!("{prompt}");
            let off = set_echo(false);
            let r = read_line();
            if off {
                set_echo(true);
                eprintln!();
            }
            r
        };
        let a = ask("Mot de passe / Password: ")?;
        let b = ask("Encore / Again: ")?;
        if a != b {
            return Err("les deux mots de passe diffèrent / passwords differ".into());
        }
        a
    } else {
        read_line()?
    };
    if pw.is_empty() {
        return Err("mot de passe vide / empty password".into());
    }
    Ok(pw)
}

fn print_fingerprint(fp: &str) {
    println!("SHA-256 : {}", fingerprint_colons(fp));
    println!("          {fp}");
}

/// Attend SIGTERM ou SIGINT puis arrête le relais.
fn watch_signals(stopper: alpirc_relay::Stopper) {
    std::thread::Builder::new()
        .name("alpirc-relay-signals".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(rt) => rt,
                Err(e) => {
                    log::warn!("signaux non surveillés : {e}");
                    return;
                }
            };
            rt.block_on(async {
                #[cfg(unix)]
                {
                    use tokio::signal::unix::{SignalKind, signal};
                    match signal(SignalKind::terminate()) {
                        Ok(mut term) => {
                            tokio::select! {
                                _ = term.recv() => {}
                                _ = tokio::signal::ctrl_c() => {}
                            }
                        }
                        Err(_) => {
                            let _ = tokio::signal::ctrl_c().await;
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    let _ = tokio::signal::ctrl_c().await;
                }
            });
            stopper.stop("Relay shutting down");
        })
        .ok();
}

fn run(args: Args) -> Result<(), String> {
    match args.action {
        Action::Version => {
            println!("alpirc-relay {}", alpirc_relay::VERSION);
            Ok(())
        }
        Action::Help => {
            print!("{USAGE}");
            Ok(())
        }
        Action::Init => {
            if config::write_template_if_missing(&args.config)? {
                println!("Réglages écrits / settings written: {}", args.config.display());
            } else {
                println!("Réglages déjà là / settings already present: {}", args.config.display());
            }
            let cfg = Config::load(&args.config)?;
            std::fs::create_dir_all(&args.data).map_err(|e| format!("{} : {e}", args.data.display()))?;
            let (_, _, fp, created) = ensure_certificate(&cfg, &args.data)?;
            let path = cfg.cert_path(&args.data);
            if created {
                println!("Certificat créé / certificate created: {}", path.display());
            } else {
                println!("Certificat déjà là / certificate already present: {}", path.display());
            }
            print_fingerprint(&fp);
            if cfg.users.is_empty() {
                println!("Ensuite / next: alpirc-relay --config {} --add-user <nom>", args.config.display());
            }
            Ok(())
        }
        Action::Fingerprint => {
            let cfg = Config::load(&args.config).unwrap_or_default();
            let path = cfg.cert_path(&args.data);
            let pem =
                std::fs::read(&path).map_err(|e| format!("{} : {e} (créer le certificat : --init)", path.display()))?;
            print_fingerprint(&fingerprint_of_pem(&pem)?);
            Ok(())
        }
        Action::AddUser(name) | Action::Passwd(name) if !auth::valid_user_name(&name) => {
            Err(format!("nom d'utilisateur invalide : « {name} » (lettres, chiffres, . _ -)"))
        }
        Action::AddUser(name) => {
            config::write_template_if_missing(&args.config)?;
            Config::load(&args.config)?;
            let hash = auth::hash_password(&read_password()?)?;
            config::set_user_password(&args.config, &name, &hash, true)?;
            println!("Utilisateur ajouté / user added: {name} ({})", args.config.display());
            println!("Redémarrer le relais / restart the relay: sudo systemctl restart alpirc-relay");
            Ok(())
        }
        Action::Passwd(name) => {
            Config::load(&args.config)?;
            let hash = auth::hash_password(&read_password()?)?;
            config::set_user_password(&args.config, &name, &hash, false)?;
            println!("Mot de passe changé / password changed: {name}");
            println!("Redémarrer le relais / restart the relay: sudo systemctl restart alpirc-relay");
            Ok(())
        }
        Action::Run => {
            let cfg = Config::load(&args.config)?;
            let handle = alpirc_relay::start(cfg, &args.data)?;
            log::info!(
                "alpirc-relay {} — certificat SHA-256 {}",
                alpirc_relay::VERSION,
                fingerprint_colons(handle.fingerprint())
            );
            watch_signals(handle.stopper());
            handle.wait();
            Ok(())
        }
    }
}

fn main() -> ExitCode {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("alpirc-relay : {e}");
            return ExitCode::from(2);
        }
    };
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("alpirc-relay : {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(v: &[&str]) -> Result<Args, String> {
        parse_args(v.iter().map(|s| s.to_string()))
    }

    #[test]
    fn args() {
        let a = p(&[]).unwrap();
        assert_eq!(a.config, PathBuf::from("/etc/alpirc-relay/relay.toml"));
        assert!(matches!(a.action, Action::Run));
        let a = p(&["--config", "/tmp/r.toml", "--data=/tmp/d", "--add-user", "gui"]).unwrap();
        assert_eq!(a.config, PathBuf::from("/tmp/r.toml"));
        assert_eq!(a.data, PathBuf::from("/tmp/d"));
        assert!(matches!(a.action, Action::AddUser(ref n) if n == "gui"));
        assert!(matches!(p(&["--passwd=gui"]).unwrap().action, Action::Passwd(_)));
        assert!(p(&["--init", "--fingerprint"]).is_err());
        assert!(p(&["--add-user"]).is_err());
        assert!(p(&["--nope"]).is_err());
        assert!(matches!(p(&["--version"]).unwrap().action, Action::Version));
    }
}
