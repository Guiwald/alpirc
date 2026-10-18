//! Paquets Linux (ADR 0002, 0009), fabriqués en Rust (rien à installer, ni `dpkg-deb` ni `tar`) :
//!
//! - `cargo xtask dist-linux [binaire]` → `AlpIRC-X.Y.Z-linux-x86_64.tar.gz` : le programme, les icônes, le fichier
//!   `.desktop` et `install.sh` (installation dans le dossier de l'utilisateur, `--remove` pour retirer) ;
//! - `cargo xtask deb [binaire]` → `alpirc_X.Y.Z_amd64.deb` : Debian, Ubuntu, Mint. Le paquet ajoute lui-même le
//!   dépôt APT d'AlpIRC (clé publique et source), comme Chrome : les versions suivantes arrivent avec
//!   `apt upgrade`. Désinstaller AlpIRC retire aussi le dépôt. Sans la clé publique
//!   (`packaging/linux/alpirc-archive-keyring.gpg`), le paquet est fait sans le dépôt, avec un avertissement.
//! - `cargo xtask deb-relay <binaire>` → `alpirc-relay_X.Y.Z_amd64.deb` : le relais (ADR 0009), service systemd
//!   `alpirc-relay.service` sous l'utilisateur système `alpirc-relay`, réglages dans `/etc/alpirc-relay/relay.toml`,
//!   données dans `/var/lib/alpirc-relay`. Il s'installe depuis le même dépôt APT (`sudo apt install alpirc-relay`).
//!
//! Le binaire par défaut est `target/release/alpirc` (ou `$CARGO_TARGET_DIR/release/alpirc`).

use std::io::Write;
use std::path::{Path, PathBuf};

use md5::{Digest, Md5};

use crate::icons::{LINUX_SIZES, Sources, filter_from_env};
use crate::{Res, Result, dist_dir, refuse_overwrite, target_dir, version, workspace_root};

/// Adresse du dépôt APT (GitHub Pages, publié par le flux « Publication »).
pub const APT_URL: &str = "https://guiwald.github.io/alpirc/apt";

/// Clé publique du dépôt APT (format binaire, pour `Signed-By`), versionnée dans le dépôt.
const KEYRING: &str = "packaging/linux/alpirc-archive-keyring.gpg";

/// Modèle des réglages du relais, installé dans `/etc/alpirc-relay/relay.toml` (fichier de configuration : une
/// version modifiée par l'utilisateur est gardée lors des mises à jour).
const RELAY_CONFIG: &str = "packaging/linux/alpirc-relay/relay.toml";

const MAINTAINER: &str = "Guiwald <guiwald@users.noreply.github.com>";

fn desktop_file(exec: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=AlpIRC\n\
         GenericName=IRC Client\n\
         GenericName[fr]=Client IRC\n\
         GenericName[es]=Cliente IRC\n\
         GenericName[de]=IRC-Client\n\
         GenericName[it]=Client IRC\n\
         GenericName[ca]=Client IRC\n\
         GenericName[ca@valencia]=Client IRC\n\
         GenericName[ga]=Cliant IRC\n\
         Comment=Chat on IRC networks, with the look of the 2000s classics\n\
         Comment[fr]=Discuter sur les réseaux IRC, avec l'allure des classiques des années 2000\n\
         Comment[es]=Chatear en las redes IRC, con el aspecto de los clásicos de los años 2000\n\
         Comment[de]=Auf IRC-Netzwerken chatten, im Stil der Klassiker der 2000er\n\
         Comment[it]=Chattare sulle reti IRC, con l'aspetto dei classici degli anni 2000\n\
         Comment[ca]=Xatejar a les xarxes IRC, amb l'aspecte dels clàssics dels anys 2000\n\
         Comment[ca@valencia]=Xatejar en les xarxes IRC, amb l'aspecte dels clàssics dels anys 2000\n\
         Comment[ga]=Déan comhrá ar líonraí IRC, le cuma na gclasaiceach ó na 2000idí\n\
         Exec={exec} %u\n\
         Icon=alpirc\n\
         Terminal=false\n\
         Categories=Network;Chat;IRCClient;\n\
         MimeType=x-scheme-handler/irc;x-scheme-handler/ircs;\n\
         Keywords=irc;chat;undernet;libera;channel;canal;\n\
         StartupWMClass=alpirc\n\
         SingleMainWindow=true\n"
    )
}

