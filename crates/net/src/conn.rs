//! Tâches des connexions : établissement (client), acceptation (serveur), lecture et écriture des lignes.
//!
//! Chaque connexion est une seule tâche tokio, qui émet tous ses événements : leur ordre est donc garanti.

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpSocket, TcpStream};
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_rustls::TlsAcceptor;
use tokio_rustls::rustls::pki_types::ServerName;

use crate::{CLOSE_FLUSH, CONNECTED, Cmd, ConnHandles, Inner, MAX_LINE, NetEvent, Target, TlsInfo, Verify, lock};

/// Délai laissé à un client pour terminer la négociation TLS avec un serveur d'écoute.
const ACCEPT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
/// Premier sondage keepalive après 60 s de silence.
const KEEPALIVE: Duration = Duration::from_secs(60);
/// Taille maximale d'un lot d'écriture.
const WRITE_BATCH: usize = 64 * 1024;

/// Échec d'une tentative de connexion.
struct Failure {
    error: String,
    cert_sha256: Option<String>,
}

impl Failure {
    fn new(error: impl Into<String>) -> Failure {
        Failure { error: error.into(), cert_sha256: None }
    }
}

enum Stream {
    Plain(TcpStream),
    Tls(Box<tokio_rustls::client::TlsStream<TcpStream>>),
}

/// Attend la demande de fermeture (ou la disparition de l'entrée, qui vaut demande).
pub(crate) async fn wait_closing(rx: &mut watch::Receiver<bool>) {
    loop {
        if *rx.borrow_and_update() {
            return;
        }
        if rx.changed().await.is_err() {
            return;
        }
    }
}

/// Tâche d'une connexion sortante.
pub(crate) async fn client_task(
    inner: Arc<Inner>,
    mut h: ConnHandles,
    target: Target,
    previous: Option<JoinHandle<()>>,
) {
    // L'ancienne connexion de même identifiant émet son événement final avant que celle-ci commence.
    if let Some(prev) = previous {
        let _ = prev.await;
    }
    let conn = h.conn;
    let outcome = tokio::select! {
        r = establish(&target) => r,
        _ = wait_closing(&mut h.closing) => {
            log::info!("connexion {conn} : tentative interrompue");
            inner.finish(&h, Some(NetEvent::Closed { conn, error: None }));
            return;
        }
    };
    let (stream, peer, local, tls) = match outcome {
        Ok(v) => v,
        Err(f) => {
            log::info!("connexion {conn} : échec : {}", f.error);
            inner.finish(&h, Some(NetEvent::Failed { conn, error: f.error, cert_sha256: f.cert_sha256 }));
            return;
        }
    };
    log::info!(
        "connexion {conn} établie : {local} → {peer}{}",
        tls.as_ref().map(|t| format!(" ({}, {})", t.protocol, t.cipher)).unwrap_or_default()
    );
    h.state.store(CONNECTED, Ordering::Release);
    inner.emit(NetEvent::Connected { conn, peer, local, tls });
    let error = match stream {
        Stream::Plain(s) => run_io(&inner, conn, s, &mut h.cmds, &mut h.closing, &h.pending).await,
        Stream::Tls(s) => run_io(&inner, conn, *s, &mut h.cmds, &mut h.closing, &h.pending).await,
    };
    log::info!("connexion {conn} fermée{}", error.as_ref().map(|e| format!(" : {e}")).unwrap_or_default());
    inner.finish(&h, Some(NetEvent::Closed { conn, error }));
}

