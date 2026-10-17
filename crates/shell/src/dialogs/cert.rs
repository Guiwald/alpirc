//! Certificat refusé : quand une connexion TLS échoue parce que le certificat du serveur n'a pas pu être vérifié,
//! la boîte montre l'erreur et l'empreinte SHA-256, et propose de faire confiance à ce certificat (empreinte
//! retenue, comme SSH) ou de ne plus vérifier ce serveur ; dans les deux cas le serveur est mis à jour dans les
//! réglages et la fenêtre Status s'y reconnecte. Annuler arrête les nouveaux essais.

use alpirc_core::ConnId;
use alpirc_core::i18n::Lang;
use alpirc_core::settings::{Network, Server, Verify};
use alpirc_ui::{ControlKind, ControlSpec, DialogMsg, DialogSpec, DialogView};

use super::network::normalize_fp;
use super::*;

const W: i32 = 500;
const TRUST: u32 = 1;
const IGNORE: u32 = 2;
const CANCEL: u32 = 3;

/// La boîte du certificat refusé.
pub struct CertPrompt {
    pub status: WinId,
    pub host: String,
    pub port: u16,
    pub error: String,
    /// Empreinte SHA-256 (hexadécimal minuscule).
    pub fp: String,
    /// Réseau des réglages et numéro du serveur, s'ils sont connus.
    pub network: Option<(String, Option<usize>)>,
}

/// Empreinte lisible : `AB:CD:…`, en deux lignes.
fn pretty_fp(fp: &str) -> String {
    let bytes: Vec<String> = fp.as_bytes().chunks(2).map(|c| String::from_utf8_lossy(c).to_ascii_uppercase()).collect();
    let half = bytes.len().div_ceil(2);
    if bytes.len() > 16 { format!("{}\n{}", bytes[..half].join(":"), bytes[half..].join(":")) } else { bytes.join(":") }
}

impl CertPrompt {
    /// La boîte pour l'échec de la connexion `conn` (avant que le moteur ne passe au serveur suivant).
    pub fn new(client: &Client, conn: ConnId, error: &str, fp: &str) -> Option<CertPrompt> {
        let info = client.conn_info(conn)?;
        let s = client.settings();
        let net = s
            .network(&info.network)
            .filter(|n| n.servers.iter().any(|x| x.host.eq_ignore_ascii_case(&info.server)))
            .or_else(|| s.network_for_host(&info.server));
        let network = net.map(|n| {
            let idx = n
                .servers
                .iter()
                .position(|x| x.host.eq_ignore_ascii_case(&info.server) && x.port == info.port)
                .or_else(|| n.servers.iter().position(|x| x.host.eq_ignore_ascii_case(&info.server)));
            (n.name.clone(), idx)
        });
        Some(CertPrompt { status: info.status_window, host: info.server, port: info.port, error: error.to_string(), fp: normalize_fp(fp), network })
    }

    /// Écrit la décision dans les réglages ; rend le réseau et le numéro du serveur où se reconnecter.
    fn remember(&self, settings: &mut alpirc_core::Settings, verify: Verify) -> (String, usize) {
        let fp = (verify == Verify::Pinned).then(|| self.fp.clone());
        if let Some((name, idx)) = &self.network
            && let Some(n) = settings.network_mut(name)
        {
            let i = match idx.filter(|&i| i < n.servers.len()) {
                Some(i) => i,
                None => {
                    n.servers.push(Server::new(&self.host, self.port, true));
                    n.servers.len() - 1
                }
            };
            let srv = &mut n.servers[i];
            srv.tls = true;
            srv.verify = verify;
            if fp.is_some() {
                srv.fingerprint = fp;
            }
            return (n.name.clone(), i);
        }
        // Serveur hors des réseaux (`/server hôte +port`) : un réseau à son nom.
        let mut name = self.host.clone();
        let mut k = 2;
        while settings.network(&name).is_some() {
            name = format!("{} ({k})", self.host);
            k += 1;
        }
        let mut srv = Server::new(&self.host, self.port, true);
        srv.verify = verify;
        srv.fingerprint = fp;
        settings.networks.push(Network::new(&name, vec![srv]));
        (name, 0)
    }

    fn decide(&self, cx: &mut Cx, verify: Verify) {
        let (name, idx) = self.remember(cx.client.settings_mut(), verify);
        cx.actions.push(Action::SettingsChanged);
        cx.client.connect(self.status, &name, Some(idx), cx.now);
    }
}