fn install_sh() -> String {
    let sizes = LINUX_SIZES.iter().map(u32::to_string).collect::<Vec<_>>().join(" ");
    format!(
        r#"#!/bin/sh
# AlpIRC — installation dans le dossier de l'utilisateur (aucun mot de passe demandé).
#   ./install.sh           installe (ou met à jour) : ~/.local/bin/alpirc, menu des applications, icône,
#                          liens irc:// et ircs://
#   ./install.sh --remove  retire AlpIRC (vos réglages et vos journaux restent dans ~/.config/alpirc)
# AlpIRC — installs into your home folder (no password needed). `./install.sh --remove` uninstalls it.
set -e
cd "$(dirname "$0")"
DATA="${{XDG_DATA_HOME:-$HOME/.local/share}}"
BIN="$HOME/.local/bin"
if [ "$1" = "--remove" ]; then
    rm -f "$BIN/alpirc" "$DATA/applications/alpirc.desktop"
    for s in {sizes}; do rm -f "$DATA/icons/hicolor/${{s}}x${{s}}/apps/alpirc.png"; done
    command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q "$DATA/applications" || true
    echo "AlpIRC retiré. / AlpIRC removed."
    exit 0
fi
mkdir -p "$BIN" "$DATA/applications"
install -m 755 alpirc "$BIN/alpirc"
for s in {sizes}; do
    mkdir -p "$DATA/icons/hicolor/${{s}}x${{s}}/apps"
    install -m 644 "icons/alpirc-$s.png" "$DATA/icons/hicolor/${{s}}x${{s}}/apps/alpirc.png"
done
sed "s|^Exec=alpirc|Exec=$BIN/alpirc|" alpirc.desktop > "$DATA/applications/alpirc.desktop"
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q "$DATA/applications" || true
# Liens irc:// et ircs:// : AlpIRC les ouvre, sauf si un autre programme en a déjà la charge.
if command -v xdg-mime >/dev/null 2>&1; then
    for u in irc ircs; do
        [ -n "$(xdg-mime query default x-scheme-handler/$u 2>/dev/null)" ] || xdg-mime default alpirc.desktop "x-scheme-handler/$u" || true
    done
fi
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t "$DATA/icons/hicolor" 2>/dev/null || true
echo "AlpIRC est installé : cherchez-le dans le menu des applications, ou lancez « alpirc »."
echo "AlpIRC is installed: look for it in your applications menu, or run \"alpirc\"."
"#
    )
}

const README: &str = "AlpIRC {v} — Linux (x86_64)\n\n\
FR : Décompresser l'archive, puis lancer ./install.sh (installation dans votre dossier, sans mot de passe) ;\n\
AlpIRC apparaît dans le menu des applications. On peut aussi lancer ./alpirc directement.\n\
Debian, Ubuntu, Mint : préférez le paquet .deb (mises à jour avec le système), voir la page des versions.\n\
Pour les boîtes « Ouvrir un fichier », le bureau doit avoir xdg-desktop-portal (GNOME, KDE…) ou zenity.\n\n\
EN: Unpack the archive, then run ./install.sh (installs into your home folder, no password);\n\
AlpIRC shows up in your applications menu. You can also run ./alpirc directly.\n\
Debian, Ubuntu, Mint: prefer the .deb package (updated with the system), see the releases page.\n\
For \"Open file\" dialogs, the desktop needs xdg-desktop-portal (GNOME, KDE…) or zenity.\n\n\
https://github.com/Guiwald/alpirc\n";

fn binary(arg: Option<PathBuf>, name: &str, package: &str) -> Result<PathBuf> {
    let p = arg.unwrap_or_else(|| target_dir().join("release").join(name));
    if !p.is_file() {
        return Err(format!("{} introuvable : cargo build --release -p {package} d'abord", p.display()).into());
    }
    Ok(p)
}

