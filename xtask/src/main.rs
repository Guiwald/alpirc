//! Tâches de développement d'AlpIRC : `cargo xtask <commande>` (voir `cargo xtask help`).
//!
//! Emplacements :
//! - racine du dépôt : le dossier parent de `xtask/` (ou `ALPIRC_ROOT`, pour essayer une commande sur une copie) ;
//! - fichiers produits : `dist/` (ou `ALPIRC_DIST`) ; les commandes de paquet refusent d'écraser un fichier ;
//! - compilation : `target/` (ou `CARGO_TARGET_DIR`).

mod apk;
mod changelog_page;
mod icons;
mod licenses;
mod linux;

use std::path::{Path, PathBuf};

const HELP: &str = "\
Outillage d'AlpIRC — cargo xtask <commande>

Versions
  bump patch|minor|major [--dry-run]  nouvelle version : VERSION, Cargo.toml (workspace.package),
                                      sections datées de CHANGELOG.md et CHANGELOG.en.md
  changelog-page [sortie.html]        page « Versions d'AlpIRC » tirée de CHANGELOG.md (défaut : dist/versions.html)

Icônes (source : assets/icons/app/alpirc-<taille>.png, la plus grande, idéalement 1024 ; sinon icône provisoire)
  icons [dossier] [--nearest]         AlpIRC.icns, PNG Linux 16 à 512, mipmaps Android
                                      (défaut : assets/icons/generated/) ; --nearest : plus proche voisin pour les
                                      tailles qui divisent exactement la source (sinon : moyenne par zones)

Paquets (dans dist/ ou $ALPIRC_DIST ; un fichier déjà produit n'est jamais écrasé)
  bundle-mac <binaire> [dossier]      AlpIRC.app, et AlpIRC-X.Y.Z.dmg sur macOS (hdiutil)
  apk [--all-abis]                    AlpIRC-X.Y.Z.apk signé, sans Gradle (SDK, NDK, JDK) ;
                                      --all-abis ajoute x86_64 (émulateur sur PC ou Mac Intel)
  dist-linux [binaire]                AlpIRC-X.Y.Z-linux-x86_64.tar.gz (défaut : target/release/alpirc)
  deb [binaire]                       alpirc_X.Y.Z_amd64.deb (défaut : target/release/alpirc)
  deb-relay <binaire>                 alpirc-relay_X.Y.Z_amd64.deb (relais, service systemd)

Divers
  licenses [paquet…]                  licences des crates tierces de alpirc, alpirc-android et alpirc-relay
                                      (ou des paquets donnés), en tableau Markdown sur la sortie standard
  help                                cette aide

Variables : ALPIRC_DIST (dossier des paquets), ALPIRC_ROOT (racine du dépôt), CARGO_TARGET_DIR,
ALPIRC_KEYSTORE + ALPIRC_KEYSTORE_PASS (+ ALPIRC_KEY_ALIAS) pour signer l'APK, ANDROID_HOME, ANDROID_NDK_HOME,
JAVA_HOME, ALPIRC_APK_LIB (APK avec une libalpirc.so arm64 déjà compilée), ALPIRC_ICON_FILTER=nearest
(comme --nearest, pour toutes les commandes).";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("help");
    let rest: Vec<&str> = args.iter().skip(1).map(String::as_str).collect();
    let flag = |f: &str| rest.contains(&f);
    let positional: Vec<&str> = rest.iter().copied().filter(|a| !a.starts_with("--")).collect();
    let arg = |i: usize| positional.get(i).map(PathBuf::from);
    let res = match cmd {
        "bump" => bump(positional.first().copied(), flag("--dry-run")),
        "changelog-page" => changelog(arg(0)),
        "icons" => icons::icons_cmd(arg(0), flag("--nearest")),
        "bundle-mac" => bundle_mac(arg(0), arg(1)),
        "apk" => apk::build_apk(flag("--all-abis")),
        "dist-linux" => linux::dist_tar(arg(0)),
        "deb" => linux::deb(arg(0)),
        "deb-relay" => linux::deb_relay(arg(0)),
        "licenses" => licenses::licenses(&positional),
        "help" | "--help" | "-h" => {
            println!("{HELP}");
            Ok(())
        }
        other => {
            eprintln!("commande inconnue : {other}\n\n{HELP}");
            std::process::exit(2);
        }
    };
    if let Err(e) = res {
        eprintln!("erreur : {e}");
        std::process::exit(1);
    }
}

