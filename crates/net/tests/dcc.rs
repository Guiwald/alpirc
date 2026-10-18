//! Essais des connexions DCC, en local (127.0.0.1) : envoi d'un fichier avec accusés de réception, reprise,
//! DCC passif (le destinataire écoute), annulation, discussion dans les deux sens, délais, pairs « à la main ».

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener as StdListener, TcpStream as StdStream};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use alpirc_net::{DccJob, IoKind, Net, NetError, NetEvent};

const WAIT: Duration = Duration::from_secs(20);
const LOCAL: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

struct Probe {
    net: Net,
    rx: mpsc::Receiver<NetEvent>,
}

impl Probe {
    fn new() -> Probe {
        let (tx, rx) = mpsc::channel();
        let net = Net::new(Box::new(move |ev| {
            let _ = tx.send(ev);
        }));
        Probe { net, rx }
    }

    /// Attend la fin de `id` ; rend (position finale, erreur, nombre d'événements d'avancement, connecté ?).
    fn wait_end(&self, id: u32) -> (u64, Option<NetError>, usize, bool) {
        let deadline = Instant::now() + WAIT;
        let (mut progress, mut connected) = (0, false);
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left).expect("fin DCC attendue") {
                NetEvent::DccEnded { id: i, bytes, error } if i == id => return (bytes, error, progress, connected),
                NetEvent::DccProgress { id: i, .. } if i == id => progress += 1,
                NetEvent::DccConnected { id: i, .. } if i == id => connected = true,
                _ => {}
            }
        }
    }

    fn wait_connected(&self, id: u32) {
        let deadline = Instant::now() + WAIT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left).expect("connexion DCC attendue") {
                NetEvent::DccConnected { id: i, .. } if i == id => return,
                NetEvent::DccEnded { id: i, error, .. } if i == id => panic!("fin inattendue : {error:?}"),
                _ => {}
            }
        }
    }

    fn wait_line(&self, id: u32) -> Vec<u8> {
        let deadline = Instant::now() + WAIT;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(left).expect("ligne DCC attendue") {
                NetEvent::DccLine { id: i, bytes } if i == id => return bytes,
                NetEvent::DccEnded { id: i, error, .. } if i == id => panic!("fin inattendue : {error:?}"),
                _ => {}
            }
        }
    }
}

fn tmpdir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("alpirc-net-dcc-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Contenu reproductible de `n` octets.
fn data(n: usize) -> Vec<u8> {
    let mut x: u32 = 0x1234_5678;
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            (x >> 24) as u8
        })
        .collect()
}

const T: Duration = Duration::from_secs(10);

#[test]
fn send_5mb_with_acks() {
    let dir = tmpdir("send");
    let content = data(5 * 1024 * 1024);
    let src = dir.join("source.bin");
    std::fs::write(&src, &content).unwrap();
    let dst = dir.join("reçu.bin");
    let (a, b) = (Probe::new(), Probe::new());
    let size = content.len() as u64;
    // L'envoyeur écoute (DCC actif), le destinataire se connecte.
    let addr = a.net.dcc_listen(1, LOCAL, (0, 0), DccJob::Send { path: src, offset: 0, size, packet: 16 * 1024 }, T).unwrap();
    b.net.dcc_connect(2, addr, DccJob::Receive { path: dst.clone(), offset: 0, size: Some(size) }, T);
    let (sb, eb, _, cb) = b.wait_end(2);
    let (sa, ea, _, ca) = a.wait_end(1);
    assert!(ca && cb);
    assert_eq!((ea, eb), (None, None));
    assert_eq!((sa, sb), (size, size));
    assert_eq!(std::fs::read(&dst).unwrap(), content);
}

#[test]
fn passive_receiver_listens() {
    let dir = tmpdir("passive");
    let content = data(1_234_567);
    let src = dir.join("a b.bin");
    std::fs::write(&src, &content).unwrap();
    let dst = dir.join("dst.bin");
    let (a, b) = (Probe::new(), Probe::new());
    let size = content.len() as u64;
    // DCC passif : le destinataire écoute, l'envoyeur se connecte et envoie.
    let addr = b.net.dcc_listen(7, LOCAL, (0, 0), DccJob::Receive { path: dst.clone(), offset: 0, size: Some(size) }, T).unwrap();
    a.net.dcc_connect(8, addr, DccJob::Send { path: src, offset: 0, size, packet: 4096 }, T);
    assert_eq!(a.wait_end(8).1, None);
    assert_eq!(b.wait_end(7).1, None);
    assert_eq!(std::fs::read(&dst).unwrap(), content);
}

