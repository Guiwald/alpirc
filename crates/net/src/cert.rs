//! Certificats autosignés (ECDSA P-256, signés par graviola) pour le relais et les essais.
//!
//! Le certificat X.509 v3 est écrit à la main en DER : la structure est petite et fixe, et cela évite une
//! pile d'encodage ASN.1 en plus.

use std::net::IpAddr;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use graviola::hashing::Sha256;
use graviola::key_agreement::p256::StaticPrivateKey;
use graviola::signing::ecdsa::{P256, SigningKey};

/// Durée de validité : 10 ans (3652 jours, années bissextiles comprises).
const VALIDITY_DAYS: i64 = 3652;

// Identifiants d'objets (OID), déjà encodés en DER (étiquette 06 comprise).
const OID_ECDSA_SHA256: &[u8] = &[0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x04, 0x03, 0x02];
const OID_CN: &[u8] = &[0x06, 0x03, 0x55, 0x04, 0x03];
const OID_O: &[u8] = &[0x06, 0x03, 0x55, 0x04, 0x0a];
const OID_BASIC_CONSTRAINTS: &[u8] = &[0x06, 0x03, 0x55, 0x1d, 0x13];
const OID_KEY_USAGE: &[u8] = &[0x06, 0x03, 0x55, 0x1d, 0x0f];
const OID_EXT_KEY_USAGE: &[u8] = &[0x06, 0x03, 0x55, 0x1d, 0x25];
const OID_SUBJECT_ALT_NAME: &[u8] = &[0x06, 0x03, 0x55, 0x1d, 0x11];
const OID_SERVER_AUTH: &[u8] = &[0x06, 0x08, 0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x03, 0x01];
const OID_CLIENT_AUTH: &[u8] = &[0x06, 0x08, 0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x03, 0x02];

/// Génère un certificat autosigné ECDSA P-256 valable 10 ans pour les noms DNS ou adresses IP donnés.
///
/// Rend `(certificat PEM, clé privée PEM PKCS#8, empreinte SHA-256 hexadécimale)`. Le premier nom sert de
/// nom commun (CN) ; sans nom, le CN vaut « AlpIRC » et le certificat n'a pas d'autre nom (il ne peut alors
/// servir qu'épinglé). Les noms DNS doivent être en ASCII (punycode pour les noms internationaux).
pub fn generate_self_signed(names: &[String]) -> Result<(Vec<u8>, Vec<u8>, String), String> {
    let key = SigningKey::<P256> {
        private_key: StaticPrivateKey::new_random().map_err(|e| format!("génération de clé : {e:?}"))?,
    };
    let mut spki_buf = [0u8; 128];
    let spki = key.to_spki_der(&mut spki_buf).map_err(|e| format!("clé publique : {e:?}"))?.to_vec();
    let mut pkcs8_buf = [0u8; 256];
    let pkcs8 = key.to_pkcs8_der(&mut pkcs8_buf).map_err(|e| format!("clé privée : {e:?}"))?.to_vec();

    // Noms alternatifs (SAN).
    let mut general_names = Vec::new();
    for name in names {
        let name = name.trim();
        if let Ok(ip) = name.trim_start_matches('[').trim_end_matches(']').parse::<IpAddr>() {
            let octets = match ip {
                IpAddr::V4(v4) => v4.octets().to_vec(),
                IpAddr::V6(v6) => v6.octets().to_vec(),
            };
            general_names.extend(tlv(0x87, &octets)); // iPAddress [7]
        } else if !name.is_empty() && name.bytes().all(|b| b.is_ascii_graphic()) {
            general_names.extend(tlv(0x82, name.as_bytes())); // dNSName [2]
        } else {
            return Err(format!("nom invalide pour un certificat : « {name} » (ASCII attendu)"));
        }
    }

    let cn: String = names.first().map(|n| n.trim().to_string()).unwrap_or_else(|| "AlpIRC".into());
    let cn: String = cn.chars().take(64).collect();
    let name = seq(&[
        &tlv(0x31, &seq(&[OID_O, &tlv(0x0c, b"AlpIRC")])),
        &tlv(0x31, &seq(&[OID_CN, &tlv(0x0c, cn.as_bytes())])),
    ]);

    // Numéro de série : 16 octets aléatoires, positif, sans zéro de tête.
    let mut serial = [0u8; 16];
    graviola::random::fill(&mut serial).map_err(|e| format!("aléa : {e:?}"))?;
    serial[0] = (serial[0] & 0x7f) | 0x40;

    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    // Un jour de marge en arrière, pour les horloges un peu en retard.
    let validity = seq(&[&der_time(now - 86_400), &der_time(now + VALIDITY_DAYS * 86_400)]);

    let mut extensions = vec![
        // basicConstraints critique, cA absent (= faux) : certificat d'entité finale.
        extension(OID_BASIC_CONSTRAINTS, true, &seq(&[])),
        // keyUsage critique : digitalSignature.
        extension(OID_KEY_USAGE, true, &[0x03, 0x02, 0x07, 0x80]),
        // extKeyUsage : serveur et client (le même certificat peut servir au CertFP).
        extension(OID_EXT_KEY_USAGE, false, &seq(&[OID_SERVER_AUTH, OID_CLIENT_AUTH])),
    ];
    if !general_names.is_empty() {
        extensions.push(extension(OID_SUBJECT_ALT_NAME, false, &tlv(0x30, &general_names)));
    }
    let extensions = tlv(0xa3, &tlv(0x30, &extensions.concat()));

    let sig_alg = seq(&[OID_ECDSA_SHA256]);
    let tbs = seq(&[
        &[0xa0, 0x03, 0x02, 0x01, 0x02], // version v3
        &tlv(0x02, &serial),
        &sig_alg,
        &name, // émetteur
        &validity,
        &name, // sujet
        &spki,
        &extensions,
    ]);

    let mut sig_buf = [0u8; 128];
    let sig = key.sign_asn1::<Sha256>(&[&tbs], &mut sig_buf).map_err(|e| format!("signature : {e:?}"))?;
    let mut sig_bits = Vec::with_capacity(sig.len() + 1);
    sig_bits.push(0); // aucun bit inutilisé
    sig_bits.extend_from_slice(sig);
    let cert = seq(&[&tbs, &sig_alg, &tlv(0x03, &sig_bits)]);

    let fp = crate::tls::fingerprint(&cert);
    Ok((pem("CERTIFICATE", &cert), pem("PRIVATE KEY", &pkcs8), fp))
}