pub type Res = Result<()>;
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Racine du dépôt : `ALPIRC_ROOT` si défini (essais sur une copie), sinon le dossier parent de `xtask/`.
pub fn workspace_root() -> PathBuf {
    if let Some(r) = std::env::var_os("ALPIRC_ROOT").filter(|r| !r.is_empty()) {
        return PathBuf::from(r);
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

/// Dossier des paquets : `ALPIRC_DIST` si défini, sinon `dist/` à la racine. Créé au besoin.
pub fn dist_dir() -> Result<PathBuf> {
    let d = match std::env::var_os("ALPIRC_DIST").filter(|d| !d.is_empty()) {
        Some(d) => PathBuf::from(d),
        None => workspace_root().join("dist"),
    };
    std::fs::create_dir_all(&d)?;
    Ok(d)
}

/// Dossier de compilation de cargo : `CARGO_TARGET_DIR` si défini (relatif : à la racine), sinon `target/`.
pub fn target_dir() -> PathBuf {
    let root = workspace_root();
    match std::env::var_os("CARGO_TARGET_DIR").filter(|d| !d.is_empty()) {
        Some(d) if Path::new(&d).is_absolute() => PathBuf::from(d),
        Some(d) => root.join(d),
        None => root.join("target"),
    }
}

/// Version courante (fichier `VERSION`, source unique).
pub fn version() -> Result<String> {
    let p = workspace_root().join("VERSION");
    let v = std::fs::read_to_string(&p).map_err(|e| format!("{} : {e}", p.display()))?.trim().to_string();
    parse_semver(&v).ok_or_else(|| format!("VERSION : « {v} » n'est pas de la forme X.Y.Z"))?;
    Ok(v)
}

pub fn parse_semver(v: &str) -> Option<(u32, u32, u32)> {
    let p: Vec<u32> = v.trim().split('.').map(|x| x.parse().ok()).collect::<Option<_>>()?;
    (p.len() == 3).then(|| (p[0], p[1], p[2]))
}

/// Numéro technique dérivé de la version : versionCode Android et CFBundleVersion macOS (MAJ×10000+MIN×100+CORR).
pub fn version_code(v: &str) -> u32 {
    let (a, b, c) = parse_semver(v).unwrap_or((0, 0, 0));
    a * 10_000 + b * 100 + c
}

/// Refuse d'écraser une version déjà produite (une version publiée ne change jamais).
pub fn refuse_overwrite(p: &Path) -> Res {
    if p.exists() {
        return Err(format!("{} existe déjà : une version produite ne change jamais (cargo xtask bump …)", p.display()).into());
    }
    Ok(())
}

/// Lance une commande et échoue si elle échoue (message clair si le programme est introuvable).
pub fn run(cmd: &mut std::process::Command) -> Res {
    let st = cmd.status().map_err(|e| format!("impossible de lancer {:?} : {e}", cmd.get_program()))?;
    if !st.success() {
        return Err(format!("échec ({st}) : {cmd:?}").into());
    }
    Ok(())
}

/// Date du jour (UTC) en AAAA-MM-JJ.
pub fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    date_from_days((secs / 86_400) as i64)
}

/// Jours depuis le 1er janvier 1970 → AAAA-MM-JJ (calendrier grégorien).
fn date_from_days(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02}")
}

// ---------------------------------------------------------------------------------------------------------------
// Versions

