//! Le pont avec le code Java d'AlpIRC (`app/android/java/fr/guiwald/alpirc`), par JNI.
//!
//! Les classes de l'appli ne se trouvent pas par `FindClass` depuis un fil natif (chargeur du système) : on les
//! charge une fois par le chargeur de l'activité, au démarrage, et on garde des références globales. Les appels
//! suivants passent par les méthodes statiques de `Bridge`, qui marchent même quand l'activité a disparu.
//!
//! Le code Java pose des événements texte dans une file ; un fil de Rust les attend (`Bridge.take`) et les
//! envoie à la boucle d'événements, qui se réveille aussitôt.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use jni::JavaVM;
use jni::objects::{GlobalRef, JClass, JIntArray, JObject, JString, JValue};
use winit::event_loop::EventLoopProxy;
use winit::platform::android::activity::AndroidApp;

use crate::app::UserEvent;
use crate::host::{Host, ServiceState};

static VM: OnceLock<JavaVM> = OnceLock::new();
static BRIDGE: OnceLock<GlobalRef> = OnceLock::new();
static STOP: AtomicBool = AtomicBool::new(false);

/// À appeler au démarrage, tant que l'activité existe : garde la machine Java et la classe `Bridge`.
pub fn init_bridge(app: &AndroidApp) {
    if BRIDGE.get().is_some() {
        return;
    }
    let Ok(vm) = (unsafe { JavaVM::from_raw(app.vm_as_ptr() as *mut jni::sys::JavaVM) }) else {
        log::error!("machine Java introuvable");
        return;
    };
    let class = (|| -> jni::errors::Result<GlobalRef> {
        let mut env = vm.attach_current_thread()?;
        let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
        let loader = env.call_method(&activity, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?.l()?;
        let name = env.new_string("fr.guiwald.alpirc.Bridge")?;
        let c = env.call_method(&loader, "loadClass", "(Ljava/lang/String;)Ljava/lang/Class;", &[JValue::Object(&name)])?.l()?;
        let r = env.new_global_ref(c);
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_describe();
            let _ = env.exception_clear();
        }
        r
    })();
    match class {
        Ok(c) => {
            let _ = BRIDGE.set(c);
        }
        Err(e) => log::error!("classe Java fr.guiwald.alpirc.Bridge introuvable ({e}) : champ de saisie, service et notifications absents"),
    }
    let _ = VM.set(vm);
}

/// Le pont Java est-il là ?
pub fn bridge_ok() -> bool {
    BRIDGE.get().is_some()
}

/// Appelle `f` avec la classe `Bridge` ; les exceptions Java sont journalisées et effacées. Chaque appel a son
/// propre cadre de références locales : les fils de Rust ne rendent jamais la main à Java, qui ne les libérerait
/// pas.
fn with_bridge<T>(f: impl FnOnce(&mut jni::JNIEnv, &JClass) -> jni::errors::Result<T>) -> Option<T> {
    let vm = VM.get()?;
    let cls = BRIDGE.get()?;
    let mut env = vm.attach_current_thread_permanently().ok()?;
    let jc: &JClass = cls.as_obj().into();
    let r = env.with_local_frame(16, |env| f(env, jc));
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }
    match r {
        Ok(v) => Some(v),
        Err(e) => {
            log::warn!("JNI : {e}");
            None
        }
    }
}

fn call_void(name: &str, sig: &str, args: &[JValue]) {
    with_bridge(|env, c| {
        env.call_static_method(c, name, sig, args)?;
        Ok(())
    });
}

fn call_str(name: &str, args_sig: &str) -> Option<String> {
    with_bridge(|env, c| {
        let o = env.call_static_method(c, name, args_sig, &[])?.l()?;
        if o.is_null() {
            return Ok(None);
        }
        let s: String = env.get_string(&JString::from(o))?.into();
        Ok(Some(s))
    })
    .flatten()
}