#[test]
fn resume_from_the_middle() {
    let dir = tmpdir("resume");
    let content = data(3 * 1024 * 1024 + 17);
    let src = dir.join("src.bin");
    std::fs::write(&src, &content).unwrap();
    let dst = dir.join("partial.bin");
    let half = content.len() / 2;
    // Fichier partiel, avec des octets en trop après la position de reprise (coupés).
    let mut partial = content[..half].to_vec();
    partial.extend_from_slice(b"garbage");
    std::fs::write(&dst, &partial).unwrap();
    let (a, b) = (Probe::new(), Probe::new());
    let size = content.len() as u64;
    let addr = a.net.dcc_listen(1, LOCAL, (0, 0), DccJob::Send { path: src, offset: 0, size, packet: 8192 }, T).unwrap();
    // RESUME accepté avant la connexion du destinataire.
    a.net.dcc_resume(1, half as u64);
    b.net.dcc_connect(2, addr, DccJob::Receive { path: dst.clone(), offset: half as u64, size: Some(size) }, T);
    let (sb, eb, _, _) = b.wait_end(2);
    let (sa, ea, _, _) = a.wait_end(1);
    assert_eq!((ea, eb), (None, None));
    assert_eq!((sa, sb), (size, size));
    assert_eq!(std::fs::read(&dst).unwrap(), content);
}

#[test]
fn cancel_sender_and_receiver() {
    let dir = tmpdir("cancel");
    let content = data(40 * 1024 * 1024);
    let src = dir.join("big.bin");
    std::fs::write(&src, &content).unwrap();
    let a = Probe::new();
    // Un destinataire qui ne lit jamais : l'envoyeur se bloque, puis est annulé.
    let addr =
        a.net.dcc_listen(1, LOCAL, (0, 0), DccJob::Send { path: src, offset: 0, size: content.len() as u64, packet: 65536 }, T).unwrap();
    let _peer = StdStream::connect(addr).unwrap();
    a.wait_connected(1);
    std::thread::sleep(Duration::from_millis(200));
    let t0 = Instant::now();
    a.net.dcc_close(1);
    let (_, e, _, _) = a.wait_end(1);
    assert_eq!(e.as_ref(), Some(&NetError::Cancelled));
    assert!(t0.elapsed() < Duration::from_secs(3));

    // Un envoyeur muet : le destinataire attend, puis est annulé.
    let srv = StdListener::bind((LOCAL, 0)).unwrap();
    let addr = srv.local_addr().unwrap();
    let b = Probe::new();
    b.net.dcc_connect(2, addr, DccJob::Receive { path: dir.join("never.bin"), offset: 0, size: Some(100) }, T);
    let (_s, _) = srv.accept().unwrap();
    b.wait_connected(2);
    b.net.dcc_close(2);
    let (pos, e, _, _) = b.wait_end(2);
    assert_eq!((pos, e.as_ref()), (0, Some(&NetError::Cancelled)));
}

#[test]
fn chat_lines_both_ways() {
    let (a, b) = (Probe::new(), Probe::new());
    let addr = a.net.dcc_listen(1, LOCAL, (0, 0), DccJob::Chat, T).unwrap();
    b.net.dcc_connect(2, addr, DccJob::Chat, T);
    a.wait_connected(1);
    b.wait_connected(2);
    a.net.dcc_send_line(1, "bonjour à toi".as_bytes().to_vec());
    assert_eq!(b.wait_line(2), "bonjour à toi".as_bytes());
    b.net.dcc_send_line(2, b"salut\r\nPRIVMSG injection".to_vec());
    assert_eq!(a.wait_line(1), b"salut");
    b.net.dcc_send_line(2, b"\x01ACTION waves\x01".to_vec());
    assert_eq!(a.wait_line(1), b"\x01ACTION waves\x01");
    // Une ligne demandée juste avant la fermeture part quand même.
    a.net.dcc_send_line(1, b"bye".to_vec());
    a.net.dcc_close(1);
    assert_eq!(b.wait_line(2), b"bye");
    assert_eq!(b.wait_end(2).1, None);
    assert_eq!(a.wait_end(1).1, None);
}