/// Version suivante selon le genre de changement (convention : docs/versions.md).
fn next_version(old: &str, kind: &str) -> Result<String> {
    let (a, b, c) = parse_semver(old).ok_or_else(|| format!("version actuelle illisible : {old}"))?;
    Ok(match kind {
        "major" => format!("{}.0.0", a + 1),
        "minor" => format!("{a}.{}.0", b + 1),
        "patch" => format!("{a}.{b}.{}", c + 1),
        _ => return Err("bump patch|minor|major".into()),
    })
}

/// Remplace la version de la section `[workspace.package]` du Cargo.toml racine.
fn set_workspace_version(toml: &str, new: &str) -> Result<String> {
    let mut in_section = false;
    let mut done = false;
    let mut out = Vec::new();
    for line in toml.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_section = t == "[workspace.package]";
        } else if in_section && !done && t.starts_with("version") && t[7..].trim_start().starts_with('=') {
            out.push(format!("version = \"{new}\""));
            done = true;
            continue;
        }
        out.push(line.to_string());
    }
    if !done {
        return Err("Cargo.toml : pas de « version = … » dans [workspace.package]".into());
    }
    let mut s = out.join("\n");
    if toml.ends_with('\n') {
        s.push('\n');
    }
    Ok(s)
}

/// Ajoute, sous le titre « non publié » d'un journal, le titre daté de la nouvelle version : les entrées en
/// attente passent ainsi dans la nouvelle version, et une section « non publié » vide reste en haut.
fn date_changelog(text: &str, unreleased: &str, new: &str, date: &str) -> Result<String> {
    if text.contains(&format!("## [{new}]")) {
        return Err(format!("le journal a déjà une section [{new}]").into());
    }
    let pos = text.lines().position(|l| l.trim_end() == unreleased).ok_or_else(|| format!("titre « {unreleased} » introuvable"))?;
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    lines.insert(pos + 1, String::new());
    lines.insert(pos + 2, format!("## [{new}] - {date}"));
    let mut s = lines.join("\n");
    if text.ends_with('\n') {
        s.push('\n');
    }
    Ok(s)
}

/// `bump patch|minor|major [--dry-run]` : VERSION, `[workspace.package] version` (toutes les crates ont
/// `version.workspace = true`), titres datés dans CHANGELOG.md et CHANGELOG.en.md.
fn bump(kind: Option<&str>, dry_run: bool) -> Res {
    let root = workspace_root();
    let old = version()?;
    let new = next_version(&old, kind.unwrap_or(""))?;
    let date = today();
    let cargo_path = root.join("Cargo.toml");
    let cargo = set_workspace_version(&std::fs::read_to_string(&cargo_path)?, &new)?;
    let mut journals = Vec::new();
    for (name, unreleased) in [("CHANGELOG.md", "## [Non publié]"), ("CHANGELOG.en.md", "## [Unreleased]")] {
        let p = root.join(name);
        let t = std::fs::read_to_string(&p).map_err(|e| format!("{} : {e}", p.display()))?;
        journals.push((p, date_changelog(&t, unreleased, &new, &date).map_err(|e| format!("{name} : {e}"))?));
    }
    // Les crates qui n'utilisent pas la version commune ne suivent pas : on le signale.
    for m in members(&root) {
        let f = root.join(&m).join("Cargo.toml");
        if let Ok(t) = std::fs::read_to_string(&f)
            && !t.contains("version.workspace = true") {
                eprintln!("attention : {} n'a pas « version.workspace = true » : sa version ne suit pas", f.display());
            }
    }
    if dry_run {
        println!("{old} → {new} (essai : rien n'est écrit)");
        println!("  VERSION, {}, CHANGELOG.md et CHANGELOG.en.md : « ## [{new}] - {date} »", cargo_path.display());
        return Ok(());
    }
    std::fs::write(root.join("VERSION"), format!("{new}\n"))?;
    std::fs::write(&cargo_path, cargo)?;
    for (p, t) in journals {
        std::fs::write(p, t)?;
    }
    // Cargo.lock porte la version des crates du projet : on la met à jour tout de suite (sans réseau).
    let cargo_bin = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let ok = std::process::Command::new(&cargo_bin)
        .args(["update", "--workspace", "--offline", "--quiet"])
        .current_dir(&root)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        eprintln!("Cargo.lock non mis à jour (il le sera à la prochaine compilation)");
    }
    println!("{old} → {new} : compléter « Mise en service » dans CHANGELOG.md et « Rollout » dans CHANGELOG.en.md");
    Ok(())
}

