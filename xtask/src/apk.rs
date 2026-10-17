//! Construction de l'APK Android sans Gradle (ADR 0006) : bibliothèque Rust `libalpirc.so` (NDK), classes Java de
//! `app/android/java` s'il y en a (`javac --release 11` + `d8`), `aapt2` pour le manifeste et les ressources, ajout
//! des bibliothèques non compressées, `zipalign -P 16` (pages de 16 Ko, exigées depuis Android 15), signature
//! `apksigner`.
//!
//! Le manifeste suit les classes Java présentes : `AlpircActivity.java` → activité `fr.guiwald.alpirc.AlpircActivity`
//! (sinon la `android.app.NativeActivity` d'origine) ; `ConnectionService.java` → service au premier plan
//! `specialUse` (connexions gardées en arrière-plan) ; `RestartActivity.java` → activité de relance dans un processus
//! à part (`:relance`, quand Android recrée l'activité dans un processus qui a déjà servi) ; `FilesProvider.java` →
//! fournisseur de fichiers en lecture seule (ouvrir un fichier reçu dans une autre appli) ; sans aucune classe,
//! `android:hasCode="false"`. Une nouvelle classe qui doit figurer dans le manifeste (récepteur, autre activité)
//! s'ajoute dans [`manifest`].
//!
//! Emplacements lus dans l'environnement :
//! - SDK : `ANDROID_HOME`, sinon `ANDROID_SDK_ROOT`, sinon les emplacements d'Android Studio
//!   (`~/Library/Android/sdk`, `~/Android/Sdk`) ; il faut `build-tools/*` et `platforms/android-34` ou plus ;
//! - NDK : `ANDROID_NDK_HOME`, `ANDROID_NDK_LATEST_HOME` (machines de GitHub), `ANDROID_NDK_ROOT`, sinon
//!   `<SDK>/ndk/*` (le plus récent) ;
//! - JDK : `JAVA_HOME`, sinon `javac` et `keytool` du `PATH` ;
//! - signature : `ALPIRC_KEYSTORE` (fichier .p12) et `ALPIRC_KEYSTORE_PASS` (et `ALPIRC_KEY_ALIAS` si le fichier
//!   contient plusieurs clés) ; sans eux, une clé de débogage gardée dans `target/alpirc-debug.p12` (la même d'une
//!   construction à l'autre sur une machine : l'APK de débogage s'installe par-dessus le précédent).
//!
//! `--all-abis` ajoute `x86_64` (émulateur sur un PC ou un Mac Intel ; l'émulateur d'un Mac à puce Apple est arm64).
//! `ALPIRC_APK_LIB=<libalpirc.so>` met dans l'APK une bibliothèque arm64 déjà compilée (sans NDK ni compilation).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{Res, Result, dist_dir, refuse_overwrite, run, target_dir, version, version_code, workspace_root};

pub const PACKAGE: &str = "fr.guiwald.alpirc";
pub const MIN_SDK: u32 = 26;
/// Android 15 : affichage bord à bord imposé, marges gérées par l'appli.
pub const TARGET_SDK: u32 = 35;
/// Plateforme minimale pour lier le manifeste (`foregroundServiceType="specialUse"`, permission
/// `FOREGROUND_SERVICE_SPECIAL_USE` : API 34).
const MIN_COMPILE_SDK: u32 = 34;
/// Paquet Cargo et nom de la bibliothèque (`[lib] name = "alpirc"`, `crate-type = ["cdylib"]`).
const CARGO_PACKAGE: &str = "alpirc-android";
const LIB: &str = "libalpirc.so";

struct Abi {
    triple: &'static str,
    dir: &'static str,
}

const ARM64: Abi = Abi { triple: "aarch64-linux-android", dir: "arm64-v8a" };
const X86_64: Abi = Abi { triple: "x86_64-linux-android", dir: "x86_64" };

/// Les classes Java présentes dans `app/android/java`, qui décident du contenu du manifeste.
#[derive(Debug, Default)]
pub struct JavaParts {
    pub sources: Vec<PathBuf>,
    /// `fr/guiwald/alpirc/AlpircActivity.java` existe.
    pub activity: bool,
    /// `fr/guiwald/alpirc/ConnectionService.java` existe.
    pub service: bool,
    /// `fr/guiwald/alpirc/RestartActivity.java` existe.
    pub restart: bool,
    /// `fr/guiwald/alpirc/FilesProvider.java` existe.
    pub provider: bool,
}