/// Appel avec des chaînes en paramètres (converties dans le même cadre JNI).
fn call_with_strings(name: &str, sig: &str, ints: &[i32], bools: &[bool], strings: &[&str]) {
    with_bridge(|env, c| {
        let js: Vec<JObject> = strings.iter().map(|s| env.new_string(s).map(JObject::from)).collect::<Result<_, _>>()?;
        let mut args: Vec<JValue> = Vec::new();
        args.extend(ints.iter().map(|i| JValue::Int(*i)));
        args.extend(bools.iter().map(|b| JValue::Bool(u8::from(*b))));
        args.extend(js.iter().map(JValue::Object));
        env.call_static_method(c, name, sig, &args)?;
        Ok(())
    });
}

/// Démarre le fil qui attend les événements du code Java et les passe à la boucle.
pub fn start_event_thread(proxy: EventLoopProxy<UserEvent>) {
    if !bridge_ok() {
        return;
    }
    let spawned = std::thread::Builder::new().name("alpirc-java".into()).spawn(move || {
        while !STOP.load(Ordering::Acquire) {
            let ev = with_bridge(|env, c| {
                let o = env.call_static_method(c, "take", "(J)Ljava/lang/String;", &[JValue::Long(500)])?.l()?;
                if o.is_null() {
                    return Ok(None);
                }
                let s: String = env.get_string(&JString::from(o))?.into();
                Ok(Some(s))
            });
            match ev {
                Some(Some(s)) => {
                    if proxy.send_event(UserEvent::Sys(s)).is_err() {
                        break;
                    }
                }
                Some(None) => {}
                None => std::thread::sleep(std::time::Duration::from_millis(200)),
            }
        }
        if let Some(vm) = VM.get() {
            // SAFETY: fil attaché par ce fil-ci (`attach_current_thread_permanently`), plus aucun appel JNI ensuite.
            unsafe { vm.detach_current_thread() };
        }
    });
    if let Err(e) = spawned {
        log::error!("fil des événements Java : {e}");
    }
}

pub fn stop_event_thread() {
    STOP.store(true, Ordering::Release);
}

/// Relance l'appli dans un processus neuf (Android a recréé l'activité dans un processus qui avait déjà servi :
/// la boucle de winit ne se recrée pas).
pub fn restart() -> bool {
    with_bridge(|env, c| env.call_static_method(c, "restart", "()Z", &[])?.z()).unwrap_or(false)
}

pub fn exit_app() {
    call_void("exitApp", "()V", &[]);
}

/// Les services d'Android pour l'appli.
pub struct AndroidHost {
    app: AndroidApp,
}

impl AndroidHost {
    pub fn new(app: AndroidApp) -> AndroidHost {
        AndroidHost { app }
    }
}

/// Polices de repli d'Android utiles à une discussion : emoji, symboles, CJK, écritures courantes.
fn system_fonts(dir: &Path) -> Vec<PathBuf> {
    const PREFIXES: &[&str] = &[
        "NotoColorEmoji",
        "NotoSansSymbols",
        "NotoSansCJK",
        "DroidSansFallback",
        "Roboto-Regular",
        "NotoSansArabic-Regular",
        "NotoSansHebrew-Regular",
        "NotoSansThai-Regular",
        "NotoSansDevanagari-Regular",
        "NotoSansArmenian-Regular",
        "NotoSansGeorgian-Regular",
    ];
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            PREFIXES.iter().any(|pre| n.starts_with(pre)) && (n.ends_with(".ttf") || n.ends_with(".ttc") || n.ends_with(".otf"))
        })
        .collect();
    out.sort();
    out
}

impl Host for AndroidHost {
    fn clipboard_get(&mut self) -> Option<String> {
        call_str("getClipboard", "()Ljava/lang/String;")
    }

    fn clipboard_set(&mut self, text: &str) {
        call_with_strings("setClipboard", "(Ljava/lang/String;)V", &[], &[], &[text]);
    }

    fn notify(&mut self, title: &str, body: &str) {
        call_with_strings("notify", "(Ljava/lang/String;Ljava/lang/String;)V", &[], &[], &[title, body]);
    }