/// Membres de l'espace de travail (liste `members = [...]` du Cargo.toml racine).
fn members(root: &Path) -> Vec<String> {
    let Ok(t) = std::fs::read_to_string(root.join("Cargo.toml")) else { return Vec::new() };
    let Some(start) = t.find("members") else { return Vec::new() };
    let Some(open) = t[start..].find('[') else { return Vec::new() };
    let Some(close) = t[start + open..].find(']') else { return Vec::new() };
    t[start + open + 1..start + open + close].split(',').map(|s| s.trim().trim_matches('"').to_string()).filter(|s| !s.is_empty()).collect()
}

/// Écrit la page des versions (HTML) tirée du CHANGELOG.
fn changelog(out: Option<PathBuf>) -> Res {
    let root = workspace_root();
    let out = match out {
        Some(o) => o,
        None => dist_dir()?.join("versions.html"),
    };
    let md = std::fs::read_to_string(root.join("CHANGELOG.md"))?;
    std::fs::write(&out, changelog_page::render(&md))?;
    println!("{}", out.display());
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// macOS

/// Info.plist d'AlpIRC.app : identifiant, versions, schémas `irc://` et `ircs://`.
fn info_plist(v: &str, build: u32) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>AlpIRC</string>
    <key>CFBundleDisplayName</key><string>AlpIRC</string>
    <key>CFBundleIdentifier</key><string>fr.guiwald.alpirc</string>
    <key>CFBundleVersion</key><string>{build}</string>
    <key>CFBundleShortVersionString</key><string>{v}</string>
    <key>CFBundleExecutable</key><string>alpirc</string>
    <key>CFBundleIconFile</key><string>AlpIRC</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
    <key>CFBundleDevelopmentRegion</key><string>en</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>LSApplicationCategoryType</key><string>public.app-category.social-networking</string>
    <key>NSHumanReadableCopyright</key><string>© Guiwald — licence MIT — github.com/Guiwald/alpirc</string>
    <key>CFBundleURLTypes</key>
    <array>
        <dict>
            <key>CFBundleURLName</key><string>fr.guiwald.alpirc.irc</string>
            <key>CFBundleTypeRole</key><string>Viewer</string>
            <key>CFBundleURLSchemes</key>
            <array><string>irc</string><string>ircs</string></array>
        </dict>
    </array>
</dict>
</plist>
"#
    )
}