impl JavaParts {
    pub fn scan(dir: &Path) -> JavaParts {
        let mut sources = Vec::new();
        collect_ext(dir, "java", &mut sources);
        sources.sort();
        let pkg = dir.join("fr/guiwald/alpirc");
        JavaParts {
            activity: pkg.join("AlpircActivity.java").is_file(),
            service: pkg.join("ConnectionService.java").is_file(),
            restart: pkg.join("RestartActivity.java").is_file(),
            provider: pkg.join("FilesProvider.java").is_file(),
            sources,
        }
    }

    pub fn has_code(&self) -> bool {
        !self.sources.is_empty()
    }
}

/// Le manifeste d'AlpIRC (versions, permissions, activité, service), selon les classes Java présentes.
pub fn manifest(v: &str, code: u32, java: &JavaParts) -> String {
    let activity = if java.activity { "fr.guiwald.alpirc.AlpircActivity" } else { "android.app.NativeActivity" };
    let service = if java.service {
        r#"
        <!-- Connexions IRC gardées ouvertes en arrière-plan, avec une notification (ADR 0006). « specialUse » n'est
             pas limité à 6 h par jour, contrairement à « dataSync » depuis Android 15. -->
        <service
            android:name="fr.guiwald.alpirc.ConnectionService"
            android:exported="false"
            android:foregroundServiceType="specialUse">
            <property
                android:name="android.app.PROPERTY_SPECIAL_USE_FGS_SUBTYPE"
                android:value="Keeps IRC connections open while the app is in background" />
        </service>"#
    } else {
        ""
    };
    let restart = if java.restart {
        r#"
        <!-- Relance d'AlpIRC dans un processus neuf (la boucle de Rust ne se recrée pas dans le même processus). -->
        <activity
            android:name="fr.guiwald.alpirc.RestartActivity"
            android:exported="false"
            android:process=":relance"
            android:excludeFromRecents="true"
            android:noHistory="true"
            android:theme="@android:style/Theme.Translucent.NoTitleBar" />"#
    } else {
        ""
    };
    let provider = if java.provider {
        r#"
        <!-- Fichiers reçus (DCC) ouverts dans une autre appli, en lecture seule, le temps de l'ouverture. -->
        <provider
            android:name="fr.guiwald.alpirc.FilesProvider"
            android:authorities="fr.guiwald.alpirc.files"
            android:exported="false"
            android:grantUriPermissions="true" />"#
    } else {
        ""
    };
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<!-- Généré par cargo xtask apk (xtask/src/apk.rs) : ne pas modifier ici. -->
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="{PACKAGE}"
    android:versionCode="{code}"
    android:versionName="{v}">
    <uses-sdk android:minSdkVersion="{MIN_SDK}" android:targetSdkVersion="{TARGET_SDK}" />
    <uses-permission android:name="android.permission.INTERNET" />
    <uses-permission android:name="android.permission.ACCESS_NETWORK_STATE" />
    <uses-permission android:name="android.permission.FOREGROUND_SERVICE" />
    <uses-permission android:name="android.permission.FOREGROUND_SERVICE_SPECIAL_USE" />
    <uses-permission android:name="android.permission.POST_NOTIFICATIONS" />
    <uses-permission android:name="android.permission.WAKE_LOCK" />
    <uses-permission android:name="android.permission.VIBRATE" />
    <!-- Les bibliothèques sont rangées non compressées et alignées sur 16 Ko (zipalign -P 16), comme pour
         Alpagamp : Android les charge directement depuis l'APK ou les extrait, selon la version. -->
    <application
        android:label="@string/app_name"
        android:icon="@mipmap/ic_launcher"
        android:roundIcon="@mipmap/ic_launcher_round"
        android:hasCode="{has_code}"
        android:allowBackup="true"
        android:theme="@android:style/Theme.DeviceDefault.NoActionBar">
        <activity
            android:name="{activity}"
            android:exported="true"
            android:launchMode="singleTask"
            android:windowSoftInputMode="adjustResize"
            android:configChanges="orientation|screenSize|screenLayout|smallestScreenSize|keyboard|keyboardHidden|navigation|uiMode|density|fontScale|locale|layoutDirection|colorMode|touchscreen|mcc|mnc|fontWeightAdjustment">
            <meta-data android:name="android.app.lib_name" android:value="alpirc" />
            <intent-filter>
                <action android:name="android.intent.action.MAIN" />
                <category android:name="android.intent.category.LAUNCHER" />
            </intent-filter>
            <!-- Liens irc:// et ircs:// (une page web, un message) : AlpIRC rejoint le canal. -->
            <intent-filter>
                <action android:name="android.intent.action.VIEW" />
                <category android:name="android.intent.category.DEFAULT" />
                <category android:name="android.intent.category.BROWSABLE" />
                <data android:scheme="irc" />
                <data android:scheme="ircs" />
            </intent-filter>
        </activity>{restart}{service}{provider}
    </application>
</manifest>
"#,
        has_code = java.has_code(),
    )
}