impl DialogCtl for CertPrompt {
    fn kind(&self) -> Option<String> {
        Some(format!("cert:{}:{}", self.host.to_lowercase(), self.port))
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let tx = MARGIN + 4 + 32 + 12;
        let tw = W - tx - MARGIN - 4;
        let body = tf(lang, "cert.text", &[("host", &self.host), ("port", &self.port.to_string()), ("error", &self.error)]);
        let bh = text_rows(&body, tw) * 16;
        let mut icon = ControlSpec::new(9, ControlKind::Icon, (MARGIN + 4, MARGIN + 4, 32, 32), "ui/msg_warning_32");
        icon.styles.push("noborder".into());
        let mut y = MARGIN + 4;
        let mut c = vec![icon, text(4, (tx, y, tw, bh), &body)];
        y += bh + 10;
        c.push(text(5, (tx, y, tw, LABEL_H), &t(lang, "cert.fingerprint")));
        y += LABEL_H + 2;
        c.push(text(6, (tx + 8, y, tw - 8, 32), &pretty_fp(&self.fp)));
        y += 32 + 10;
        let advice = t(lang, "cert.advice");
        let ah = text_rows(&advice, tw) * 16;
        c.push(text(7, (tx, y, tw, ah), &advice));
        y += ah + 16;
        let bw = [168, 206, BTN_W];
        let mut x = W - MARGIN - bw.iter().sum::<i32>() - 12;
        for (k, (id, key)) in [(TRUST, "cert.trust"), (IGNORE, "cert.ignore"), (CANCEL, "dlg.cancel")].into_iter().enumerate() {
            let mut b = button(id, (x, y, bw[k], BTN_H), &t(lang, key));
            if id == TRUST {
                b = b.style("default");
            }
            if id == CANCEL {
                b = b.style("cancel");
            }
            c.push(b);
            x += bw[k] + 6;
        }
        spec(&t(lang, "cert.title"), W, y + BTN_H + MARGIN + 4, c)
    }

    fn init(&mut self, view: &mut DialogView, _cx: &mut Cx) {
        view.set_focus(CANCEL);
    }

    fn on(&mut self, msg: DialogMsg, _view: &mut DialogView, cx: &mut Cx) -> Outcome {
        match msg {
            DialogMsg::Clicked(TRUST) => {
                self.decide(cx, Verify::Pinned);
                Outcome::Close
            }
            DialogMsg::Clicked(IGNORE) => {
                self.decide(cx, Verify::AcceptInvalid);
                Outcome::Close
            }
            DialogMsg::Cancel | DialogMsg::Clicked(CANCEL) => {
                // Pas de nouvel essai vers un serveur dont on refuse le certificat.
                cx.client.disconnect(self.status, None, cx.now);
                Outcome::Close
            }
            _ => Outcome::Keep,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testkit::{Kit, assert_translated};

    #[test]
    fn trust_pins_the_fingerprint_and_reconnects() {
        let mut k = Kit::new("cert");
        k.client.settings_mut().networks.insert(0, Network::new("Local", vec![Server::new("127.0.0.1", 16697, true)]));
        let st = k.client.windows()[0].id;
        k.client.connect(st, "Local", None, k.now);
        let conn = k.client.conns()[0];
        k.client.take_effects();
        let fp = "AB:CD:".repeat(16);
        let mut d = CertPrompt::new(&k.client, conn, "certificat inconnu", &fp).unwrap();
        assert_translated(&d);
        assert_eq!(d.network, Some(("Local".into(), Some(0))));
        let (mut v, _) = k.open(&mut d);
        let (o, a) = k.send(&mut d, &mut v, DialogMsg::Clicked(TRUST));
        assert_eq!(o, Outcome::Close);
        assert!(a.iter().any(|a| matches!(a, Action::SettingsChanged)));
        let srv = &k.client.settings().network("Local").unwrap().servers[0];
        assert_eq!(srv.verify, Verify::Pinned);
        assert_eq!(srv.fingerprint.as_deref(), Some("abcd".repeat(16).as_str()));
        let fx = k.client.take_effects();
        assert!(
            fx.iter().any(|e| matches!(e, alpirc_core::Effect::Connect { verify: alpirc_core::TlsVerify::Pinned(f), .. } if f == &"abcd".repeat(16))),
            "{fx:?}"
        );
    }

    #[test]
    fn unknown_server_gets_its_own_network() {
        let mut k = Kit::new("cert2");
        let st = k.client.windows()[0].id;
        k.client.command(st, "/server selfsigned.example +6697", k.now);
        let conn = k.client.conns()[0];
        let mut d = CertPrompt::new(&k.client, conn, "auto-signé", "00ff").unwrap();
        assert_eq!(d.network, None);
        let (mut v, _) = k.open(&mut d);
        k.send(&mut d, &mut v, DialogMsg::Clicked(IGNORE));
        let n = k.client.settings().network("selfsigned.example").unwrap();
        assert_eq!((n.servers[0].port, n.servers[0].verify), (6697, Verify::AcceptInvalid));
    }
}
