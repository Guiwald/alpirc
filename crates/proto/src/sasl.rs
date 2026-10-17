//! SASL (IRCv3) : PLAIN, EXTERNAL et SCRAM-SHA-256 (RFC 5802, RFC 7677), et découpage en `AUTHENTICATE` de 400.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

type HmacSha256 = Hmac<Sha256>;

/// Mécanismes gérés, du préféré au moins préféré.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mechanism {
    External,
    ScramSha256,
    Plain,
}

impl Mechanism {
    pub fn name(self) -> &'static str {
        match self {
            Mechanism::External => "EXTERNAL",
            Mechanism::ScramSha256 => "SCRAM-SHA-256",
            Mechanism::Plain => "PLAIN",
        }
    }

    pub fn from_name(s: &str) -> Option<Mechanism> {
        match s.to_ascii_uppercase().as_str() {
            "EXTERNAL" => Some(Mechanism::External),
            "SCRAM-SHA-256" => Some(Mechanism::ScramSha256),
            "PLAIN" => Some(Mechanism::Plain),
            _ => None,
        }
    }
}

/// Réponse PLAIN : `authzid \0 authcid \0 mot de passe`, en base64.
pub fn plain(authcid: &str, password: &str) -> String {
    B64.encode(format!("{authcid}\0{authcid}\0{password}"))
}

/// Découpe une réponse base64 en paramètres d'`AUTHENTICATE` (400 caractères au plus ; `+` pour vide ou pour
/// marquer la fin quand le dernier morceau fait exactement 400).
pub fn chunks(b64: &str) -> Vec<String> {
    if b64.is_empty() {
        return vec!["+".into()];
    }
    let mut out: Vec<String> = b64.as_bytes().chunks(400).map(|c| String::from_utf8_lossy(c).into_owned()).collect();
    if b64.len() % 400 == 0 {
        out.push("+".into());
    }
    out
}

/// Assemble les morceaux reçus du serveur ; `None` tant qu'il en manque.
#[derive(Default)]
pub struct Reassembler {
    buf: String,
}

impl Reassembler {
    pub fn push(&mut self, part: &str) -> Option<Vec<u8>> {
        if part != "+" {
            self.buf.push_str(part);
        }
        if part.len() == 400 {
            return None;
        }
        let data = std::mem::take(&mut self.buf);
        Some(B64.decode(data).unwrap_or_default())
    }
}

/// Client SCRAM-SHA-256.
pub struct Scram {
    user: String,
    password: String,
    nonce: String,
    client_first_bare: String,
    server_signature: Option<Vec<u8>>,
}

fn escape_user(u: &str) -> String {
    u.replace('=', "=3D").replace(',', "=2C")
}

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut m = HmacSha256::new_from_slice(key).expect("HMAC accepte toutes les clés");
    m.update(data);
    m.finalize().into_bytes().to_vec()
}

impl Scram {
    pub fn new(user: &str, password: &str) -> Scram {
        let mut raw = [0u8; 18];
        getrandom::fill(&mut raw).expect("aléa du système");
        Scram::with_nonce(user, password, &B64.encode(raw))
    }

    pub fn with_nonce(user: &str, password: &str, nonce: &str) -> Scram {
        let bare = format!("n={},r={}", escape_user(user), nonce);
        Scram {
            user: user.to_string(),
            password: password.to_string(),
            nonce: nonce.to_string(),
            client_first_bare: bare,
            server_signature: None,
        }
    }

    pub fn user(&self) -> &str {
        &self.user
    }

    /// Premier message du client (non encodé).
    pub fn client_first(&self) -> String {
        format!("n,,{}", self.client_first_bare)
    }

    /// Lit le premier message du serveur et rend le message final du client.
    pub fn client_final(&mut self, server_first: &str) -> Result<String, &'static str> {
        let mut r = None;
        let mut s = None;
        let mut i = None;
        for part in server_first.split(',') {
            if let Some(v) = part.strip_prefix("r=") {
                r = Some(v);
            } else if let Some(v) = part.strip_prefix("s=") {
                s = Some(v);
            } else if let Some(v) = part.strip_prefix("i=") {
                i = v.parse::<u32>().ok();
            }
        }
        let (r, s, i) = (r.ok_or("nonce absent")?, s.ok_or("sel absent")?, i.ok_or("itérations absentes")?);
        if !r.starts_with(&self.nonce) {
            return Err("nonce du serveur invalide");
        }
        if i > 1_000_000 {
            return Err("trop d'itérations");
        }
        let salt = B64.decode(s).map_err(|_| "sel illisible")?;
        let mut salted = [0u8; 32];
        pbkdf2::pbkdf2::<HmacSha256>(self.password.as_bytes(), &salt, i, &mut salted).map_err(|_| "pbkdf2")?;
        let client_key = hmac(&salted, b"Client Key");
        let stored_key = Sha256::digest(&client_key);
        let without_proof = format!("c=biws,r={r}");
        let auth = format!("{},{},{}", self.client_first_bare, server_first, without_proof);
        let client_sig = hmac(&stored_key, auth.as_bytes());
        let proof: Vec<u8> = client_key.iter().zip(client_sig.iter()).map(|(a, b)| a ^ b).collect();
        let server_key = hmac(&salted, b"Server Key");
        self.server_signature = Some(hmac(&server_key, auth.as_bytes()));
        Ok(format!("{without_proof},p={}", B64.encode(proof)))
    }

    /// Vérifie le message final du serveur (`v=…`).
    pub fn verify(&self, server_final: &str) -> bool {
        let Some(v) = server_final.split(',').find_map(|p| p.strip_prefix("v=")) else { return false };
        match (&self.server_signature, B64.decode(v)) {
            (Some(expected), Ok(got)) => *expected == got,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc7677_vector() {
        let mut s = Scram::with_nonce("user", "pencil", "rOprNGfwEbeRWgbNEkqO");
        assert_eq!(s.client_first(), "n,,n=user,r=rOprNGfwEbeRWgbNEkqO");
        let fin = s
            .client_final("r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,s=W22ZaJ0SNY7soEsUEjb6gQ==,i=4096")
            .unwrap();
        assert_eq!(
            fin,
            "c=biws,r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,p=dHzbZapWIk4jUhN+Ute9ytag9zjfMHgsqmmiz7AndVQ="
        );
        assert!(s.verify("v=6rriTRBi23WpRR/wtup+mMhUZUn/dB5nLTJRsjl95G4="));
        assert!(!s.verify("v=AAAA"));
    }

    #[test]
    fn plain_and_chunks() {
        assert_eq!(plain("gui", "pw"), B64.encode("gui\0gui\0pw"));
        let long = "a".repeat(800);
        let c = chunks(&long);
        assert_eq!(c.len(), 3);
        assert_eq!(c[2], "+");
        assert_eq!(chunks(""), vec!["+"]);
        let mut r = Reassembler::default();
        assert_eq!(r.push("+"), Some(vec![]));
    }
}