fn strings_xml() -> &'static str {
    "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<resources>\n    <string name=\"app_name\">AlpIRC</string>\n</resources>\n"
}

/// Tri par numéros (« 9.0.0 » < « 35.0.0 » ; « android-36 » > « android-35 »).
fn version_key(name: &str) -> Vec<u64> {
    name.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty()).map(|s| s.parse().unwrap_or(0)).collect()
}

/// Les sous-dossiers d'un dossier qui vérifient `pred`, du plus récent au plus ancien.
fn dirs_newest_first(dir: &Path, pred: impl Fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir() && pred(p)).collect();
    v.sort_by_key(|p| std::cmp::Reverse(version_key(&p.file_name().unwrap_or_default().to_string_lossy())));
    v
}

fn exe(name: &str) -> String {
    if cfg!(windows) { format!("{name}.exe") } else { name.to_string() }
}

fn script(name: &str) -> String {
    if cfg!(windows) { format!("{name}.bat") } else { name.to_string() }
}

struct Sdk {
    build_tools: PathBuf,
    android_jar: PathBuf,
    /// Outils du NDK (compilation de la bibliothèque) ; inutiles avec `ALPIRC_APK_LIB`.
    ndk_bin: Option<PathBuf>,
}

fn env_path(names: &[&str]) -> Option<PathBuf> {
    names.iter().find_map(|n| std::env::var_os(n).filter(|v| !v.is_empty()).map(PathBuf::from))
}