/// Architectures d'un binaire Mach-O : simple (64 bits) ou universel (« fat », fait par `lipo`) ; `None` si ce n'est
/// pas un Mach-O (un binaire Linux, par exemple).
fn macho_archs(bin: &[u8]) -> Option<Vec<String>> {
    let be = |o: usize| bin.get(o..o + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    let le = |o: usize| bin.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let name = |cpu: u32| match cpu {
        0x0100_000c => "arm64".to_string(),
        0x0100_0007 => "x86_64".to_string(),
        other => format!("cpu {other:#x}"),
    };
    match be(0)? {
        // FAT_MAGIC (entrées de 20 octets) et FAT_MAGIC_64 (32 octets), en gros-boutiste.
        m @ (0xcafe_babe | 0xcafe_babf) => {
            let n = be(4)? as usize;
            let size = if m == 0xcafe_babe { 20 } else { 32 };
            // 0xcafebabe est aussi la signature des classes Java : un nombre d'architectures absurde trahit ces fichiers.
            if n == 0 || n > 16 {
                return None;
            }
            (0..n).map(|i| be(8 + i * size).map(name)).collect()
        }
        // MH_MAGIC_64 lu en gros-boutiste (le fichier est en petit-boutiste).
        0xcffa_edfe => Some(vec![name(le(4)?)]),
        _ => None,
    }
}

/// Construit `AlpIRC.app` (et, sur macOS, `AlpIRC-X.Y.Z.dmg`) à partir d'un binaire (universel ou non).
fn bundle_mac(binary: Option<PathBuf>, out_dir: Option<PathBuf>) -> Res {
    let v = version()?;
    let binary = binary.ok_or("binaire manquant : cargo xtask bundle-mac <binaire> [dossier]")?;
    if !binary.is_file() {
        return Err(format!("{} introuvable : cargo build --release -p alpirc d'abord", binary.display()).into());
    }
    let out_dir = match out_dir {
        Some(d) => {
            std::fs::create_dir_all(&d)?;
            d
        }
        None => dist_dir()?,
    };
    let dmg = out_dir.join(format!("AlpIRC-{v}.dmg"));
    if cfg!(target_os = "macos") {
        refuse_overwrite(&dmg)?;
    }
    // Le .app est un intermédiaire : refait à chaque fois.
    let app = out_dir.join("AlpIRC.app");
    let _ = std::fs::remove_dir_all(&app);
    let contents = app.join("Contents");
    std::fs::create_dir_all(contents.join("MacOS"))?;
    std::fs::create_dir_all(contents.join("Resources"))?;
    let exe = contents.join("MacOS/alpirc");
    std::fs::copy(&binary, &exe)?;
    // Pour le journal de la CI : un paquet universel doit montrer arm64 et x86_64.
    match macho_archs(&std::fs::read(&binary)?) {
        Some(a) if a.len() > 1 => println!("binaire universel : {}", a.join(" + ")),
        Some(a) => println!("binaire : {} seulement", a.join("")),
        None => eprintln!("attention : {} n'est pas un binaire macOS (Mach-O)", binary.display()),
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755))?;
    }
    let src = icons::Sources::load(icons::filter_from_env())?;
    std::fs::write(contents.join("Resources/AlpIRC.icns"), icons::icns(&src)?)?;
    std::fs::write(contents.join("Info.plist"), info_plist(&v, version_code(&v)))?;
    std::fs::write(contents.join("PkgInfo"), "APPL????")?;
    println!("{}", app.display());
    // Le .dmg ne se fabrique que sur macOS (hdiutil) ; ailleurs, seul le .app est produit.
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        // Signature ad hoc (indispensable sur les Mac à puce Apple ; pas de certificat payant, ADR 0002).
        run(Command::new("codesign").args(["--force", "--deep", "--sign", "-"]).arg(&app))?;
        let stage = out_dir.join("dmg-stage");
        let _ = std::fs::remove_dir_all(&stage);
        std::fs::create_dir_all(&stage)?;
        run(Command::new("cp").arg("-R").arg(&app).arg(&stage))?;
        std::os::unix::fs::symlink("/Applications", stage.join("Applications"))?;
        run(Command::new("hdiutil").args(["create", "-volname", "AlpIRC", "-fs", "HFS+", "-format", "UDZO", "-srcfolder"]).arg(&stage).arg(&dmg))?;
        let _ = std::fs::remove_dir_all(&stage);
        println!("{}", dmg.display());
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = dmg;
        eprintln!("(hors macOS : pas de .dmg, seulement AlpIRC.app)");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_suivantes() {
        assert_eq!(next_version("0.1.0", "patch").unwrap(), "0.1.1");
        assert_eq!(next_version("0.1.9", "minor").unwrap(), "0.2.0");
        assert_eq!(next_version("0.9.3", "major").unwrap(), "1.0.0");
        assert!(next_version("0.1.0", "huge").is_err());
        assert!(next_version("0.1", "patch").is_err());
    }

    #[test]
    fn version_code_derivee() {
        assert_eq!(version_code("0.1.0"), 100);
        assert_eq!(version_code("1.2.3"), 10_203);
        assert_eq!(version_code("12.34.56"), 123_456);
    }

    #[test]
    fn version_de_l_espace_de_travail() {
        let t = "[workspace]\nmembers = [\"a\"]\n\n[workspace.package]\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace.dependencies]\nfoo = { version = \"1\" }\n";
        let n = set_workspace_version(t, "0.2.0").unwrap();
        assert!(n.contains("[workspace.package]\nversion = \"0.2.0\"\n"));
        assert!(n.contains("foo = { version = \"1\" }"), "les dépendances ne bougent pas");
        assert_eq!(n.matches("0.2.0").count(), 1);
        assert!(n.ends_with('\n'));
        assert!(set_workspace_version("[package]\nversion = \"1.0.0\"\n", "2.0.0").is_err());
        // `versionx = …` n'est pas la clé `version`.
        assert!(set_workspace_version("[workspace.package]\nversionx = \"1\"\n", "2.0.0").is_err());
    }

    #[test]
    fn journal_date() {
        let t = "# Changelog\n\n## [Non publié]\n\n### Ajouté\n- Socle : espace de travail.\n";
        let n = date_changelog(t, "## [Non publié]", "0.1.0", "2026-10-17").unwrap();
        assert_eq!(n, "# Changelog\n\n## [Non publié]\n\n## [0.1.0] - 2026-10-17\n\n### Ajouté\n- Socle : espace de travail.\n");
        assert!(date_changelog(&n, "## [Non publié]", "0.1.0", "2026-10-18").is_err(), "pas deux fois la même version");
        assert!(date_changelog("# rien\n", "## [Unreleased]", "0.1.0", "2026-10-17").is_err());
        let en = "## [Unreleased]\n### Added\n- x\n";
        assert_eq!(date_changelog(en, "## [Unreleased]", "1.0.0", "2027-01-02").unwrap(), "## [Unreleased]\n\n## [1.0.0] - 2027-01-02\n### Added\n- x\n");
    }

    #[test]
    fn dates() {
        assert_eq!(date_from_days(0), "1970-01-01");
        assert_eq!(date_from_days(20_743), "2026-10-17");
        assert_eq!(date_from_days(11_016), "2000-02-29");
    }

    #[test]
    fn info_plist_complet() {
        let p = info_plist("1.2.3", 10_203);
        for k in [
            "<key>CFBundleIdentifier</key><string>fr.guiwald.alpirc</string>",
            "<key>CFBundleExecutable</key><string>alpirc</string>",
            "<key>CFBundleShortVersionString</key><string>1.2.3</string>",
            "<key>CFBundleVersion</key><string>10203</string>",
            "<key>LSMinimumSystemVersion</key><string>11.0</string>",
            "<key>NSHighResolutionCapable</key><true/>",
            "public.app-category.social-networking",
            "<string>irc</string><string>ircs</string>",
            "<key>CFBundleIconFile</key><string>AlpIRC</string>",
        ] {
            assert!(p.contains(k), "{k}");
        }
    }

    #[test]
    fn architectures_mach_o() {
        let mut thin = vec![0xcf, 0xfa, 0xed, 0xfe, 0x0c, 0x00, 0x00, 0x01];
        thin.resize(64, 0);
        assert_eq!(macho_archs(&thin), Some(vec!["arm64".to_string()]));
        // Universel (lipo) : en-tête gros-boutiste, une entrée de 20 octets par architecture.
        let mut fat = vec![0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 2];
        for cpu in [0x0100_0007u32, 0x0100_000c] {
            fat.extend_from_slice(&cpu.to_be_bytes());
            fat.extend_from_slice(&[0; 16]);
        }
        assert_eq!(macho_archs(&fat), Some(vec!["x86_64".to_string(), "arm64".to_string()]));
        assert_eq!(macho_archs(b"\x7fELF\x02\x01\x01\x00"), None);
        assert_eq!(macho_archs(&[0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 52]), None, "classe Java");
        assert_eq!(macho_archs(&fat[..20]), None, "tronqué");
    }

    #[test]
    fn membres() {
        let m = members(&workspace_root());
        assert!(m.iter().any(|x| x == "xtask"));
        assert!(m.iter().any(|x| x == "app/desktop"));
    }
}
