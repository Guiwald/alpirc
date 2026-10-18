//! Protocole IRC d'AlpIRC, sans réseau ni état : de quoi lire et écrire les lignes d'IRC (RFC 1459, RFC 2812,
//! IRCv3), et les petits formats qui vont avec (CTCP, codes de couleur de mIRC, masques, SASL). Aussi, partagée
//! par `alpirc-net` (qui la produit) et `alpirc-core` (qui la rédige), la description des échecs du réseau
//! ([`neterror`]).

pub mod casemap;
pub mod ctcp;
pub mod encoding;
pub mod format;
pub mod isupport;
pub mod mask;
pub mod message;
pub mod modes;
pub mod neterror;
pub mod numeric;
pub mod sasl;
pub mod split;

pub use casemap::CaseMapping;
pub use isupport::ISupport;
pub use message::{Message, ParseError, Source, Tags};