/// Résolution, connexion TCP, négociation TLS, dans le délai global de la cible.
async fn establish(t: &Target) -> Result<(Stream, SocketAddr, SocketAddr, Option<TlsInfo>), Failure> {
    let host = t.host.trim().trim_start_matches('[').trim_end_matches(']').to_string();
    let deadline = Instant::now() + t.connect_timeout;

    // Configuration TLS d'abord : une empreinte mal saisie ou un certificat client illisible échouent tout
    // de suite. Le premier chargement des racines du système lit des fichiers : hors des fils asynchrones.
    let tls = if t.tls {
        let seen = Arc::new(Mutex::new(None));
        let (verify, client_cert, seen2) = (t.verify.clone(), t.client_cert.clone(), seen.clone());
        let cfg = tokio::task::spawn_blocking(move || {
            let cc = client_cert.as_ref().map(|(c, k)| (c.as_slice(), k.as_slice()));
            crate::tls::build_client_config(&verify, cc, Some(seen2))
        })
        .await
        .map_err(|e| Failure::new(format!("TLS : {e}")))?
        .map_err(Failure::new)?;
        Some((cfg, seen, server_name(&host)?))
    } else {
        None
    };

    let tcp = match tokio::time::timeout_at(deadline, tcp_connect(&host, t.port, t.bind, deadline)).await {
        Ok(r) => r?,
        Err(_) => return Err(Failure::new(format!("délai de connexion dépassé ({})", secs(t.connect_timeout)))),
    };
    configure_socket(&tcp);
    let peer = tcp.peer_addr().map_err(|e| Failure::new(describe_io(&e)))?;
    let local = tcp.local_addr().map_err(|e| Failure::new(describe_io(&e)))?;

    let Some((cfg, seen, name)) = tls else {
        return Ok((Stream::Plain(tcp), peer, local, None));
    };
    let connector = tokio_rustls::TlsConnector::from(cfg);
    match tokio::time::timeout_at(deadline, connector.connect(name, tcp)).await {
        Err(_) => Err(Failure::new(format!("délai dépassé pendant la négociation TLS ({})", secs(t.connect_timeout)))),
        Ok(Err(e)) => {
            let rustls_error = e.get_ref().and_then(|i| i.downcast_ref::<rustls::Error>());
            let cert_sha256 = match rustls_error {
                Some(rustls::Error::InvalidCertificate(_)) => lock(&seen).clone(),
                _ => None,
            };
            let error = match rustls_error {
                Some(r) => crate::tls::describe_tls_error(r, &host),
                None => format!("négociation TLS : {}", describe_io(&e)),
            };
            Err(Failure { error, cert_sha256 })
        }
        Ok(Ok(stream)) => {
            let info = crate::tls::tls_info(stream.get_ref().1, t.verify == Verify::Full);
            Ok((Stream::Tls(Box::new(stream)), peer, local, Some(info)))
        }
    }
}

/// Nom présenté en SNI et vérifié : nom DNS, ou adresse IP (pas de SNI dans ce cas).
fn server_name(host: &str) -> Result<ServerName<'static>, Failure> {
    match host.parse::<IpAddr>() {
        Ok(ip) => Ok(ServerName::IpAddress(ip.into())),
        Err(_) => ServerName::try_from(host.to_string())
            .map_err(|_| Failure::new(format!("nom d'hôte invalide pour TLS : « {host} »"))),
    }
}

/// Résout puis essaie chaque adresse dans l'ordre (IPv6 et IPv4), avec un délai par adresse.
async fn tcp_connect(host: &str, port: u16, bind: Option<IpAddr>, deadline: Instant) -> Result<TcpStream, Failure> {
    let mut addrs: Vec<SocketAddr> = match host.parse::<IpAddr>() {
        Ok(ip) => vec![SocketAddr::new(ip, port)],
        Err(_) => tokio::net::lookup_host((host, port))
            .await
            .map_err(|e| Failure::new(format!("adresse introuvable pour « {host} » ({e})")))?
            .collect(),
    };
    if let Some(b) = bind {
        addrs.retain(|a| a.is_ipv4() == b.is_ipv4());
    }
    if addrs.is_empty() {
        return Err(Failure::new(match bind {
            Some(b) => format!("aucune adresse de « {host} » de la même famille que l'adresse locale {b}"),
            None => format!("aucune adresse pour « {host} »"),
        }));
    }
    let n = addrs.len();
    let mut last = String::new();
    for (i, addr) in addrs.iter().enumerate() {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let left = (n - i) as u32;
        let per = if left == 1 {
            remaining
        } else {
            (remaining / left).clamp(Duration::from_secs(3), Duration::from_secs(10)).min(remaining)
        };
        let attempt = async {
            let sock = if addr.is_ipv4() { TcpSocket::new_v4()? } else { TcpSocket::new_v6()? };
            if let Some(b) = bind {
                sock.bind(SocketAddr::new(b, 0))?;
            }
            sock.connect(*addr).await
        };
        match tokio::time::timeout(per, attempt).await {
            Ok(Ok(s)) => return Ok(s),
            Ok(Err(e)) => {
                log::debug!("{addr} : {e}");
                last = format!("{addr} : {}", describe_io(&e));
            }
            Err(_) => last = format!("{addr} : délai dépassé"),
        }
    }
    if last.is_empty() {
        last = "délai dépassé".into();
    }
    Err(Failure::new(format!("connexion impossible à {host}:{port} ({last})")))
}

