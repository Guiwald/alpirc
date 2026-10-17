//! Identification des appareils : empreintes argon2id des mots de passe, lecture des identifiants
//! (`PASS utilisateur[/réseau][@appareil]:mot-de-passe`, SASL PLAIN), limitation des essais ratés.

use std::collections::HashMap;
use std::net::IpAddr;

use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use base64::Engine as _;

/// Empreinte argon2id (paramètres par défaut d'argon2 : 19 Mio, 2 passes, 1 fil) au format PHC
/// (`$argon2id$v=19$m=19456,t=2,p=1$…`).
pub fn hash_password(password: &str) -> Result<String, String> {
    let mut salt = [0u8; 16];
    getrandom::fill(&mut salt).map_err(|e| format!("aléa indisponible : {e}"))?;
    let salt = SaltString::encode_b64(&salt).map_err(|e| format!("sel : {e}"))?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("argon2 : {e}"))
}

/// Vérifie un mot de passe contre son empreinte (comparaison en temps constant). Une empreinte illisible
/// n'accepte rien.
pub fn verify_password(hash: &str, password: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(h) => Argon2::default().verify_password(password.as_bytes(), &h).is_ok(),
        Err(_) => false,
    }
}

/// Vrai si la chaîne ressemble à une empreinte PHC lisible.
pub fn is_valid_hash(hash: &str) -> bool {
    PasswordHash::new(hash).is_ok()
}

/// Ce que l'appareil annonce : `utilisateur[/réseau][@appareil]`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Login {
    pub user: String,
    /// Réseau à lier (identifiant ou nom).
    pub network: Option<String>,
    /// Nom de l'appareil (informatif).
    pub client: Option<String>,
}

/// Lit `utilisateur[/réseau][@appareil]` (aussi `utilisateur@appareil/réseau`).
pub fn parse_login(s: &str) -> Login {
    let s = s.trim();
    let end = s.find(['/', '@']).unwrap_or(s.len());
    let mut login = Login { user: s[..end].to_string(), ..Login::default() };
    let mut rest = &s[end..];
    while let Some(c) = rest.chars().next() {
        let body = &rest[1..];
        let stop = body.find(['/', '@']).unwrap_or(body.len());
        let value = body[..stop].to_string();
        match c {
            '/' if !value.is_empty() => login.network = Some(value),
            '@' if !value.is_empty() => login.client = Some(value),
            _ => {}
        }
        rest = &body[stop..];
    }
    login
}

/// Lit le mot de passe de serveur : `utilisateur[/réseau][@appareil]:mot-de-passe`. Sans `:`, rend `None`
/// (le mot de passe seul, l'utilisateur venant alors de `USER`).
pub fn parse_pass(pass: &str) -> Option<(Login, String)> {
    let (login, password) = pass.split_once(':')?;
    if login.is_empty() {
        return None;
    }
    Some((parse_login(login), password.to_string()))
}

/// Décode une réponse SASL PLAIN (base64 de `authzid NUL authcid NUL mot-de-passe`) → (authzid, authcid,
/// mot de passe).
pub fn decode_sasl_plain(b64: &str) -> Option<(String, String, String)> {
    let raw = base64::engine::general_purpose::STANDARD.decode(b64.trim()).ok()?;
    let mut parts = raw.split(|&b| b == 0);
    let authzid = String::from_utf8(parts.next()?.to_vec()).ok()?;
    let authcid = String::from_utf8(parts.next()?.to_vec()).ok()?;
    let password = String::from_utf8(parts.next()?.to_vec()).ok()?;
    if parts.next().is_some() || authcid.is_empty() {
        return None;
    }
    Some((authzid, authcid, password))
}

/// Nom d'utilisateur acceptable (lettres, chiffres, `.`, `_`, `-` ; 1 à 64 caractères) : il sert aussi de
/// nom de dossier.
pub fn valid_user_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
        && !name.starts_with('.')
}

/// Freine les essais de mots de passe : après `MAX_FAILURES` échecs en `WINDOW_MS` depuis une même adresse,
/// les essais suivants sont refusés sans calcul jusqu'à la fin de la fenêtre.
#[derive(Debug, Default)]
pub struct Throttle {
    failures: HashMap<IpAddr, (u32, i64)>,
}

