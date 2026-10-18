//! Contre-pression : [`Net::pending_bytes`] compte ce qui attend d'être écrit, grandit quand le pair ne lit plus
//! et revient à zéro quand il a tout lu.

use std::io::Read;
use std::net::TcpStream;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use alpirc_net::{Net, NetEvent};

#[test]
fn pending_bytes_track_a_slow_peer() {
    let (tx, rx) = mpsc::channel();
    let net = Net::new(Box::new(move |ev| {
        let _ = tx.send(ev);
    }));
    let addr = net.listen(1, "127.0.0.1:0".parse().unwrap(), None).unwrap();
    let mut peer = TcpStream::connect(addr).unwrap();
    let conn = loop {
        match rx.recv_timeout(Duration::from_secs(10)).expect("Accepted attendu") {
            NetEvent::Accepted { conn, .. } => break conn,
            _ => continue,
        }
    };
    assert_eq!(net.pending_bytes(conn), 0);
    assert_eq!(net.pending_bytes(424_242), 0, "connexion inconnue");

    // 32 Mio en lignes de 1 Kio (CRLF compris) ; le pair ne lit pas : bien plus que les tampons du système.
    const LINES: usize = 32 * 1024;
    let line = vec![b'x'; 1022];
    for _ in 0..LINES {
        net.send(conn, line.clone());
    }
    std::thread::sleep(Duration::from_millis(500));
    let stuck = net.pending_bytes(conn);
    assert!(stuck > 8 * 1024 * 1024, "le pair ne lit pas : {stuck} octets devraient attendre");
    assert!(stuck <= LINES * 1024);

    // Le pair lit tout : la file se vide.
    let total = LINES * 1024;
    let reader = std::thread::spawn(move || {
        let mut buf = vec![0u8; 256 * 1024];
        let mut got = 0;
        while got < total {
            match peer.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => got += n,
            }
        }
        got
    });
    assert_eq!(reader.join().unwrap(), total);
    let deadline = Instant::now() + Duration::from_secs(10);
    while net.pending_bytes(conn) != 0 {
        assert!(Instant::now() < deadline, "file non vidée : {}", net.pending_bytes(conn));
        std::thread::sleep(Duration::from_millis(20));
    }
    net.close(conn);
    net.shutdown();
}
