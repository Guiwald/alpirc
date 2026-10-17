//! `alpirc-relay` : le relais (bouncer) IRC d'AlpIRC (ADR 0009, réalisation : ADR 0014).
//!
//! Il reste connecté aux réseaux de chaque utilisateur sous un seul pseudo, garde l'historique, et laisse
//! plusieurs appareils (AlpIRC sur l'ordinateur et sur le téléphone, ou d'autres clients) partager la même
//! session : ce qui est écrit sur l'un apparaît sur l'autre. Il parle comme le bouncer soju
//! (`soju.im/bouncer-networks`, `draft/chathistory`, `draft/read-marker`).
//!
//! Organisation :
//! - [`relay`] : le cœur (un fil, les événements d'`alpirc-net`), [`relay::start`] ;
//! - [`upstream`] : la connexion à un réseau, au-dessus de la session d'`alpirc-core` ;
//! - [`downstream`] : la connexion d'un appareil et la mise en forme selon ses capacités ;
//! - [`routing`] : quel appareil reçoit la réponse à sa question ;
//! - [`history`], [`markers`], [`networks`] : ce qui est gardé sur le disque ;
//! - [`config`], [`auth`], [`ids`] : réglages, mots de passe, identifiants et heures.

pub mod auth;
pub mod config;
pub mod downstream;
pub mod history;
pub mod ids;
pub mod markers;
pub mod networks;
pub mod relay;
pub mod routing;
pub mod upstream;

pub use config::Config;
pub use relay::{
    NetKey, RelayHandle, Stopper, VERSION, ensure_certificate, fingerprint_colons, fingerprint_of_pem, start,
};