fn find_sdk(need_ndk: bool) -> Result<Sdk> {
    let home_dir = env_path(&["HOME", "USERPROFILE"]).unwrap_or_default();
    let sdk = env_path(&["ANDROID_HOME", "ANDROID_SDK_ROOT"])
        .or_else(|| [home_dir.join("Library/Android/sdk"), home_dir.join("Android/Sdk"), PathBuf::from("/opt/android-sdk")].into_iter().find(|p| p.is_dir()))
        .ok_or("SDK Android introuvable : définir ANDROID_HOME (Android Studio l'installe dans ~/Library/Android/sdk sur Mac, ~/Android/Sdk sur Linux)")?;
    if !sdk.is_dir() {
        return Err(format!("SDK Android : {} n'existe pas (ANDROID_HOME)", sdk.display()).into());
    }
    let build_tools = dirs_newest_first(&sdk.join("build-tools"), |p| ["aapt2", "zipalign"].iter().all(|t| p.join(exe(t)).is_file()) && p.join(script("apksigner")).is_file())
        .into_iter()
        .next()
        .ok_or_else(|| format!("build-tools introuvables dans {} : sdkmanager \"build-tools;35.0.0\"", sdk.join("build-tools").display()))?;
    let platform = dirs_newest_first(&sdk.join("platforms"), |p| p.join("android.jar").is_file())
        .into_iter()
        .next()
        .ok_or_else(|| format!("aucune plateforme Android dans {} : sdkmanager \"platforms;android-{TARGET_SDK}\"", sdk.join("platforms").display()))?;
    let api = version_key(&platform.file_name().unwrap_or_default().to_string_lossy()).first().copied().unwrap_or(0);
    if api < MIN_COMPILE_SDK as u64 {
        return Err(format!("{} est trop ancienne (API {MIN_COMPILE_SDK} au moins) : sdkmanager \"platforms;android-{TARGET_SDK}\"", platform.display()).into());
    }
    let android_jar = platform.join("android.jar");
    if !need_ndk {
        return Ok(Sdk { build_tools, android_jar, ndk_bin: None });
    }
    let ndk = env_path(&["ANDROID_NDK_HOME", "ANDROID_NDK_LATEST_HOME", "ANDROID_NDK_ROOT", "ANDROID_NDK"])
        .or_else(|| dirs_newest_first(&sdk.join("ndk"), |_| true).into_iter().next())
        .or_else(|| Some(sdk.join("ndk-bundle")).filter(|p| p.is_dir()))
        .ok_or("NDK introuvable : définir ANDROID_NDK_HOME (ou sdkmanager \"ndk;27.2.12479018\")")?;
    // Le NDK range ses outils sous « darwin-x86_64 » même sur les Mac à puce Apple (binaires universels).
    let host = if cfg!(target_os = "macos") {
        "darwin-x86_64"
    } else if cfg!(windows) {
        "windows-x86_64"
    } else {
        "linux-x86_64"
    };
    let ndk_bin = ndk.join("toolchains/llvm/prebuilt").join(host).join("bin");
    if !ndk_bin.is_dir() {
        return Err(format!("NDK incomplet : {} n'existe pas (ANDROID_NDK_HOME = {} ?)", ndk_bin.display(), ndk.display()).into());
    }
    Ok(Sdk { build_tools, android_jar, ndk_bin: Some(ndk_bin) })
}

fn jdk_tool(name: &str) -> PathBuf {
    env_path(&["JAVA_HOME"]).map(|h| h.join("bin").join(exe(name))).filter(|p| p.is_file()).unwrap_or_else(|| PathBuf::from(exe(name)))
}

fn check_tool(tool: &Path, arg: &str, hint: &str) -> Res {
    match Command::new(tool).arg(arg).output() {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("{} introuvable ({e}) : {hint}", tool.display()).into()),
    }
}

fn collect_ext(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_ext(&p, ext, out);
        } else if p.extension().is_some_and(|x| x == ext) {
            out.push(p);
        }
    }
}

/// Compile les classes Java en `classes.dex`.
fn build_dex(java: &JavaParts, sdk: &Sdk, work: &Path) -> Result<PathBuf> {
    let javac = jdk_tool("javac");
    check_tool(&javac, "-version", "installer un JDK 17 ou plus récent (ou définir JAVA_HOME)")?;
    let classes = work.join("classes");
    std::fs::create_dir_all(&classes)?;
    run(Command::new(&javac).args(["--release", "11", "-encoding", "UTF-8", "-nowarn", "-cp"]).arg(&sdk.android_jar).arg("-d").arg(&classes).args(&java.sources))?;
    let mut class_files = Vec::new();
    collect_ext(&classes, "class", &mut class_files);
    class_files.sort();
    let dex_dir = work.join("dex");
    std::fs::create_dir_all(&dex_dir)?;
    run(Command::new(sdk.build_tools.join(script("d8"))).args(["--release", "--min-api", &MIN_SDK.to_string(), "--lib"]).arg(&sdk.android_jar).arg("--output").arg(&dex_dir).args(&class_files))?;
    Ok(dex_dir.join("classes.dex"))
}