/// Une archive .tar.gz en mémoire : fichiers (chemin, droits, contenu) et dossiers créés au passage.
struct TarGz {
    builder: tar::Builder<flate2::write::GzEncoder<Vec<u8>>>,
    dirs: std::collections::BTreeSet<String>,
    mtime: u64,
}

impl TarGz {
    fn new() -> TarGz {
        let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
        let mut builder = tar::Builder::new(gz);
        builder.mode(tar::HeaderMode::Deterministic);
        // Date fixe : minuit du jour, les paquets d'une même journée sont identiques octet pour octet.
        let mtime = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() / 86_400 * 86_400).unwrap_or(0);
        TarGz { builder, dirs: Default::default(), mtime }
    }

    fn dir(&mut self, path: &str) -> Res {
        let path = path.trim_end_matches('/');
        if path.is_empty() || path == "." || !self.dirs.insert(path.to_string()) {
            return Ok(());
        }
        if let Some((parent, _)) = path.rsplit_once('/') {
            self.dir(parent)?;
        }
        let mut h = tar::Header::new_gnu();
        h.set_entry_type(tar::EntryType::Directory);
        h.set_mode(0o755);
        h.set_size(0);
        h.set_mtime(self.mtime);
        h.set_uid(0);
        h.set_gid(0);
        self.builder.append_data(&mut h, format!("{path}/"), std::io::empty())?;
        Ok(())
    }

    fn file(&mut self, path: &str, mode: u32, data: &[u8]) -> Res {
        if let Some((parent, _)) = path.rsplit_once('/') {
            self.dir(parent)?;
        }
        let mut h = tar::Header::new_gnu();
        h.set_entry_type(tar::EntryType::Regular);
        h.set_mode(mode);
        h.set_size(data.len() as u64);
        h.set_mtime(self.mtime);
        h.set_uid(0);
        h.set_gid(0);
        self.builder.append_data(&mut h, path, data)?;
        Ok(())
    }

    fn finish(self) -> Result<Vec<u8>> {
        Ok(self.builder.into_inner()?.finish()?)
    }
}

/// `cargo xtask dist-linux` : l'archive .tar.gz.
pub fn dist_tar(bin: Option<PathBuf>) -> Res {
    let root = workspace_root();
    let v = version()?;
    let bin = binary(bin, "alpirc", "alpirc")?;
    let out = dist_dir()?.join(format!("AlpIRC-{v}-linux-x86_64.tar.gz"));
    refuse_overwrite(&out)?;
    let icons = Sources::load(filter_from_env())?;
    let d = format!("AlpIRC-{v}");
    let mut t = TarGz::new();
    t.file(&format!("{d}/alpirc"), 0o755, &std::fs::read(&bin)?)?;
    t.file(&format!("{d}/install.sh"), 0o755, install_sh().as_bytes())?;
    t.file(&format!("{d}/alpirc.desktop"), 0o644, desktop_file("alpirc").as_bytes())?;
    for s in LINUX_SIZES {
        t.file(&format!("{d}/icons/alpirc-{s}.png"), 0o644, &icons.png(s)?)?;
    }
    t.file(&format!("{d}/LISEZMOI-README.txt"), 0o644, README.replace("{v}", &v).as_bytes())?;
    t.file(&format!("{d}/LICENSE.txt"), 0o644, &std::fs::read(root.join("LICENSE"))?)?;
    if let Ok(tp) = std::fs::read(root.join("THIRD_PARTY.md")) {
        t.file(&format!("{d}/THIRD_PARTY.md"), 0o644, &tp)?;
    }
    std::fs::write(&out, t.finish()?)?;
    println!("{}", out.display());
    Ok(())
}

/// Source APT au format deb822 (`/etc/apt/sources.list.d/alpirc.sources`).
pub fn apt_source() -> String {
    format!("Types: deb\nURIs: {APT_URL}\nSuites: stable\nComponents: main\nArchitectures: amd64\nSigned-By: /usr/share/keyrings/alpirc-archive-keyring.gpg\n")
}

/// Un fichier installé par un paquet .deb (chemin sans « / » initial).
struct DebFile {
    path: String,
    mode: u32,
    data: Vec<u8>,
}

