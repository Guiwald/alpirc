//! Configuration TLS : fournisseur cryptographique graviola, racines de confiance, vérificateurs de
//! certificat (complet, sans vérification, épinglé), certificat client, configuration serveur.

use std::sync::{Arc, Mutex, OnceLock};

use rustls::client::WebPkiServerVerifier;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{CryptoProvider, WebPkiSupportedAlgorithms};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::{CertificateError, DigitallySignedStruct, SignatureScheme};

use crate::{CertProblem, NetError, PemProblem, ServerTls, TlsInfo, Verify};

/// Versions de TLS acceptées : 1.3 de préférence, 1.2 pour les serveurs plus anciens.
static VERSIONS: &[&rustls::SupportedProtocolVersion] = &[&rustls::version::TLS13, &rustls::version::TLS12];

/// Le fournisseur graviola (partagé).
pub(crate) fn provider() -> Arc<CryptoProvider> {
    static P: OnceLock<Arc<CryptoProvider>> = OnceLock::new();
    P.get_or_init(|| Arc::new(rustls_graviola::default_provider())).clone()
}

/// Empreinte SHA-256 d'un certificat DER, en hexadécimal minuscule sans séparateur.
pub fn fingerprint(der: &[u8]) -> String {
    let mut ctx = graviola::hashing::sha2::Sha256Context::new();
    ctx.update(der);
    hex(&ctx.finish())
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(DIGITS[(b >> 4) as usize] as char);
        s.push(DIGITS[(b & 15) as usize] as char);
    }
    s
}

/// Normalise une empreinte saisie (majuscules, `:` ou espaces acceptés) en 64 chiffres hexadécimaux minuscules.
pub(crate) fn parse_pin(s: &str) -> Result<String, NetError> {
    let hex: String = s.chars().filter(|c| *c != ':' && !c.is_whitespace()).map(|c| c.to_ascii_lowercase()).collect();
    if hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(hex)
    } else {
        Err(NetError::BadPin { pin: s.to_string() })
    }
}

/// Racines de confiance : celles de Mozilla, plus celles du système (dont les autorités ajoutées par une
/// entreprise ou par l'utilisateur, et `SSL_CERT_FILE`).
fn root_store() -> rustls::RootCertStore {
    let mut roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
    #[cfg(not(target_os = "android"))]
    {
        let native = rustls_native_certs::load_native_certs();
        for e in &native.errors {
            log::debug!("racines du système : {e}");
        }
        let (added, _) = roots.add_parsable_certificates(native.certs);
        log::debug!("{added} certificats racines du système");
    }
    #[cfg(target_os = "android")]
    {
        // Pas d'API système accessible sans Java : on lit les dossiers de certificats d'Android.
        for dir in
            ["/system/etc/security/cacerts", "/apex/com.android.conscrypt/cacerts", "/data/misc/user/0/cacerts-added"]
        {
            let Ok(rd) = std::fs::read_dir(dir) else { continue };
            for e in rd.flatten() {
                if let Ok(bytes) = std::fs::read(e.path()) {
                    let certs: Vec<CertificateDer<'static>> =
                        CertificateDer::pem_slice_iter(&bytes).flatten().collect();
                    roots.add_parsable_certificates(certs);
                }
            }
        }
    }
    roots
}

/// Vérificateur webpki standard (construit une fois : charger les racines du système prend du temps).
fn webpki_verifier() -> Result<Arc<WebPkiServerVerifier>, NetError> {
    static V: OnceLock<Result<Arc<WebPkiServerVerifier>, NetError>> = OnceLock::new();
    V.get_or_init(|| {
        WebPkiServerVerifier::builder_with_provider(Arc::new(root_store()), provider())
            .build()
            .map_err(|e| NetError::TlsSetup { detail: format!("trusted roots: {e}") })
    })
    .clone()
}

#[derive(Debug)]
enum Mode {
    Full(Arc<WebPkiServerVerifier>),
    AcceptAny,
    Pinned(String),
}