/// TCP_NODELAY et keepalive (premier sondage après 60 s de silence).
fn configure_socket(tcp: &TcpStream) {
    if let Err(e) = tcp.set_nodelay(true) {
        log::debug!("TCP_NODELAY : {e}");
    }
    let ka = socket2::TcpKeepalive::new().with_time(KEEPALIVE);
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "windows"
    ))]
    let ka = ka.with_interval(Duration::from_secs(15));
    if let Err(e) = socket2::SockRef::from(tcp).set_tcp_keepalive(&ka) {
        log::debug!("keepalive : {e}");
    }
}

/// Socket d'écoute (SO_REUSEADDR sous Unix, pour relancer le relais sans attendre).
pub(crate) fn bind_listener(addr: SocketAddr) -> io::Result<std::net::TcpListener> {
    let sock =
        socket2::Socket::new(socket2::Domain::for_address(addr), socket2::Type::STREAM, Some(socket2::Protocol::TCP))?;
    #[cfg(unix)]
    sock.set_reuse_address(true)?;
    sock.bind(&addr.into())?;
    sock.listen(1024)?;
    sock.set_nonblocking(true)?;
    Ok(sock.into())
}

/// Boucle d'acceptation d'un serveur d'écoute.
pub(crate) async fn accept_loop(
    inner: Arc<Inner>,
    listener: u32,
    generation: u64,
    tcp: TcpListener,
    acceptor: Option<TlsAcceptor>,
    mut stop: watch::Receiver<bool>,
    done: std::sync::mpsc::SyncSender<()>,
) {
    let mut errors = 0u32;
    let error = loop {
        tokio::select! {
            _ = wait_closing(&mut stop) => break None,
            r = tcp.accept() => match r {
                Ok((sock, peer)) => {
                    errors = 0;
                    let Some(h) = inner.register_accepted() else { continue };
                    let (conn, conn_generation) = (h.conn, h.generation);
                    log::debug!("écoute {listener} : connexion {conn} depuis {peer}");
                    let task = tokio::spawn(server_task(inner.clone(), listener, h, sock, peer, acceptor.clone()));
                    inner.set_task(conn, conn_generation, task);
                }
                Err(e) => {
                    // Erreurs passagères (trop de fichiers ouverts, connexion avortée) : on réessaie.
                    errors += 1;
                    log::warn!("écoute {listener} : {e}");
                    if errors >= 50 {
                        break Some(describe_io(&e));
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    };
    drop(tcp);
    inner.remove_listener_if(listener, generation);
    log::info!("écoute {listener} arrêtée{}", error.as_ref().map(|e| format!(" : {e}")).unwrap_or_default());
    inner.emit(NetEvent::ListenerClosed { listener, error });
    let _ = done.try_send(());
}

/// Tâche d'une connexion acceptée. Un client qui échoue la négociation TLS ne produit aucun événement.
async fn server_task(
    inner: Arc<Inner>,
    listener: u32,
    mut h: ConnHandles,
    sock: TcpStream,
    peer: SocketAddr,
    acceptor: Option<TlsAcceptor>,
) {
    let conn = h.conn;
    configure_socket(&sock);
    let error = match acceptor {
        None => {
            h.state.store(CONNECTED, Ordering::Release);
            inner.emit(NetEvent::Accepted { listener, conn, peer, tls: false });
            run_io(&inner, conn, sock, &mut h.cmds, &mut h.closing, &h.pending).await
        }
        Some(acceptor) => {
            let handshake = tokio::select! {
                r = tokio::time::timeout(ACCEPT_HANDSHAKE_TIMEOUT, acceptor.accept(sock)) => r,
                _ = wait_closing(&mut h.closing) => {
                    inner.finish(&h, None);
                    return;
                }
            };
            let stream = match handshake {
                Ok(Ok(s)) => s,
                Ok(Err(e)) => {
                    log::info!("écoute {listener} : négociation TLS échouée avec {peer} : {e}");
                    inner.finish(&h, None);
                    return;
                }
                Err(_) => {
                    log::info!("écoute {listener} : négociation TLS trop longue avec {peer}");
                    inner.finish(&h, None);
                    return;
                }
            };
            h.state.store(CONNECTED, Ordering::Release);
            inner.emit(NetEvent::Accepted { listener, conn, peer, tls: true });
            run_io(&inner, conn, stream, &mut h.cmds, &mut h.closing, &h.pending).await
        }
    };
    log::debug!("connexion {conn} fermée{}", error.as_ref().map(|e| format!(" : {e}")).unwrap_or_default());
    inner.finish(&h, Some(NetEvent::Closed { conn, error }));
}

/// Échanges d'une connexion établie, jusqu'à sa fin ; rend l'erreur éventuelle.
///
/// La lecture et l'écriture avancent en parallèle. À la demande de fermeture, l'écrivain vide la file puis
/// ferme (close_notify, FIN) ; on continue de lire (la réponse du serveur, `ERROR :Closing link`, est
/// livrée) jusqu'à la fin du flux ou au bout de [`CLOSE_FLUSH`].
async fn run_io<S>(
    inner: &Inner,
    conn: u32,
    stream: S,
    cmds: &mut mpsc::UnboundedReceiver<Cmd>,
    closing: &mut watch::Receiver<bool>,
    pending: &AtomicUsize,
) -> Option<String>
where
    S: AsyncRead + AsyncWrite + Send + Unpin,
{
    let (rd, wr) = tokio::io::split(stream);
    let reader = read_lines(inner, conn, rd);
    let writer = write_lines(wr, cmds, pending);
    tokio::pin!(reader, writer);
    let timer = tokio::time::sleep(Duration::from_secs(365 * 86_400));
    tokio::pin!(timer);
    let mut closing_seen = false;
    let mut writer_done = false;
    loop {
        tokio::select! {
            r = &mut reader => {
                return match r {
                    Ok(()) => None,
                    Err(_) if closing_seen => None,
                    Err(e) => Some(describe_io(&e)),
                };
            }
            w = &mut writer, if !writer_done => match w {
                Ok(()) => {
                    writer_done = true;
                    if !closing_seen {
                        closing_seen = true;
                        timer.as_mut().reset(Instant::now() + CLOSE_FLUSH);
                    }
                }
                Err(_) if closing_seen => return None,
                Err(e) => return Some(describe_io(&e)),
            },
            _ = wait_closing(closing), if !closing_seen => {
                closing_seen = true;
                timer.as_mut().reset(Instant::now() + CLOSE_FLUSH);
            }
            _ = &mut timer, if closing_seen => {
                log::debug!("connexion {conn} : fermeture forcée au bout de {CLOSE_FLUSH:?}");
                return None;
            }
        }
    }
}

/// Lit et découpe les lignes (LF, CR final retiré, lignes vides ignorées, [`MAX_LINE`] octets au plus).
/// Une fin de flux TLS sans close_notify compte comme une fin normale (beaucoup de serveurs IRC le font).
async fn read_lines<R: AsyncRead + Unpin>(inner: &Inner, conn: u32, mut rd: R) -> io::Result<()> {
    let mut buf = vec![0u8; 16 * 1024];
    let mut line = Vec::with_capacity(512);
    loop {
        let n = match rd.read(&mut buf).await {
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => 0,
            Err(e) => return Err(e),
        };
        if n == 0 {
            emit_line(inner, conn, &mut line);
            return Ok(());
        }
        let mut chunk = &buf[..n];
        while let Some(pos) = chunk.iter().position(|&b| b == b'\n') {
            push_limited(&mut line, &chunk[..pos]);
            emit_line(inner, conn, &mut line);
            chunk = &chunk[pos + 1..];
        }
        push_limited(&mut line, chunk);
    }
}

/// Ajoute à la ligne en cours sans dépasser `MAX_LINE + 1` octets (place pour un CR final).
fn push_limited(line: &mut Vec<u8>, data: &[u8]) {
    let room = (MAX_LINE + 1).saturating_sub(line.len());
    line.extend_from_slice(&data[..data.len().min(room)]);
}

fn emit_line(inner: &Inner, conn: u32, line: &mut Vec<u8>) {
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    if line.len() > MAX_LINE {
        log::debug!("connexion {conn} : ligne tronquée à {MAX_LINE} octets");
        line.truncate(MAX_LINE);
    }
    if line.is_empty() {
        return;
    }
    let bytes = std::mem::replace(line, Vec::with_capacity(512));
    inner.emit(NetEvent::Line { conn, bytes });
}

/// Écrit les lignes de la file (par lots), jusqu'à l'ordre de fermeture : alors vide, termine TLS et TCP. Chaque
/// lot écrit est retiré du compteur `pending` (voir [`crate::Net::pending_bytes`]).
async fn write_lines<W: AsyncWrite + Unpin>(
    mut wr: W,
    cmds: &mut mpsc::UnboundedReceiver<Cmd>,
    pending: &AtomicUsize,
) -> io::Result<()> {
    let mut out = Vec::with_capacity(4096);
    loop {
        let mut close = match cmds.recv().await {
            Some(Cmd::Line(l)) => {
                push_crlf(&mut out, &l);
                false
            }
            Some(Cmd::Close) | None => true,
        };
        while !close && out.len() < WRITE_BATCH {
            match cmds.try_recv() {
                Ok(Cmd::Line(l)) => push_crlf(&mut out, &l),
                Ok(Cmd::Close) | Err(TryRecvError::Disconnected) => close = true,
                Err(TryRecvError::Empty) => break,
            }
        }
        if !out.is_empty() {
            wr.write_all(&out).await?;
            wr.flush().await?;
            let n = out.len();
            let _ = pending.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |p| Some(p.saturating_sub(n)));
            out.clear();
        }
        if close {
            wr.shutdown().await?;
            return Ok(());
        }
    }
}

fn push_crlf(out: &mut Vec<u8>, line: &[u8]) {
    out.extend_from_slice(line);
    out.extend_from_slice(b"\r\n");
}

/// Message clair pour une erreur d'entrée-sortie.
pub(crate) fn describe_io(e: &io::Error) -> String {
    use io::ErrorKind::*;
    match e.kind() {
        ConnectionRefused => "connexion refusée".into(),
        ConnectionReset => "connexion réinitialisée par le pair".into(),
        ConnectionAborted => "connexion interrompue".into(),
        BrokenPipe => "connexion rompue".into(),
        TimedOut => "délai dépassé".into(),
        HostUnreachable => "hôte injoignable".into(),
        NetworkUnreachable => "réseau injoignable".into(),
        AddrNotAvailable => "adresse locale indisponible".into(),
        AddrInUse => "adresse déjà utilisée".into(),
        _ => e.to_string(),
    }
}

/// Durée lisible : « 30 s », « 0,5 s ».
fn secs(d: Duration) -> String {
    format!("{} s", d.as_secs_f64()).replace('.', ",")
}