/// Compile `libalpirc.so` pour une architecture, avec l'éditeur de liens du NDK.
fn build_lib(root: &Path, sdk: &Sdk, abi: &Abi) -> Result<PathBuf> {
    let ndk_bin = sdk.ndk_bin.as_ref().ok_or("NDK introuvable")?;
    let clang = ndk_bin.join(format!("{}{MIN_SDK}-clang{}", abi.triple, if cfg!(windows) { ".cmd" } else { "" }));
    if !clang.is_file() {
        return Err(format!("{} introuvable : NDK trop ancien ou incomplet", clang.display()).into());
    }
    let env_target = abi.triple.replace('-', "_");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let st = Command::new(&cargo)
        .args(["build", "--release", "-p", CARGO_PACKAGE, "--lib", "--target", abi.triple])
        .current_dir(root)
        .env(format!("CARGO_TARGET_{}_LINKER", env_target.to_uppercase()), &clang)
        .env(format!("CC_{env_target}"), &clang)
        .env(format!("AR_{env_target}"), ndk_bin.join(exe("llvm-ar")))
        .status()?;
    if !st.success() {
        return Err(format!("échec de la compilation pour {} (cible installée ? rustup target add {})", abi.triple, abi.triple).into());
    }
    let so = target_dir().join(abi.triple).join("release").join(LIB);
    if !so.is_file() {
        return Err(format!("{} introuvable : app/android/Cargo.toml doit avoir [lib] name = \"alpirc\" et crate-type = [\"cdylib\"]", so.display()).into());
    }
    Ok(so)
}

