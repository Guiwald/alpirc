//! macOS : les liens `irc://` et `ircs://` ouverts depuis le système (navigateur, Finder, `open irc://…`).
//!
//! macOS ne passe pas l'adresse en argument : il l'envoie par l'Apple Event « GURL » (kInternetEventClass,
//! kAEGetURL), que l'appli soit déjà ouverte ou lancée par le lien. On installe un gestionnaire auprès de
//! NSAppleEventManager, une première fois avant de lancer la boucle, puis de nouveau à
//! `NSApplicationWillFinishLaunchingNotification` : AppKit installe ses propres gestionnaires au lancement, et c'est
//! le moment indiqué par Apple pour les remplacer, avant que l'événement qui a lancé l'appli ne soit distribué.
//! Le délégué de l'application, qui appartient à winit, n'est pas touché. L'adresse part vers la boucle
//! d'événements (`UserEvent::OpenUrl`), puis `Shell::open_irc_url`.

use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_foundation::{NSAppleEventDescriptor, NSAppleEventManager, NSNotification, NSNotificationCenter, NSObjectProtocol};
use winit::event_loop::EventLoopProxy;

use crate::UserEvent;

const fn fourcc(s: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*s)
}

/// `kInternetEventClass` et `kAEGetURL` (tous deux « GURL »), `keyDirectObject` (« ---- »).
const K_INTERNET_EVENT_CLASS: u32 = fourcc(b"GURL");
const K_AE_GET_URL: u32 = fourcc(b"GURL");
const KEY_DIRECT_OBJECT: u32 = fourcc(b"----");

struct Ivars {
    proxy: EventLoopProxy<UserEvent>,
}

define_class!(
    // SAFETY : NSObject n'impose rien aux sous-classes ; `UrlEvents` n'implémente pas Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "AlpIRCUrlEvents"]
    #[ivars = Ivars]
    struct UrlEvents;

    impl UrlEvents {
        #[unsafe(method(handleGetURLEvent:withReplyEvent:))]
        fn handle_get_url(&self, event: &NSAppleEventDescriptor, _reply: &NSAppleEventDescriptor) {
            // SAFETY : `paramDescriptorForKeyword:` prend un AEKeyword (entier de 32 bits).
            let d: Option<Retained<NSAppleEventDescriptor>> =
                unsafe { msg_send![event, paramDescriptorForKeyword: KEY_DIRECT_OBJECT] };
            let Some(url) = d.and_then(|d| d.stringValue()) else { return };
            let url = url.to_string();
            if crate::is_irc_url(&url) {
                let _ = self.ivars().proxy.send_event(UserEvent::OpenUrl(url));
            } else {
                log::info!("lien ignoré : {url}");
            }
        }

        #[unsafe(method(applicationWillFinishLaunching:))]
        fn will_finish_launching(&self, _note: &NSNotification) {
            self.register();
        }
    }

    unsafe impl NSObjectProtocol for UrlEvents {}
);

impl UrlEvents {
    fn register(&self) {
        let manager = NSAppleEventManager::sharedAppleEventManager();
        // SAFETY : le gestionnaire répond au sélecteur donné, avec la signature attendue (événement, réponse) ; les
        // deux codes sont des FourCharCode (entiers de 32 bits).
        unsafe {
            let _: () = msg_send![
                &*manager,
                setEventHandler: self,
                andSelector: sel!(handleGetURLEvent:withReplyEvent:),
                forEventClass: K_INTERNET_EVENT_CLASS,
                andEventID: K_AE_GET_URL
            ];
        }
    }
}

/// Installe le gestionnaire des liens (à appeler sur le fil principal, avant `run_app`).
pub fn install(proxy: EventLoopProxy<UserEvent>) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let this = UrlEvents::alloc(mtm).set_ivars(Ivars { proxy });
    // SAFETY : `init` de NSObject.
    let handler: Retained<UrlEvents> = unsafe { msg_send![super(this), init] };
    handler.register();
    // SAFETY : l'observateur répond au sélecteur (une notification en argument) ; le nom est une constante d'AppKit.
    unsafe {
        NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
            &handler,
            sel!(applicationWillFinishLaunching:),
            Some(objc2_app_kit::NSApplicationWillFinishLaunchingNotification),
            None,
        );
    }
    // Ni NSAppleEventManager ni le centre de notifications ne le retiennent : il vit autant que l'application.
    std::mem::forget(handler);
}