fn f(path: &str, mode: u32, data: impl Into<Vec<u8>>) -> DebFile {
    DebFile { path: path.to_string(), mode, data: data.into() }
}

/// Champs de contrôle d'un paquet (le reste est calculé : version, taille installée).
struct Control<'a> {
    package: &'a str,
    depends: &'a str,
    recommends: &'a str,
    suggests: &'a str,
    description: &'a str,
}

fn control_text(c: &Control, v: &str, installed_kb: u64) -> String {
    let mut s = format!(
        "Package: {}\nVersion: {v}\nArchitecture: amd64\nMaintainer: {MAINTAINER}\nInstalled-Size: {installed_kb}\n",
        c.package
    );
    for (k, val) in [("Depends", c.depends), ("Recommends", c.recommends), ("Suggests", c.suggests)] {
        if !val.is_empty() {
            s.push_str(&format!("{k}: {val}\n"));
        }
    }
    s.push_str("Section: net\nPriority: optional\nHomepage: https://github.com/Guiwald/alpirc\n");
    s.push_str(&format!("Description: {}\n", c.description.trim_end()));
    s
}

/// Fabrique un paquet .deb (format « ar » : `debian-binary`, `control.tar.gz`, `data.tar.gz`).
/// `conffiles` : fichiers de configuration (gardés s'ils ont été modifiés) ; `scripts` : (nom, contenu).
fn write_deb(out: &Path, control: &Control, v: &str, files: &[DebFile], conffiles: &[&str], scripts: &[(&str, &str)]) -> Res {
    let installed_kb: u64 = files.iter().map(|f| f.data.len() as u64).sum::<u64>().div_ceil(1024);
    let mut data = TarGz::new();
    let mut md5sums = String::new();
    for file in files {
        data.file(&format!("./{}", file.path), file.mode, &file.data)?;
        // Les fichiers de /etc sont suivis à part par dpkg : pas dans md5sums.
        if !file.path.starts_with("etc/") {
            let sum = Md5::digest(&file.data);
            let hex: String = sum.iter().map(|b| format!("{b:02x}")).collect();
            md5sums.push_str(&format!("{hex}  {}\n", file.path));
        }
    }
    let mut ctl = TarGz::new();
    ctl.file("./control", 0o644, control_text(control, v, installed_kb).as_bytes())?;
    ctl.file("./md5sums", 0o644, md5sums.as_bytes())?;
    if !conffiles.is_empty() {
        let list: String = conffiles.iter().map(|c| format!("{c}\n")).collect();
        ctl.file("./conffiles", 0o644, list.as_bytes())?;
    }
    for (name, body) in scripts {
        ctl.file(&format!("./{name}"), 0o755, body.as_bytes())?;
    }
    let mut ar = Vec::new();
    ar.extend_from_slice(b"!<arch>\n");
    ar_member(&mut ar, "debian-binary", b"2.0\n")?;
    ar_member(&mut ar, "control.tar.gz", &ctl.finish()?)?;
    ar_member(&mut ar, "data.tar.gz", &data.finish()?)?;
    std::fs::write(out, ar)?;
    Ok(())
}