/// Vérificateur de certificat serveur. Il note l'empreinte du certificat présenté (pour la proposer à
/// l'utilisateur en cas de refus) puis applique le mode choisi. Les signatures de la négociation sont
/// toujours vérifiées : le serveur doit détenir la clé du certificat, même en mode épinglé ou permissif.
#[derive(Debug)]
struct CertVerifier {
    mode: Mode,
    algs: WebPkiSupportedAlgorithms,
    seen: Option<Arc<Mutex<Option<String>>>>,
}

impl ServerCertVerifier for CertVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let fp = fingerprint(end_entity.as_ref());
        if let Some(seen) = &self.seen {
            *seen.lock().unwrap_or_else(|e| e.into_inner()) = Some(fp.clone());
        }
        match &self.mode {
            Mode::Full(v) => v.verify_server_cert(end_entity, intermediates, server_name, ocsp_response, now),
            Mode::AcceptAny => Ok(ServerCertVerified::assertion()),
            Mode::Pinned(expected) if *expected == fp => Ok(ServerCertVerified::assertion()),
            // Seul ce vérificateur produit cette erreur : elle signifie « empreinte différente ».
            Mode::Pinned(_) => Err(rustls::Error::InvalidCertificate(CertificateError::ApplicationVerificationFailure)),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.algs)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.algs)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algs.supported_schemes()
    }
}

/// Lit une chaîne de certificats PEM.
pub(crate) fn load_chain(pem: &[u8]) -> Result<Vec<CertificateDer<'static>>, PemProblem> {
    let certs = CertificateDer::pem_slice_iter(pem)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| PemProblem::BadCert(e.to_string()))?;
    if certs.is_empty() {
        return Err(PemProblem::NoCert);
    }
    Ok(certs)
}

/// Lit une clé privée PEM (PKCS#8, SEC1 « EC PRIVATE KEY » ou PKCS#1 « RSA PRIVATE KEY »).
pub(crate) fn load_key(pem: &[u8]) -> Result<PrivateKeyDer<'static>, PemProblem> {
    PrivateKeyDer::from_pem_slice(pem).map_err(|e| PemProblem::BadKey(e.to_string()))
}

/// Configuration client ; `seen` reçoit l'empreinte du certificat présenté par le serveur.
pub(crate) fn build_client_config(
    verify: &Verify,
    client_cert: Option<(&[u8], &[u8])>,
    seen: Option<Arc<Mutex<Option<String>>>>,
) -> Result<Arc<rustls::ClientConfig>, NetError> {
    let mode = match verify {
        Verify::Full => Mode::Full(webpki_verifier()?),
        Verify::AcceptInvalid => Mode::AcceptAny,
        Verify::Pinned(p) => Mode::Pinned(parse_pin(p)?),
    };
    let prov = provider();
    let verifier = Arc::new(CertVerifier { mode, algs: prov.signature_verification_algorithms, seen });
    let builder = rustls::ClientConfig::builder_with_provider(prov)
        .with_protocol_versions(VERSIONS)
        .map_err(|e| NetError::TlsSetup { detail: e.to_string() })?
        .dangerous()
        .with_custom_certificate_verifier(verifier);
    // Pas d'ALPN : IRC n'en définit pas. La reprise de session reste propre à cette configuration, pour
    // qu'une session ouverte en mode permissif ne serve jamais à court-circuiter une vérification complète.
    let cfg = match client_cert {
        None => builder.with_no_client_auth(),
        Some((chain, key)) => {
            let certs = load_chain(chain).map_err(NetError::ClientCert)?;
            let key = load_key(key).map_err(NetError::ClientCert)?;
            builder
                .with_client_auth_cert(certs, key)
                .map_err(|e| NetError::ClientCert(PemProblem::Rejected(e.to_string())))?
        }
    };
    Ok(Arc::new(cfg))
}

/// Configuration serveur (relais) à partir d'un certificat et d'une clé PEM. Le texte de l'erreur sert aux
/// journaux du relais.
pub(crate) fn server_config(tls: &ServerTls) -> Result<Arc<rustls::ServerConfig>, String> {
    let certs = load_chain(&tls.cert_chain_pem).map_err(|e| format!("certificat du serveur : {e}"))?;
    let key = load_key(&tls.key_pem).map_err(|e| format!("certificat du serveur : {e}"))?;
    let cfg = rustls::ServerConfig::builder_with_provider(provider())
        .with_protocol_versions(VERSIONS)
        .map_err(|e| format!("TLS : {e}"))?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| format!("certificat du serveur : {e}"))?;
    Ok(Arc::new(cfg))
}