    fn vibrate(&mut self, ms: i32) {
        call_void("vibrate", "(I)V", &[JValue::Int(ms)]);
    }

    fn open_url(&mut self, url: &str) {
        call_with_strings("openUrl", "(Ljava/lang/String;)V", &[], &[], &[url]);
    }

    fn data_dir(&self) -> PathBuf {
        self.app.internal_data_path().unwrap_or_else(|| PathBuf::from("/data/data/fr.guiwald.alpirc/files"))
    }

    fn utc_offset(&self, now_ms: i64) -> i32 {
        // Le crate `time` refuse de lire l'heure locale dans un processus à plusieurs fils (la machine Java en a
        // toujours) : on la demande à Java.
        with_bridge(|env, c| env.call_static_method(c, "utcOffset", "(J)I", &[JValue::Long(now_ms)])?.i()).unwrap_or(0)
    }

    fn locale(&self) -> Option<String> {
        call_str("locale", "()Ljava/lang/String;")
    }

    fn density(&self) -> Option<f32> {
        with_bridge(|env, c| env.call_static_method(c, "density", "()F", &[])?.f())
            .filter(|d| *d > 0.1)
            .or_else(|| self.app.config().density().map(|dpi| dpi as f32 / 160.0))
    }

    fn font_fallbacks(&self) -> Vec<PathBuf> {
        system_fonts(Path::new("/system/fonts"))
    }

    fn insets(&self) -> [i32; 4] {
        with_bridge(|env, c| {
            let o = env.call_static_method(c, "insets", "()[I", &[])?.l()?;
            let arr = JIntArray::from(o);
            let mut v = [0i32; 4];
            env.get_int_array_region(&arr, 0, &mut v)?;
            Ok(v)
        })
        .unwrap_or([0; 4])
    }

    fn native_input(&self) -> bool {
        bridge_ok()
    }

    fn set_input(&mut self, visible: bool, hint: &str) {
        call_with_strings("setInput", "(ZLjava/lang/String;)V", &[], &[visible], &[hint]);
    }

    fn set_input_text(&mut self, text: &str) {
        call_with_strings("setInputText", "(Ljava/lang/String;)V", &[], &[], &[text]);
    }

    fn set_input_labels(&mut self, send: &str, tab: &str) {
        call_with_strings("setInputLabels", "(Ljava/lang/String;Ljava/lang/String;)V", &[], &[], &[send, tab]);
    }

    fn show_keyboard(&mut self, show: bool) {
        call_void("showKeyboard", "(Z)V", &[JValue::Bool(u8::from(show))]);
    }

    fn service(&mut self, s: &ServiceState) {
        call_with_strings(
            "service",
            "(ILjava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
            &[s.active as i32],
            &[],
            &[&s.title, &s.text, &s.quit, &s.channel, &s.notify_channel],
        );
    }

    fn to_back(&mut self) -> bool {
        with_bridge(|env, c| env.call_static_method(c, "toBack", "()Z", &[])?.z()).unwrap_or(false)
    }

    fn exit_app(&mut self) {
        stop_event_thread();
        exit_app();
    }

    /// `Android/data/fr.guiwald.alpirc/files/Download/AlpIRC` (stockage de l'appli, sans permission), sinon le
    /// dossier des données.
    fn downloads_dir(&self) -> Option<PathBuf> {
        let d = call_str("downloadsDir", "()Ljava/lang/String;").map(PathBuf::from).unwrap_or_else(|| self.data_dir().join("downloads"));
        let _ = std::fs::create_dir_all(&d);
        Some(d)
    }

    fn pick_file(&mut self, token: u32) -> bool {
        with_bridge(|env, c| env.call_static_method(c, "pickFile", "(I)Z", &[JValue::Int(token as i32)])?.z()).unwrap_or(false)
    }

    fn open_path(&mut self, path: &Path) {
        call_with_strings("openPath", "(Ljava/lang/String;)V", &[], &[], &[&path.to_string_lossy()]);
    }
}