/// `cargo xtask deb` : le paquet de l'appli.
pub fn deb(bin: Option<PathBuf>) -> Res {
    let root = workspace_root();
    let v = version()?;
    let bin = binary(bin, "alpirc", "alpirc")?;
    let out = dist_dir()?.join(format!("alpirc_{v}_amd64.deb"));
    refuse_overwrite(&out)?;
    let keyring = root.join(KEYRING);
    let icons = Sources::load(filter_from_env())?;
    let mut files = vec![
        f("usr/bin/alpirc", 0o755, std::fs::read(&bin)?),
        f("usr/share/applications/alpirc.desktop", 0o644, desktop_file("alpirc")),
        f("usr/share/doc/alpirc/copyright", 0o644, copyright(&root, "AlpIRC")?),
    ];
    for s in LINUX_SIZES {
        files.push(f(&format!("usr/share/icons/hicolor/{s}x{s}/apps/alpirc.png"), 0o644, icons.png(s)?));
    }
    // Le dépôt APT, si sa clé publique est là (sinon, un paquet sans mises à jour automatiques).
    let with_repo = keyring.is_file();
    if with_repo {
        files.push(f("usr/share/keyrings/alpirc-archive-keyring.gpg", 0o644, std::fs::read(&keyring)?));
        files.push(f("etc/apt/sources.list.d/alpirc.sources", 0o644, apt_source()));
    } else {
        eprintln!("attention : {} absent : le paquet n'ajoutera pas le dépôt APT (pas de mises à jour automatiques)", keyring.display());
    }
    let control = Control {
        package: "alpirc",
        depends: "libc6 (>= 2.35), libasound2t64 | libasound2, libx11-6, libxcursor1, libxrandr2, libxi6, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0",
        recommends: "xdg-desktop-portal, libdbus-1-3, fonts-noto-color-emoji",
        suggests: "zenity",
        description: "IRC client with the look of the 2000s classics\n \
AlpIRC is an IRC client rewritten from scratch in Rust, with the layout of the\n \
Windows chat clients of the 2000s: windows inside a main window, switchbar,\n \
nickname list, fixed-width font, mIRC colour codes and mIRC-compatible scripts\n \
(sandboxed). Several networks at once, TLS, SASL, IRCv3, DCC, logs, and\n \
Undernet, Libera.Chat and other networks preconfigured. Installing this package\n \
also adds the AlpIRC APT repository, so new versions arrive with your system\n \
updates.\n",
    };
    // La source APT n'est pas un fichier de configuration : elle part avec le paquet (« apt remove »).
    let postrm = "#!/bin/sh\nset -e\nif [ \"$1\" = purge ]; then rm -f /etc/apt/sources.list.d/alpirc.sources; fi\nexit 0\n";
    let scripts: Vec<(&str, &str)> = if with_repo { vec![("postrm", postrm)] } else { vec![] };
    write_deb(&out, &control, &v, &files, &[], &scripts)?;
    println!("{}", out.display());
    Ok(())
}

/// Service systemd du relais. Contrat avec `alpirc-relay` : `--config <fichier>` et `--data <dossier>`.
pub const RELAY_UNIT: &str = "[Unit]
Description=AlpIRC relay (IRC bouncer)
Documentation=https://github.com/Guiwald/alpirc
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=alpirc-relay
Group=alpirc-relay
ExecStart=/usr/bin/alpirc-relay --config /etc/alpirc-relay/relay.toml --data /var/lib/alpirc-relay
Restart=on-failure
RestartSec=5
# /var/lib/alpirc-relay, créé par systemd au besoin, à l'utilisateur du relais.
StateDirectory=alpirc-relay
StateDirectoryMode=0750
# Durcissement : le relais ne touche qu'à ses données et au réseau.
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
PrivateDevices=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectControlGroups=yes
ProtectClock=yes
ProtectHostname=yes
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
RestrictNamespaces=yes
RestrictRealtime=yes
LockPersonality=yes
MemoryDenyWriteExecute=yes
SystemCallArchitectures=native
# Pour écouter sur un port inférieur à 1024 (443…), décommenter dans « systemctl edit alpirc-relay » :
#AmbientCapabilities=CAP_NET_BIND_SERVICE

[Install]
WantedBy=multi-user.target
";

const RELAY_POSTINST: &str = r#"#!/bin/sh
set -e
if [ "$1" = configure ]; then
    if ! getent passwd alpirc-relay >/dev/null; then
        adduser --system --group --home /var/lib/alpirc-relay --no-create-home --quiet alpirc-relay
    fi
    mkdir -p /var/lib/alpirc-relay
    chown alpirc-relay:alpirc-relay /var/lib/alpirc-relay
    chmod 0750 /var/lib/alpirc-relay
    # Les réglages peuvent contenir des mots de passe : lisibles par root et le relais seulement.
    chown root:alpirc-relay /etc/alpirc-relay /etc/alpirc-relay/relay.toml
    chmod 0750 /etc/alpirc-relay
    chmod 0640 /etc/alpirc-relay/relay.toml
    if [ -d /run/systemd/system ]; then
        systemctl daemon-reload >/dev/null 2>&1 || true
        if [ -n "$2" ]; then
            # Mise à jour : le relais est relancé s'il tournait.
            systemctl try-restart alpirc-relay.service >/dev/null 2>&1 || true
        else
            echo "Relais d'AlpIRC installé. Régler /etc/alpirc-relay/relay.toml, puis : sudo systemctl enable --now alpirc-relay"
            echo "AlpIRC relay installed. Edit /etc/alpirc-relay/relay.toml, then: sudo systemctl enable --now alpirc-relay"
        fi
    fi
