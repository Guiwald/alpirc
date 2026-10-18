//! macOS : notifications natives (UNUserNotificationCenter), au nom et avec l'icône d'AlpIRC.
//!
//! Seulement depuis AlpIRC.app : hors paquet, le centre de notifications lève une exception. La permission est
//! demandée une fois par lancement, à la première notification (macOS ne repose la question que si l'utilisateur ne
//! l'a jamais tranchée) ; les notifications arrivées entre-temps attendent la réponse. Hors paquet, sans permission
//! ou si l'envoi échoue : repli sur `osascript` (la notification vient alors de « Éditeur de script »).
//! Un délégué fait afficher les notifications même quand AlpIRC est au premier plan (le shell ne notifie alors que
//! pour une autre fenêtre que l'active) et ramène la fenêtre quand on clique dessus (macOS active l'appli de
//! lui-même).

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{Bool, NSObject, ProtocolObject};
use objc2::{AllocAnyThread, DefinedClass, define_class, msg_send};
use objc2_foundation::{NSBundle, NSError, NSObjectProtocol, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotification, UNNotificationPresentationOptions,
    UNNotificationRequest, UNNotificationResponse, UNUserNotificationCenter, UNUserNotificationCenterDelegate,
};
use winit::event_loop::EventLoopProxy;

use crate::UserEvent;

/// Notifications en attente de la permission, au plus.
const WAITING_MAX: usize = 20;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Auth {
    /// Pas encore demandée.
    Unknown,
    /// Demandée, réponse attendue.
    Pending,
    Granted,
    Denied,
}

struct State {
    auth: Auth,
    waiting: Vec<(String, String)>,
}

/// Partagé avec les rappels du système (qui arrivent sur un autre fil).
static STATE: Mutex<State> = Mutex::new(State { auth: Auth::Unknown, waiting: Vec::new() });
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

struct Ivars {
    proxy: EventLoopProxy<UserEvent>,
}

define_class!(
    // SAFETY : NSObject n'impose rien aux sous-classes ; `NotifyDelegate` n'implémente pas Drop. Le délégué peut
    // être appelé sur n'importe quel fil : il ne touche qu'au proxy de la boucle (Send + Sync).
    #[unsafe(super(NSObject))]
    #[name = "AlpIRCNotifyDelegate"]
    #[ivars = Ivars]
    struct NotifyDelegate;

    unsafe impl NSObjectProtocol for NotifyDelegate {}

    unsafe impl UNUserNotificationCenterDelegate for NotifyDelegate {
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            handler: &block2::DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            handler.call((UNNotificationPresentationOptions::Banner | UNNotificationPresentationOptions::List,));
        }

        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive(
            &self,
            _center: &UNUserNotificationCenter,
            _response: &UNNotificationResponse,
            handler: &block2::DynBlock<dyn Fn()>,
        ) {
            let _ = self.ivars().proxy.send_event(UserEvent::Raise);
            handler.call(());
        }
    }
);

/// Les notifications de l'application.
pub struct Notifier {
    proxy: EventLoopProxy<UserEvent>,
    /// Lancé depuis AlpIRC.app (sinon : toujours `osascript`).
    bundled: bool,
    center: Option<Retained<UNUserNotificationCenter>>,
    _delegate: Option<Retained<NotifyDelegate>>,
}

impl Notifier {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Notifier {
        let bundle = NSBundle::mainBundle();
        let bundled = bundle.bundleIdentifier().is_some() && bundle.bundlePath().to_string().ends_with(".app");
        Notifier { proxy, bundled, center: None, _delegate: None }
    }

    pub fn notify(&mut self, title: &str, body: &str) {
        if !self.bundled {
            osascript(title, body);
            return;
        }
        let center = self.center();
        let mut st = state();
        match st.auth {
            Auth::Granted => {
                drop(st);
                post(&center, title, body);
            }
            Auth::Denied => {
                drop(st);
                osascript(title, body);
            }
            Auth::Pending => {
                if st.waiting.len() < WAITING_MAX {
                    st.waiting.push((title.to_string(), body.to_string()));
                }
            }
            Auth::Unknown => {
                st.auth = Auth::Pending;
                st.waiting.push((title.to_string(), body.to_string()));
                drop(st);
                request_authorization(&center);
            }
        }
    }

    /// Le centre de notifications, avec notre délégué (créés au premier besoin).
    fn center(&mut self) -> Retained<UNUserNotificationCenter> {
        if let Some(c) = &self.center {
            return c.clone();
        }
        let center = UNUserNotificationCenter::currentNotificationCenter();
        let this = NotifyDelegate::alloc().set_ivars(Ivars { proxy: self.proxy.clone() });
        // SAFETY : `init` de NSObject.
        let delegate: Retained<NotifyDelegate> = unsafe { msg_send![super(this), init] };
        // Le centre ne retient pas son délégué : on le garde ici.
        center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        self._delegate = Some(delegate);
        self.center = Some(center.clone());
        center
    }
}

fn request_authorization(center: &UNUserNotificationCenter) {
    let done = RcBlock::new(|granted: Bool, error: *mut NSError| {
        let granted = granted.as_bool();
        if !error.is_null() {
            // SAFETY : un NSError valide quand il n'est pas nul.
            let e = unsafe { &*error };
            log::warn!("notifications : permission non accordée ({})", e.localizedDescription());
        }
        let waiting = {
            let mut st = state();
            st.auth = if granted { Auth::Granted } else { Auth::Denied };
            std::mem::take(&mut st.waiting)
        };
        let center = UNUserNotificationCenter::currentNotificationCenter();
        for (title, body) in waiting {
            if granted {
                post(&center, &title, &body);
            } else {
                osascript(&title, &body);
            }
        }
    });
    center.requestAuthorizationWithOptions_completionHandler(UNAuthorizationOptions::Alert, &done);
}

/// Envoie une notification tout de suite (sans déclencheur) ; en cas d'échec, `osascript`.
fn post(center: &UNUserNotificationCenter, title: &str, body: &str) {
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    content.setBody(&NSString::from_str(body));
    let id = NSString::from_str(&format!("alpirc-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed)));
    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(&id, &content, None);
    let (title, body) = (title.to_string(), body.to_string());
    let done = RcBlock::new(move |error: *mut NSError| {
        if !error.is_null() {
            // SAFETY : un NSError valide quand il n'est pas nul.
            let e = unsafe { &*error };
            log::warn!("notification non envoyée ({}) : repli sur osascript", e.localizedDescription());
            osascript(&title, &body);
        }
    });
    center.addNotificationRequest_withCompletionHandler(&request, Some(&done));
}

/// Échappe une chaîne pour AppleScript.
fn applescript_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Repli : `display notification` d'AppleScript.
pub fn osascript(title: &str, body: &str) {
    let script = format!("display notification {} with title {}", applescript_string(body), applescript_string(title));
    let _ = std::process::Command::new("osascript").args(["-e", &script]).spawn();
}