/// Informations sur une session TLS établie.
pub(crate) fn tls_info(state: &rustls::CommonState, verified: bool) -> TlsInfo {
    let protocol = match state.protocol_version() {
        Some(rustls::ProtocolVersion::TLSv1_3) => "TLSv1.3".to_string(),
        Some(rustls::ProtocolVersion::TLSv1_2) => "TLSv1.2".to_string(),
        Some(v) => format!("{v:?}"),
        None => String::new(),
    };
    let cipher = state
        .negotiated_cipher_suite()
        .map(|s| s.suite().as_str().map(str::to_string).unwrap_or_else(|| format!("{:?}", s.suite())))
        .unwrap_or_default();
    let cert_sha256 =
        state.peer_certificates().and_then(|c| c.first()).map(|c| fingerprint(c.as_ref())).unwrap_or_default();
    TlsInfo { protocol, cipher, cert_sha256, verified }
}

/// L'échec d'une négociation TLS, décrit par ses données.
pub(crate) fn tls_failure(e: &rustls::Error, host: &str) -> NetError {
    match e {
        rustls::Error::InvalidCertificate(ce) => {
            NetError::CertRejected { host: host.to_string(), problem: cert_problem(ce) }
        }
        rustls::Error::AlertReceived(a) => NetError::TlsAlert { alert: format!("{a:?}") },
        rustls::Error::NoCertificatesPresented => NetError::NoServerCert,
        other => NetError::Tls { detail: other.to_string() },
    }
}

#[allow(deprecated)] // `UnsupportedSignatureAlgorithm` : variante ancienne, encore possible.
fn cert_problem(ce: &CertificateError) -> CertProblem {
    match ce {
        CertificateError::Expired => CertProblem::Expired { not_after: None },
        CertificateError::ExpiredContext { not_after, .. } => {
            CertProblem::Expired { not_after: Some(not_after.as_secs() as i64) }
        }
        CertificateError::NotValidYet => CertProblem::NotValidYet { not_before: None },
        CertificateError::NotValidYetContext { not_before, .. } => {
            CertProblem::NotValidYet { not_before: Some(not_before.as_secs() as i64) }
        }
        CertificateError::UnknownIssuer => CertProblem::UnknownIssuer,
        CertificateError::NotValidForName => CertProblem::NameMismatch { presented: Vec::new() },
        CertificateError::NotValidForNameContext { presented, .. } => {
            CertProblem::NameMismatch { presented: presented.clone() }
        }
        CertificateError::BadSignature => CertProblem::BadSignature,
        CertificateError::Revoked => CertProblem::Revoked,
        CertificateError::BadEncoding => CertProblem::BadEncoding,
        CertificateError::InvalidPurpose | CertificateError::InvalidPurposeContext { .. } => CertProblem::WrongPurpose,
        CertificateError::UnsupportedSignatureAlgorithm
        | CertificateError::UnsupportedSignatureAlgorithmContext { .. }
        | CertificateError::UnsupportedSignatureAlgorithmForPublicKeyContext { .. } => {
            CertProblem::UnsupportedAlgorithm
        }
        // Seul notre vérificateur épinglé produit cette erreur.
        CertificateError::ApplicationVerificationFailure => CertProblem::PinMismatch,
        other => CertProblem::Other(format!("{other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins() {
        let fp = fingerprint(b"abc");
        assert_eq!(fp, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let colons: Vec<String> = fp.as_bytes().chunks(2).map(|c| String::from_utf8_lossy(c).to_uppercase()).collect();
        assert_eq!(parse_pin(&colons.join(":")).unwrap(), fp);
        assert_eq!(parse_pin(&format!(" {} ", fp.to_uppercase())).unwrap(), fp);
        assert!(parse_pin("abcd").is_err());
        assert!(parse_pin(&"zz".repeat(32)).is_err());
    }
}