/// Encode une extension X.509.
fn extension(oid: &[u8], critical: bool, value: &[u8]) -> Vec<u8> {
    let crit: &[u8] = if critical { &[0x01, 0x01, 0xff] } else { &[] };
    seq(&[oid, crit, &tlv(0x04, value)])
}

/// Élément DER : étiquette, longueur, contenu.
fn tlv(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(content.len() + 4);
    out.push(tag);
    let len = content.len();
    if len < 0x80 {
        out.push(len as u8);
    } else {
        let bytes = len.to_be_bytes();
        let skip = bytes.iter().take_while(|&&b| b == 0).count();
        out.push(0x80 | (bytes.len() - skip) as u8);
        out.extend_from_slice(&bytes[skip..]);
    }
    out.extend_from_slice(content);
    out
}

fn seq(parts: &[&[u8]]) -> Vec<u8> {
    tlv(0x30, &parts.concat())
}

/// Date X.509 : UTCTime jusqu'en 2049, GeneralizedTime ensuite (RFC 5280).
fn der_time(unix: i64) -> Vec<u8> {
    let (y, mo, d, h, mi, s) = civil(unix);
    if (1950..2050).contains(&y) {
        tlv(0x17, format!("{:02}{mo:02}{d:02}{h:02}{mi:02}{s:02}Z", y % 100).as_bytes())
    } else {
        tlv(0x18, format!("{y:04}{mo:02}{d:02}{h:02}{mi:02}{s:02}Z").as_bytes())
    }
}

/// Date civile (UTC) d'un instant Unix : (année, mois, jour, heure, minute, seconde).
pub(crate) fn civil(unix: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = unix.div_euclid(86_400);
    let secs = unix.rem_euclid(86_400);
    // Algorithme « civil_from_days » de Howard Hinnant.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m as u32, d as u32, (secs / 3600) as u32, (secs % 3600 / 60) as u32, (secs % 60) as u32)
}

/// Date au format « JJ/MM/AAAA » (messages d'erreur).
pub(crate) fn french_date(unix: i64) -> String {
    let (y, m, d, ..) = civil(unix);
    format!("{d:02}/{m:02}/{y}")
}