/// `cargo xtask apk [--all-abis]` : construit `AlpIRC-X.Y.Z.apk` dans le dossier des paquets.
pub fn build_apk(all_abis: bool) -> Res {
    let root = workspace_root();
    let v = version()?;
    let code = version_code(&v);
    let out = dist_dir()?.join(format!("AlpIRC-{v}.apk"));
    refuse_overwrite(&out)?;
    let prebuilt = env_path(&["ALPIRC_APK_LIB"]);
    let sdk = find_sdk(prebuilt.is_none())?;
    let java = JavaParts::scan(&root.join("app/android/java"));
    if !java.has_code() {
        eprintln!("aucune classe Java dans app/android/java : NativeActivity seule, sans service (hasCode=false)");
    }
    // La clé de signature, vérifiée avant les longues compilations.
    let (ks, pass, alias) = match (env_path(&["ALPIRC_KEYSTORE"]), std::env::var("ALPIRC_KEYSTORE_PASS")) {
        (Some(k), Ok(p)) => {
            if !k.is_file() {
                return Err(format!("ALPIRC_KEYSTORE : {} introuvable", k.display()).into());
            }
            (k, p, std::env::var("ALPIRC_KEY_ALIAS").ok())
        }
        (Some(_), Err(_)) => return Err("ALPIRC_KEYSTORE est défini mais pas ALPIRC_KEYSTORE_PASS".into()),
        (None, _) => {
            let ks = target_dir().join("alpirc-debug.p12");
            if !ks.is_file() {
                eprintln!("ALPIRC_KEYSTORE absent : création d'une clé de débogage ({})", ks.display());
                std::fs::create_dir_all(target_dir())?;
                let keytool = jdk_tool("keytool");
                check_tool(&keytool, "-help", "installer un JDK (ou définir JAVA_HOME)")?;
                run(Command::new(keytool)
                    .args(["-genkeypair", "-keystore"])
                    .arg(&ks)
                    .args(["-storetype", "PKCS12", "-storepass", "android", "-keypass", "android", "-alias", "debug", "-keyalg", "RSA", "-keysize", "2048", "-validity", "10000", "-dname", "CN=AlpIRC debug"]))?;
            } else {
                eprintln!("ALPIRC_KEYSTORE absent : signature avec la clé de débogage {}", ks.display());
            }
            (ks, "android".to_string(), None)
        }
    };
    // 1. Bibliothèques Rust.
    let mut libs = Vec::new();
    if let Some(so) = prebuilt {
        if !so.is_file() {
            return Err(format!("ALPIRC_APK_LIB : {} introuvable", so.display()).into());
        }
        eprintln!("bibliothèque déjà compilée : {} (arm64-v8a)", so.display());
        libs.push((ARM64.dir, so));
    } else {
        let abis: Vec<&Abi> = if all_abis { vec![&ARM64, &X86_64] } else { vec![&ARM64] };
        for abi in &abis {
            libs.push((abi.dir, build_lib(&root, &sdk, abi)?));
        }
    }
    // 2. Classes Java, manifeste et ressources.
    let work = target_dir().join("apk");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work)?;
    let dex = if java.has_code() { Some(build_dex(&java, &sdk, &work)?) } else { None };
    let res = work.join("res");
    std::fs::create_dir_all(res.join("values"))?;
    std::fs::write(res.join("values/strings.xml"), strings_xml())?;
    crate::icons::write_android_res(&crate::icons::Sources::load(crate::icons::filter_from_env())?, &res)?;
    let manifest_path = work.join("AndroidManifest.xml");
    std::fs::write(&manifest_path, manifest(&v, code, &java))?;
    let aapt2 = sdk.build_tools.join(exe("aapt2"));
    run(Command::new(&aapt2).arg("compile").arg("--dir").arg(&res).arg("-o").arg(work.join("res.zip")))?;
    let base = work.join("base.apk");
    run(Command::new(&aapt2)
        .arg("link")
        .arg("-o")
        .arg(&base)
        .arg("-I")
        .arg(&sdk.android_jar)
        .arg("--manifest")
        .arg(&manifest_path)
        .args(["--min-sdk-version", &MIN_SDK.to_string(), "--target-sdk-version", &TARGET_SDK.to_string()])
        .args(["--version-code", &code.to_string(), "--version-name", &v])
        .arg(work.join("res.zip")))?;
    // 3. Ajout du code Java et des bibliothèques, non compressées (chargées directement depuis l'APK).
    let unaligned = work.join("unaligned.apk");
    {
        let mut src = zip::ZipArchive::new(std::fs::File::open(&base)?)?;
        let mut dst = zip::ZipWriter::new(std::fs::File::create(&unaligned)?);
        for i in 0..src.len() {
            dst.raw_copy_file(src.by_index(i)?)?;
        }
        if let Some(dex) = &dex {
            let deflated = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            dst.start_file("classes.dex", deflated)?;
            dst.write_all(&std::fs::read(dex)?)?;
        }
        let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (dir, so) in &libs {
            dst.start_file(format!("lib/{dir}/{LIB}"), stored)?;
            let mut bytes = Vec::new();
            std::fs::File::open(so)?.read_to_end(&mut bytes)?;
            dst.write_all(&bytes)?;
        }
        dst.finish()?;
    }
    // 4. Alignement (pages de 16 Ko) et signature.
    let aligned = work.join("aligned.apk");
    run(Command::new(sdk.build_tools.join(exe("zipalign"))).args(["-f", "-P", "16", "4"]).arg(&unaligned).arg(&aligned))?;
    let apksigner = sdk.build_tools.join(script("apksigner"));
    let mut sign = Command::new(&apksigner);
    sign.args(["sign", "--ks"]).arg(&ks).args(["--ks-pass", "env:ALPIRC_APKSIGNER_PASS", "--key-pass", "env:ALPIRC_APKSIGNER_PASS"]);
    if let Some(a) = &alias {
        sign.args(["--ks-key-alias", a]);
    }
    run(sign.arg("--out").arg(&out).arg(&aligned).env("ALPIRC_APKSIGNER_PASS", &pass))?;
    run(Command::new(&apksigner).args(["verify", "--print-certs"]).arg(&out))?;
    let _ = std::fs::remove_file(out.with_extension("apk.idsig"));
    println!("{}", out.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts(activity: bool, service: bool) -> JavaParts {
        let mut sources = Vec::new();
        if activity {
            sources.push(PathBuf::from("AlpircActivity.java"));
        }
        if service {
            sources.push(PathBuf::from("ConnectionService.java"));
        }
        JavaParts { sources, activity, service, restart: false, provider: false }
    }

    #[test]
    fn manifeste_sans_java() {
        let m = manifest("0.1.0", 100, &parts(false, false));
        assert!(m.contains("package=\"fr.guiwald.alpirc\""));
        assert!(m.contains("android:versionCode=\"100\""));
        assert!(m.contains("android:versionName=\"0.1.0\""));
        assert!(m.contains("android:hasCode=\"false\""));
        assert!(m.contains("android:name=\"android.app.NativeActivity\""));
        assert!(!m.contains("<service"));
        assert!(!m.contains("RestartActivity"));
        assert!(m.contains("<meta-data android:name=\"android.app.lib_name\" android:value=\"alpirc\" />"));
        assert!(m.contains("<uses-sdk android:minSdkVersion=\"26\" android:targetSdkVersion=\"35\" />"));
    }

    #[test]
    fn manifeste_avec_java() {
        let m = manifest("1.2.3", 10_203, &parts(true, true));
        assert!(m.contains("android:hasCode=\"true\""));
        assert!(m.contains("android:name=\"fr.guiwald.alpirc.AlpircActivity\""));
        assert!(m.contains("android:name=\"fr.guiwald.alpirc.ConnectionService\""));
        assert!(m.contains("android:foregroundServiceType=\"specialUse\""));
        assert!(m.contains("android:name=\"android.app.PROPERTY_SPECIAL_USE_FGS_SUBTYPE\""));
        assert!(m.contains("android:value=\"Keeps IRC connections open while the app is in background\""));
        // Une classe seule (le service) : du code, mais l'activité d'origine.
        // L'activité de relance, dans son propre processus.
        let mut all = parts(true, true);
        all.restart = true;
        all.provider = true;
        let m2 = manifest("1.2.3", 10_203, &all);
        assert!(m2.contains("android:name=\"fr.guiwald.alpirc.FilesProvider\""));
        assert!(m2.contains("android:grantUriPermissions=\"true\""));
        assert!(!m.contains("<provider"));
        assert!(m2.contains("android:name=\"fr.guiwald.alpirc.RestartActivity\""));
        assert!(m2.contains("android:process=\":relance\""));
        assert_eq!(m2.matches("<activity").count(), 2);
        assert_eq!(m2.matches("android.intent.action.MAIN").count(), 1, "une seule entrée de lanceur");
        let m = manifest("1.2.3", 10_203, &parts(false, true));
        assert!(m.contains("android:hasCode=\"true\""));
        assert!(m.contains("android:name=\"android.app.NativeActivity\""));
        assert!(m.contains("ConnectionService"));
    }

    #[test]
    fn manifeste_permissions_et_liens() {
        let m = manifest("0.1.0", 100, &parts(true, false));
        for p in ["INTERNET", "ACCESS_NETWORK_STATE", "FOREGROUND_SERVICE", "FOREGROUND_SERVICE_SPECIAL_USE", "POST_NOTIFICATIONS", "WAKE_LOCK", "VIBRATE"] {
            assert!(m.contains(&format!("<uses-permission android:name=\"android.permission.{p}\" />")), "{p}");
        }
        assert!(m.contains("<data android:scheme=\"irc\" />") && m.contains("<data android:scheme=\"ircs\" />"));
        assert!(m.contains("android.intent.category.BROWSABLE"));
        assert!(m.contains("android:launchMode=\"singleTask\""));
        assert!(m.contains("android:windowSoftInputMode=\"adjustResize\""));
        assert!(m.contains("android:roundIcon=\"@mipmap/ic_launcher_round\""));
        let cc = m.split("android:configChanges=\"").nth(1).unwrap().split('"').next().unwrap();
        for c in ["orientation", "screenSize", "keyboard", "keyboardHidden", "screenLayout", "uiMode", "density", "smallestScreenSize", "navigation"] {
            assert!(cc.split('|').any(|x| x == c), "{c}");
        }
        assert_eq!(m.matches("<activity").count(), 1);
    }

    #[test]
    fn classes_java_detectees() {
        let dir = std::env::temp_dir().join(format!("alpirc-java-{}", std::process::id()));
        let pkg = dir.join("fr/guiwald/alpirc");
        assert!(!JavaParts::scan(&dir).has_code(), "dossier absent : pas de code");
        std::fs::create_dir_all(&pkg).unwrap();
        std::fs::write(pkg.join("ConnectionService.java"), "class X {}").unwrap();
        std::fs::write(pkg.join("Notes.txt"), "").unwrap();
        let j = JavaParts::scan(&dir);
        assert!(j.has_code() && j.service && !j.activity && !j.restart);
        assert_eq!(j.sources.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tri_des_versions() {
        assert!(version_key("35.0.0") > version_key("9.0.0"));
        assert!(version_key("android-36") > version_key("android-35"));
        assert!(version_key("27.2.12479018") > version_key("26.3.11579264"));
    }
}