#[test]
fn listen_timeout_and_refused_connect() {
    let a = Probe::new();
    let t0 = Instant::now();
    a.net.dcc_listen(1, LOCAL, (0, 0), DccJob::Chat, Duration::from_millis(300)).unwrap();
    let (_, e, _, c) = a.wait_end(1);
    assert!(!c);
    assert_eq!(e, Some(NetError::Timeout { ms: 300 }));
    assert!(t0.elapsed() < Duration::from_secs(3));
    // Port fermé : refus.
    let port = StdListener::bind((LOCAL, 0)).unwrap().local_addr().unwrap().port();
    a.net.dcc_connect(2, SocketAddr::new(LOCAL, port), DccJob::Chat, T);
    let (_, e, _, _) = a.wait_end(2);
    assert_eq!(e, Some(NetError::Io(IoKind::Refused)));
}

#[test]
fn port_range() {
    let a = Probe::new();
    let free = StdListener::bind((LOCAL, 0)).unwrap();
    let busy = free.local_addr().unwrap().port();
    // Le premier port est pris : le suivant de la plage sert.
    let addr = a.net.dcc_listen(1, LOCAL, (busy, busy.saturating_add(20)), DccJob::Chat, T).unwrap();
    assert!(addr.port() > busy && addr.port() <= busy + 20);
    a.net.dcc_close(1);
    assert_eq!(a.wait_end(1).1.as_ref(), Some(&NetError::Cancelled));
}

/// Destinataire « à la main » : lit tout et vérifie les accusés (4 octets, ordre réseau, position absolue).
#[test]
fn acks_are_32bit_big_endian_positions() {
    let dir = tmpdir("acks");
    let content = data(300_000);
    let srv = StdListener::bind((LOCAL, 0)).unwrap();
    let addr = srv.local_addr().unwrap();
    let b = Probe::new();
    let dst = dir.join("x.bin");
    std::fs::write(&dst, &content[..1000]).unwrap();
    b.net.dcc_connect(2, addr, DccJob::Receive { path: dst.clone(), offset: 1000, size: Some(content.len() as u64) }, T);
    let (mut s, _) = srv.accept().unwrap();
    // Envoyeur à l'ancienne : un bloc, puis attente de son accusé.
    let mut pos = 1000usize;
    let mut last_ack = 0u32;
    while pos < content.len() {
        let end = (pos + 50_000).min(content.len());
        s.write_all(&content[pos..end]).unwrap();
        pos = end;
        loop {
            let mut ack = [0u8; 4];
            s.read_exact(&mut ack).unwrap();
            last_ack = u32::from_be_bytes(ack);
            if last_ack as usize >= pos {
                break;
            }
        }
    }
    assert_eq!(last_ack as usize, content.len());
    let (bytes, e, _, _) = b.wait_end(2);
    assert_eq!((bytes, e), (content.len() as u64, None));
    assert_eq!(std::fs::read(&dst).unwrap(), content);
}

/// Envoyeur qui s'arrête au milieu : le destinataire le signale.
#[test]
fn interrupted_transfer_fails() {
    let dir = tmpdir("interrupted");
    let srv = StdListener::bind((LOCAL, 0)).unwrap();
    let addr = srv.local_addr().unwrap();
    let b = Probe::new();
    b.net.dcc_connect(2, addr, DccJob::Receive { path: dir.join("y.bin"), offset: 0, size: Some(10_000) }, T);
    let (mut s, _) = srv.accept().unwrap();
    s.write_all(&[7u8; 4000]).unwrap();
    // Fin propre (FIN) : on ferme l'écriture et on lit les accusés jusqu'à la fermeture par le destinataire
    // (fermer avec des accusés non lus enverrait un RST, autre erreur).
    s.shutdown(std::net::Shutdown::Write).unwrap();
    let mut acks = Vec::new();
    let _ = s.read_to_end(&mut acks);
    drop(s);
    let (bytes, e, _, _) = b.wait_end(2);
    assert_eq!(bytes, 4000);
    assert_eq!(e, Some(NetError::PeerClosed { pos: 4000, size: 10_000 }));
}

/// Destinataire qui n'accuse rien et ferme après avoir tout lu : l'envoi est réussi.
#[test]
fn receiver_without_acks() {
    let dir = tmpdir("noacks");
    let content = data(200_000);
    let src = dir.join("s.bin");
    std::fs::write(&src, &content).unwrap();
    let a = Probe::new();
    let addr =
        a.net.dcc_listen(1, LOCAL, (0, 0), DccJob::Send { path: src, offset: 0, size: content.len() as u64, packet: 4096 }, T).unwrap();
    let mut s = StdStream::connect(addr).unwrap();
    let mut got = Vec::new();
    s.read_to_end(&mut got).unwrap();
    assert_eq!(got, content);
    drop(s);
    let (bytes, e, _, _) = a.wait_end(1);
    assert_eq!((bytes, e), (content.len() as u64, None));
}
