//! Moteur client IRC d'AlpIRC, sans entrées-sorties et déterministe.
//!
//! Deux couches :
//! - [`session`] : le protocole d'UNE connexion (enregistrement, CAP 302, SASL, ISUPPORT, canaux, IAL, lots,
//!   MONITOR, STS, extensions soju), réutilisable par le relais (`alpirc-relay`) ;
//! - [`client`] : le client à la manière de mIRC (fenêtres, lignes, commandes, carnet d'adresses, journaux,
//!   scripts mSL, bouncers…), au-dessus des sessions.
//!
//! Le moteur ne touche ni aux sockets, ni aux fils, ni à l'interface : l'hôte lui passe ce qui arrive et applique
//! les [`Effect`] qu'il rend. Tout ce qui dépend de l'heure reçoit `now_ms` (ms depuis 1970).
//!
//! Autour : [`settings`] (réglages TOML), [`i18n`] (sept langues), [`timeutil`] (dates), [`dcc`] (protocole
//! DCC : offres, noms de fichiers sûrs).

pub mod client;
pub mod dcc;
pub mod i18n;
pub mod session;
pub mod settings;
pub mod timeutil;

pub use client::{
    Activity, CaughtUrl, ChanEntry, ChanList, Client, ConnId, ConnInfo, ConnState, DccAnswer, DccId, DccPrompt, DccType,
    DccUi, Effect, Line, LineKind, NickEntry, NotifyStatus, TlsVerify, Transfer, TransferState, UiDialog, WinId, WinKind,
    WinState, Window,
};
pub use settings::Settings;