fn pem(label: &str, der: &[u8]) -> Vec<u8> {
    let b64 = base64::engine::general_purpose::STANDARD.encode(der);
    let mut s = format!("-----BEGIN {label}-----\n");
    for chunk in b64.as_bytes().chunks(64) {
        s.push_str(std::str::from_utf8(chunk).unwrap_or_default());
        s.push('\n');
    }
    s.push_str(&format!("-----END {label}-----\n"));
    s.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustls::client::danger::ServerCertVerifier;
    use rustls::pki_types::{ServerName, UnixTime};
    use std::sync::Arc;

    #[test]
    fn civil_dates() {
        assert_eq!(civil(0), (1970, 1, 1, 0, 0, 0));
        assert_eq!(civil(951_782_400), (2000, 2, 29, 0, 0, 0));
        assert_eq!(civil(1_700_000_000), (2023, 11, 14, 22, 13, 20));
        assert_eq!(civil(-1), (1969, 12, 31, 23, 59, 59));
        assert_eq!(french_date(4_102_444_800), "01/01/2100");
        assert_eq!(der_time(1_700_000_000), tlv(0x17, b"231114221320Z"));
        assert_eq!(der_time(2_524_608_000), tlv(0x18, b"20500101000000Z"));
    }

    #[test]
    fn der_lengths() {
        assert_eq!(tlv(0x04, &[0; 3])[..2], [0x04, 3]);
        assert_eq!(tlv(0x04, &[0; 200])[..3], [0x04, 0x81, 200]);
        assert_eq!(tlv(0x04, &[0; 300])[..4], [0x04, 0x82, 0x01, 0x2c]);
    }

    /// Le certificat généré passe la vérification webpki complète quand on lui fait confiance comme
    /// racine : il est donc bien formé (structure, signature, dates, noms, usages).
    #[test]
    fn generated_cert_is_valid_x509() {
        let names = vec!["localhost".to_string(), "127.0.0.1".to_string(), "::1".to_string()];
        let (cert_pem, key_pem, fp) = generate_self_signed(&names).unwrap();
        let chain = crate::tls::load_chain(&cert_pem).unwrap();
        assert_eq!(chain.len(), 1);
        assert_eq!(crate::tls::fingerprint(chain[0].as_ref()), fp);
        assert!(crate::tls::load_key(&key_pem).is_ok());

        let mut roots = rustls::RootCertStore::empty();
        roots.add(chain[0].clone()).unwrap();
        let verifier =
            rustls::client::WebPkiServerVerifier::builder_with_provider(Arc::new(roots), crate::tls::provider())
                .build()
                .unwrap();
        let now = UnixTime::now();
        for name in ["localhost", "127.0.0.1", "::1"] {
            let sn = ServerName::try_from(name.to_string()).unwrap();
            verifier.verify_server_cert(&chain[0], &[], &sn, &[], now).unwrap();
        }
        // Encore valable dans 9 ans, plus dans 11 ans.
        let in_9y = UnixTime::since_unix_epoch(std::time::Duration::from_secs(now.as_secs() + 9 * 365 * 86_400));
        verifier.verify_server_cert(&chain[0], &[], &ServerName::try_from("localhost").unwrap(), &[], in_9y).unwrap();
        let in_11y = UnixTime::since_unix_epoch(std::time::Duration::from_secs(now.as_secs() + 11 * 365 * 86_400));
        let err = verifier
            .verify_server_cert(&chain[0], &[], &ServerName::try_from("localhost").unwrap(), &[], in_11y)
            .unwrap_err();
        assert!(matches!(err, rustls::Error::InvalidCertificate(rustls::CertificateError::ExpiredContext { .. })));
        // Mauvais nom.
        let err = verifier
            .verify_server_cert(&chain[0], &[], &ServerName::try_from("example.org").unwrap(), &[], now)
            .unwrap_err();
        assert!(matches!(
            err,
            rustls::Error::InvalidCertificate(
                rustls::CertificateError::NotValidForName | rustls::CertificateError::NotValidForNameContext { .. }
            )
        ));
        // La clé correspond au certificat (rustls le contrôle en construisant la configuration serveur).
        crate::tls::server_config(&crate::ServerTls { cert_chain_pem: cert_pem, key_pem }).unwrap();
    }

    #[test]
    fn names() {
        let (_, _, a) = generate_self_signed(&[]).unwrap();
        let (_, _, b) = generate_self_signed(&[]).unwrap();
        assert_ne!(a, b);
        assert!(generate_self_signed(&["é.example".to_string()]).is_err());
        assert!(generate_self_signed(&["*.example.org".to_string(), "[::1]".to_string()]).is_ok());
    }
}