fi
exit 0
"#;

const RELAY_PRERM: &str = r#"#!/bin/sh
set -e
if [ "$1" = remove ] && [ -d /run/systemd/system ]; then
    systemctl stop alpirc-relay.service >/dev/null 2>&1 || true
    systemctl disable alpirc-relay.service >/dev/null 2>&1 || true
fi
exit 0
"#;

// L'historique des conversations n'est jamais effacé, même par « apt purge » : on le retire à la main.
const RELAY_POSTRM: &str = r#"#!/bin/sh
set -e
if [ -d /run/systemd/system ]; then
    systemctl daemon-reload >/dev/null 2>&1 || true
fi
if [ "$1" = purge ] && [ -d /var/lib/alpirc-relay ]; then
    echo "L'historique du relais reste dans /var/lib/alpirc-relay (à effacer à la main si besoin)."
    echo "The relay's history stays in /var/lib/alpirc-relay (delete it by hand if needed)."
fi
exit 0
"#;

const RELAY_README: &str = "AlpIRC relay (alpirc-relay) — Debian, Ubuntu\n\n\
FR : le relais garde vos connexions IRC ouvertes et l'historique, pour que le téléphone et l'ordinateur\n\
partagent la même session (ADR 0009). Réglages : /etc/alpirc-relay/relay.toml (lisible par root et le relais).\n\
Données : /var/lib/alpirc-relay. Service : sudo systemctl enable --now alpirc-relay ;\n\
journal : journalctl -u alpirc-relay. Après une modification des réglages : sudo systemctl restart alpirc-relay.\n\n\
EN: the relay keeps your IRC connections open and stores the history, so that your phone and your computer share\n\
the same session. Settings: /etc/alpirc-relay/relay.toml (readable by root and the relay only). Data:\n\
/var/lib/alpirc-relay. Service: sudo systemctl enable --now alpirc-relay; log: journalctl -u alpirc-relay.\n\
After changing the settings: sudo systemctl restart alpirc-relay.\n\n\
https://github.com/Guiwald/alpirc\n";

/// `cargo xtask deb-relay <binaire>` : le paquet du relais.
pub fn deb_relay(bin: Option<PathBuf>) -> Res {
    let root = workspace_root();
    let v = version()?;
    let bin = binary(bin, "alpirc-relay", "alpirc-relay")?;
    let out = dist_dir()?.join(format!("alpirc-relay_{v}_amd64.deb"));
    refuse_overwrite(&out)?;
    let config_path = root.join(RELAY_CONFIG);
    let config = std::fs::read(&config_path).map_err(|e| format!("{} : {e}", config_path.display()))?;
    let files = vec![
        f("usr/bin/alpirc-relay", 0o755, std::fs::read(&bin)?),
        f("usr/lib/systemd/system/alpirc-relay.service", 0o644, RELAY_UNIT),
        f("etc/alpirc-relay/relay.toml", 0o640, config),
        f("usr/share/doc/alpirc-relay/README.Debian", 0o644, RELAY_README),
        f("usr/share/doc/alpirc-relay/copyright", 0o644, copyright(&root, "AlpIRC relay")?),
    ];
    let control = Control {
        package: "alpirc-relay",
        depends: "libc6 (>= 2.35), adduser",
        recommends: "",
        suggests: "",
        description: "relay (IRC bouncer) for AlpIRC\n \
alpirc-relay stays connected to your IRC networks under a single nickname,\n \
keeps the history, and lets several devices (AlpIRC on the computer and on the\n \
phone) share the same session. It also speaks the soju bouncer extensions\n \
(bouncer-networks, chathistory, read-marker), so other IRC clients can use it.\n \
Runs as the systemd service alpirc-relay.service.\n",
    };
    write_deb(
        &out,
        &control,
        &v,
        &files,
        &["/etc/alpirc-relay/relay.toml"],
        &[("postinst", RELAY_POSTINST), ("prerm", RELAY_PRERM), ("postrm", RELAY_POSTRM)],
    )?;
    println!("{}", out.display());
    Ok(())
}