const MAX_FAILURES: u32 = 8;
const WINDOW_MS: i64 = 60_000;

impl Throttle {
    /// Vrai si l'adresse peut encore essayer.
    pub fn allowed(&mut self, ip: IpAddr, now: i64) -> bool {
        match self.failures.get(&ip) {
            Some((n, start)) if now - start < WINDOW_MS => *n < MAX_FAILURES,
            Some(_) => {
                self.failures.remove(&ip);
                true
            }
            None => true,
        }
    }

    pub fn failed(&mut self, ip: IpAddr, now: i64) {
        let e = self.failures.entry(ip).or_insert((0, now));
        if now - e.1 >= WINDOW_MS {
            *e = (0, now);
        }
        e.0 += 1;
    }

    pub fn succeeded(&mut self, ip: IpAddr) {
        self.failures.remove(&ip);
    }

    /// Oublie les fenêtres terminées.
    pub fn prune(&mut self, now: i64) {
        self.failures.retain(|_, (_, start)| now - *start < WINDOW_MS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify() {
        let h = hash_password("correct horse").unwrap();
        assert!(h.starts_with("$argon2id$v=19$"), "{h}");
        assert!(is_valid_hash(&h));
        assert!(verify_password(&h, "correct horse"));
        assert!(!verify_password(&h, "wrong"));
        assert!(!verify_password("pas une empreinte", "x"));
        // Deux empreintes du même mot de passe diffèrent (sel).
        assert_ne!(h, hash_password("correct horse").unwrap());
    }

    #[test]
    fn logins() {
        assert_eq!(parse_login("gui"), Login { user: "gui".into(), network: None, client: None });
        assert_eq!(
            parse_login("gui/Libera"),
            Login { user: "gui".into(), network: Some("Libera".into()), client: None }
        );
        assert_eq!(
            parse_login("gui/Libera@phone"),
            Login { user: "gui".into(), network: Some("Libera".into()), client: Some("phone".into()) }
        );
        assert_eq!(
            parse_login("gui@phone/3"),
            Login { user: "gui".into(), network: Some("3".into()), client: Some("phone".into()) }
        );
        let (l, p) = parse_pass("gui/Undernet:mot:de:passe").unwrap();
        assert_eq!(l.user, "gui");
        assert_eq!(l.network.as_deref(), Some("Undernet"));
        assert_eq!(p, "mot:de:passe");
        assert!(parse_pass("motdepasse").is_none());
        assert!(parse_pass(":x").is_none());
    }

    #[test]
    fn sasl_plain() {
        let b = base64::engine::general_purpose::STANDARD.encode(b"\0gui/Libera\0secret");
        assert_eq!(decode_sasl_plain(&b), Some((String::new(), "gui/Libera".into(), "secret".into())));
        let b = base64::engine::general_purpose::STANDARD.encode(b"gui\0gui\0pw");
        assert_eq!(decode_sasl_plain(&b).unwrap().0, "gui");
        assert!(decode_sasl_plain("###").is_none());
        let b = base64::engine::general_purpose::STANDARD.encode(b"\0\0pw");
        assert!(decode_sasl_plain(&b).is_none());
    }

    #[test]
    fn user_names() {
        assert!(valid_user_name("guillaume"));
        assert!(valid_user_name("gui-2.test_x"));
        assert!(!valid_user_name(""));
        assert!(!valid_user_name("../etc"));
        assert!(!valid_user_name("a b"));
        assert!(!valid_user_name(".cache"));
    }

    #[test]
    fn throttle() {
        let ip: IpAddr = "192.0.2.1".parse().unwrap();
        let mut t = Throttle::default();
        for _ in 0..MAX_FAILURES {
            assert!(t.allowed(ip, 0));
            t.failed(ip, 0);
        }
        assert!(!t.allowed(ip, 1000));
        assert!(t.allowed(ip, WINDOW_MS + 1));
        t.failed(ip, 0);
        t.succeeded(ip);
        assert!(t.allowed(ip, 0));
    }
}