/// Un membre d'archive « ar » (en-tête de 60 octets, contenu aligné sur 2 octets).
fn ar_member(out: &mut Vec<u8>, name: &str, data: &[u8]) -> Res {
    let mtime = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    writeln!(out, "{:<16}{:<12}{:<6}{:<6}{:<8}{:<10}`", name, mtime, 0, 0, "100644", data.len())?;
    out.extend_from_slice(data);
    if data.len() % 2 == 1 {
        out.push(b'\n');
    }
    Ok(())
}

fn copyright(root: &Path, name: &str) -> Result<String> {
    let license = std::fs::read_to_string(root.join("LICENSE"))?;
    let indented: String = license.lines().map(|l| if l.trim().is_empty() { " .\n".to_string() } else { format!(" {l}\n") }).collect();
    Ok(format!(
        "Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/\n\
         Upstream-Name: {name}\n\
         Source: https://github.com/Guiwald/alpirc\n\n\
         Files: *\n\
         Copyright: Guiwald\n\
         License: MIT\n{indented}\n\
         Comment: Third-party components and their licenses are listed in THIRD_PARTY.md in the source repository\n \
         (crates: cargo xtask licenses).\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fichier_desktop() {
        let d = desktop_file("alpirc");
        assert!(d.contains("\nExec=alpirc %u\n"));
        assert!(d.contains("\nMimeType=x-scheme-handler/irc;x-scheme-handler/ircs;\n"));
        assert!(d.contains("\nStartupWMClass=alpirc\n"));
        // Une seule fenêtre : la seconde instance passe la main à la première (app/desktop/src/instance.rs).
        assert!(d.contains("\nSingleMainWindow=true\n"));
        assert!(d.contains("\nIcon=alpirc\n"));
        // Pas d'espace en tête de ligne (les « \ » de fin de ligne les mangent).
        assert!(d.lines().all(|l| !l.starts_with(' ')));
    }

    #[test]
    fn installeur() {
        let s = install_sh();
        assert!(s.contains("for s in 16 24 32 48 64 128 256 512; do"));
        assert!(s.contains("${s}x${s}"));
        assert!(s.contains("s|^Exec=alpirc|Exec=$BIN/alpirc|"));
        assert!(s.contains("xdg-mime default alpirc.desktop \"x-scheme-handler/$u\""));
    }

    #[test]
    fn source_apt() {
        let s = apt_source();
        assert!(s.contains("URIs: https://guiwald.github.io/alpirc/apt\n"));
        assert!(s.contains("Signed-By: /usr/share/keyrings/alpirc-archive-keyring.gpg\n"));
    }

    #[test]
    fn controle() {
        let c = Control { package: "alpirc-relay", depends: "libc6 (>= 2.35)", recommends: "", suggests: "", description: "court\n long\n" };
        let t = control_text(&c, "1.2.3", 42);
        assert!(t.starts_with("Package: alpirc-relay\nVersion: 1.2.3\nArchitecture: amd64\n"));
        assert!(t.contains("Installed-Size: 42\nDepends: libc6 (>= 2.35)\nSection: net\n"));
        assert!(!t.contains("Recommends"));
        assert!(t.ends_with("Description: court\n long\n"));
    }

    #[test]
    fn service_systemd() {
        assert!(RELAY_UNIT.contains("\nUser=alpirc-relay\n"));
        assert!(RELAY_UNIT.contains("--config /etc/alpirc-relay/relay.toml --data /var/lib/alpirc-relay"));
        assert!(RELAY_UNIT.contains("\nWantedBy=multi-user.target\n"));
    }
}
