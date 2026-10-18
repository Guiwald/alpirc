//! DCC : connexions directes entre clients (hors serveur IRC), en flux d'octets.
//!
//! Une connexion DCC naît d'une écoute ([`Net::dcc_listen`] : le premier pair qui se connecte, dans le délai)
//! ou d'une connexion sortante ([`Net::dcc_connect`]), puis porte une seule tâche ([`DccJob`]) :
//! - **discussion** (DCC CHAT) : des lignes dans les deux sens ([`NetEvent::DccLine`], [`Net::dcc_send_line`]) ;
//! - **envoi** (DCC SEND) : le fichier à partir d'une position, envoyé d'avance (sans attendre chaque
//!   accusé de réception), en lisant les accusés du destinataire (positions sur 4 octets, ou 8 au-delà de
//!   4 Gio, ordre réseau) ; fini quand le dernier accusé arrive, ou quand le destinataire ferme après avoir tout
//!   reçu ;
//! - **réception** (DCC GET) : écrit le fichier à partir d'une position et accuse réception après chaque bloc ;
//!   fini quand la taille annoncée est atteinte (ou à la fermeture, taille inconnue).
//!
//! Événements : [`NetEvent::DccConnected`], [`NetEvent::DccProgress`] (5 par seconde au plus), puis toujours
//! exactement un [`NetEvent::DccEnded`] par identifiant (succès, échec, délai, annulation par
//! [`Net::dcc_close`]). Les identifiants DCC sont distincts de ceux des connexions IRC.

use std::io::{self, SeekFrom};
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::conn::{millis, wait_closing};
use crate::{Inner, IoKind, MAX_LINE, Net, NetError, NetEvent, lock};

/// Intervalle minimal entre deux [`NetEvent::DccProgress`].
const PROGRESS_EVERY: Duration = Duration::from_millis(200);

/// Ce que porte une connexion DCC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DccJob {
    /// DCC CHAT : des lignes dans les deux sens.
    Chat,
    /// Envoyer `path` à partir de `offset` ; `size` : taille totale annoncée ; `packet` : taille des blocs.
    Send { path: PathBuf, offset: u64, size: u64, packet: usize },
    /// Recevoir dans `path` à partir de `offset` (le fichier est coupé là) ; `size` : taille annoncée (`None` :
    /// jusqu'à la fermeture).
    Receive { path: PathBuf, offset: u64, size: Option<u64> },
}

/// Une connexion DCC enregistrée.
pub(crate) struct DccEntry {
    generation: u64,
    cancel: watch::Sender<bool>,
    lines: mpsc::UnboundedSender<Vec<u8>>,
    /// Position de départ d'un envoi pas encore connecté (changée par [`Net::dcc_resume`]).
    offset: Arc<AtomicU64>,
    task: Option<JoinHandle<()>>,
}

/// Comment obtenir la connexion.
enum How {
    Listen(TcpListener),
    Connect(SocketAddr),
}

/// Annule toutes les connexions DCC (arrêt de la couche réseau) et rend leurs tâches.
pub(crate) fn cancel_all(inner: &Inner) -> Vec<JoinHandle<()>> {
    lock(&inner.dcc)
        .values_mut()
        .filter_map(|e| {
            e.cancel.send_replace(true);
            e.task.take()
        })
        .collect()
}

impl Net {
    /// Écoute sur `ip` (un port de `ports`, `(0, 0)` : choisi par le système) et rend l'adresse effective ;
    /// le premier pair qui se connecte dans `timeout` reçoit `job` (l'écoute s'arrête alors). Une connexion DCC
    /// de même identifiant est d'abord annulée.
    pub fn dcc_listen(&self, id: u32, ip: IpAddr, ports: (u16, u16), job: DccJob, timeout: Duration) -> Result<SocketAddr, NetError> {
        let inner = &self.inner;
        if inner.stopping.load(Ordering::Acquire) {
            return Err(NetError::Stopped);
        }
        let (lo, hi) = if ports.0 == 0 { (0, 0) } else { (ports.0, ports.1.max(ports.0)) };
        let mut last = None;
        let mut bound = None;
        for port in lo..=hi {
            match std::net::TcpListener::bind(SocketAddr::new(ip, port)) {
                Ok(l) => {
                    bound = Some(l);
                    break;
                }
                Err(e) => last = Some(e),
            }
        }
        let fail = |e: io::Error| NetError::DccListen { ip, cause: IoKind::of(&e) };
        let std_listener = match bound {
            Some(l) => l,
            None => return Err(fail(last.unwrap_or_else(|| io::Error::from(io::ErrorKind::AddrInUse)))),
        };
        std_listener.set_nonblocking(true).map_err(fail)?;
        let addr = std_listener.local_addr().map_err(fail)?;
        let tcp = {
            let _guard = inner.handle.enter();
            TcpListener::from_std(std_listener).map_err(fail)?
        };
        log::info!("DCC {id} : écoute sur {addr}");
        self.dcc_spawn(id, How::Listen(tcp), job, timeout);
        Ok(addr)
    }

    /// Se connecte à `addr` (dans `timeout`) puis porte `job`.
    pub fn dcc_connect(&self, id: u32, addr: SocketAddr, job: DccJob, timeout: Duration) {
        if self.inner.stopping.load(Ordering::Acquire) {
            log::warn!("DCC {id} refusé : couche réseau arrêtée");
            return;
        }
        log::info!("DCC {id} : connexion à {addr}");
        self.dcc_spawn(id, How::Connect(addr), job, timeout);
    }

    /// DCC CHAT : envoie une ligne (Net ajoute LF ; coupée au premier CR ou LF qu'elle contiendrait).
    pub fn dcc_send_line(&self, id: u32, mut line: Vec<u8>) {
        if let Some(cut) = line.iter().position(|&b| b == b'\r' || b == b'\n') {
            line.truncate(cut);
        }
        if let Some(e) = lock(&self.inner.dcc).get(&id) {
            let _ = e.lines.send(line);
        }
    }

    /// Envoi en écoute, pas encore connecté : partira de `offset` (DCC RESUME accepté).
    pub fn dcc_resume(&self, id: u32, offset: u64) {
        if let Some(e) = lock(&self.inner.dcc).get(&id) {
            e.offset.store(offset, Ordering::Release);
        }
    }

    /// Annule (transfert) ou ferme (discussion, après les lignes en attente) ; produit `DccEnded`.
    pub fn dcc_close(&self, id: u32) {
        if let Some(e) = lock(&self.inner.dcc).get(&id) {
            e.cancel.send_replace(true);
        }
    }

    fn dcc_spawn(&self, id: u32, how: How, job: DccJob, timeout: Duration) {
        let inner = &self.inner;
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let (lines_tx, lines_rx) = mpsc::unbounded_channel();
        let start = match &job {
            DccJob::Send { offset, .. } | DccJob::Receive { offset, .. } => *offset,
            DccJob::Chat => 0,
        };
        let offset = Arc::new(AtomicU64::new(start));
        let generation = inner.next_generation.fetch_add(1, Ordering::Relaxed);
        let mut map = lock(&inner.dcc);
        if let Some(old) = map.remove(&id) {
            old.cancel.send_replace(true);
        }
        let ctx = Task { inner: inner.clone(), id, generation, timeout, cancel: cancel_rx, lines: lines_rx, offset: offset.clone() };
        let task = inner.handle.spawn(run(ctx, how, job));
        map.insert(id, DccEntry { generation, cancel: cancel_tx, lines: lines_tx, offset, task: Some(task) });
    }
}

/// Ce que reçoit la tâche d'une connexion DCC.
struct Task {
    inner: Arc<Inner>,
    id: u32,
    generation: u64,
    timeout: Duration,
    cancel: watch::Receiver<bool>,
    lines: mpsc::UnboundedReceiver<Vec<u8>>,
    offset: Arc<AtomicU64>,
}

impl Task {
    fn emit(&self, ev: NetEvent) {
        self.inner.emit(ev);
    }

    /// Événement final, puis retrait du registre.
    fn end(&self, bytes: u64, error: Option<NetError>) {
        log::info!("DCC {} terminé ({bytes} octets){}", self.id, error.as_ref().map(|e| format!(" : {e}")).unwrap_or_default());
        self.emit(NetEvent::DccEnded { id: self.id, bytes, error });
        let mut map = lock(&self.inner.dcc);
        if map.get(&self.id).is_some_and(|e| e.generation == self.generation) {
            map.remove(&self.id);
        }
    }

    fn progress(&self, bytes: u64) {
        self.emit(NetEvent::DccProgress { id: self.id, bytes });
    }

    fn cancelled(&self) -> bool {
        *self.cancel.borrow()
    }
}

fn timed_out(d: Duration) -> NetError {
    NetError::Timeout { ms: millis(d) }
}

/// La tâche : connexion (écoute ou sortie), puis le travail.
async fn run(mut t: Task, how: How, job: DccJob) {
    let start = match &job {
        DccJob::Send { .. } | DccJob::Receive { .. } => t.offset.load(Ordering::Acquire),
        DccJob::Chat => 0,
    };
    let timeout = t.timeout;
    let established = async {
        match how {
            How::Listen(l) => {
                let (s, peer) = l.accept().await?;
                Ok::<(TcpStream, SocketAddr), io::Error>((s, peer))
            }
            How::Connect(addr) => {
                let s = TcpStream::connect(addr).await?;
                Ok((s, addr))
            }
        }
    };
    let r = tokio::select! {
        r = tokio::time::timeout(timeout, established) => r,
        _ = wait_closing(&mut t.cancel) => return t.end(start, Some(NetError::Cancelled)),
    };
    let (stream, peer) = match r {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => return t.end(start, Some(NetError::io(&e))),
        Err(_) => return t.end(start, Some(timed_out(timeout))),
    };
    let _ = stream.set_nodelay(true);
    log::info!("DCC {} : connecté à {peer}", t.id);
    t.emit(NetEvent::DccConnected { id: t.id, peer });
    let (bytes, error) = match job {
        DccJob::Chat => (0, run_chat(&mut t, stream).await),
        DccJob::Send { path, size, packet, .. } => {
            // La position a pu changer (RESUME) avant la connexion.
            let offset = t.offset.load(Ordering::Acquire).min(size);
            run_send(&mut t, stream, &path, offset, size, packet.clamp(512, 1 << 20)).await
        }
        DccJob::Receive { path, offset, size } => run_receive(&mut t, stream, &path, offset, size).await,
    };
    t.end(bytes, error);
}

// -------------------------------------------------------------------------------------------------------------
// Discussion

async fn run_chat(t: &mut Task, stream: TcpStream) -> Option<NetError> {
    let (rd, mut wr) = stream.into_split();
    let inner = t.inner.clone();
    let id = t.id;
    let reader = read_chat_lines(&inner, id, rd);
    tokio::pin!(reader);
    loop {
        tokio::select! {
            r = &mut reader => {
                return match r {
                    Ok(()) => None,
                    Err(e) => Some(NetError::io(&e)),
                };
            }
            l = t.lines.recv() => match l {
                Some(mut l) => {
                    l.push(b'\n');
                    if let Err(e) = wr.write_all(&l).await {
                        return Some(NetError::io(&e));
                    }
                }
                None => {
                    let _ = wr.shutdown().await;
                    return None;
                }
            },
            _ = wait_closing(&mut t.cancel) => {
                // Les lignes déjà demandées partent avant la fermeture.
                while let Ok(mut l) = t.lines.try_recv() {
                    l.push(b'\n');
                    let _ = wr.write_all(&l).await;
                }
                let _ = wr.shutdown().await;
                return None;
            }
        }
    }
}

/// Lit les lignes d'une discussion (LF, CR final retiré, lignes vides ignorées, [`MAX_LINE`] octets au plus).
async fn read_chat_lines(inner: &Inner, id: u32, mut rd: tokio::net::tcp::OwnedReadHalf) -> io::Result<()> {
    let mut buf = vec![0u8; 8192];
    let mut line: Vec<u8> = Vec::with_capacity(512);
    let emit = |line: &mut Vec<u8>| {
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        line.truncate(MAX_LINE);
        if !line.is_empty() {
            inner.emit(NetEvent::DccLine { id, bytes: std::mem::take(line) });
        }
        line.clear();
    };
    loop {
        let n = match rd.read(&mut buf).await {
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => 0,
            Err(e) => return Err(e),
        };
        if n == 0 {
            emit(&mut line);
            return Ok(());
        }
        let mut chunk = &buf[..n];
        while let Some(pos) = chunk.iter().position(|&b| b == b'\n') {
            if line.len() <= MAX_LINE {
                line.extend_from_slice(&chunk[..pos]);
            }
            emit(&mut line);
            chunk = &chunk[pos + 1..];
        }
        if line.len() <= MAX_LINE {
            line.extend_from_slice(chunk);
        }
    }
}

// -------------------------------------------------------------------------------------------------------------
// Envoi

/// Position 64 bits d'un accusé sur 32 bits (les positions ne font que croître).
fn unwrap32(prev: u64, v: u32) -> u64 {
    let mut c = (prev & !0xFFFF_FFFF) | v as u64;
    if c < prev && prev - c > 0x8000_0000 {
        c += 1 << 32;
    }
    c
}

async fn run_send(t: &mut Task, stream: TcpStream, path: &Path, offset: u64, size: u64, packet: usize) -> (u64, Option<NetError>) {
    use std::sync::atomic::Ordering::Relaxed;
    let rerr = |e: io::Error| NetError::FileRead { path: path.to_path_buf(), cause: IoKind::of(&e) };
    let mut file = match tokio::fs::File::open(path).await {
        Ok(f) => f,
        Err(e) => return (offset, Some(rerr(e))),
    };
    if offset > 0
        && let Err(e) = file.seek(SeekFrom::Start(offset)).await
    {
        return (offset, Some(rerr(e)));
    }
    let (mut rd, mut wr) = stream.into_split();
    let wide = size > u32::MAX as u64;
    // Partagés par l'écrivain et le lecteur (une seule tâche, mais la tâche doit pouvoir changer de fil).
    let sent = AtomicU64::new(offset);
    let acked = AtomicU64::new(offset);
    let acks_seen = AtomicBool::new(false);
    let base = Instant::now();
    let activity = AtomicU64::new(0);
    let touch = || activity.store(base.elapsed().as_millis() as u64, Relaxed);
    let timeout = t.timeout;
    let result: (u64, Option<NetError>) = {
        // Écrivain : tout le fichier, d'avance.
        let writer = async {
            let mut buf = vec![0u8; packet];
            while sent.load(Relaxed) < size {
                let want = ((size - sent.load(Relaxed)) as usize).min(packet);
                let n = file.read(&mut buf[..want]).await.map_err(rerr)?;
                if n == 0 {
                    return Err(NetError::FileShrunk { path: path.to_path_buf() });
                }
                wr.write_all(&buf[..n]).await.map_err(|e| NetError::io(&e))?;
                sent.store(sent.load(Relaxed) + n as u64, Relaxed);
                touch();
            }
            wr.flush().await.map_err(|e| NetError::io(&e))?;
            // Fin de flux tout de suite (les accusés continuent d'arriver dans l'autre sens) : sert aux
            // destinataires qui lisent jusqu'à la fermeture.
            let _ = wr.shutdown().await;
            Ok::<(), NetError>(())
        };
        // Lecteur : les accusés de réception ; vrai quand le dernier arrive, faux à la fermeture par l'autre.
        let reader = async {
            let w = if wide { 8 } else { 4 };
            let mut buf = [0u8; 4096];
            let mut pending: Vec<u8> = Vec::new();
            loop {
                if acked.load(Relaxed) >= size && acks_seen.load(Relaxed) {
                    return Ok::<bool, io::Error>(true);
                }
                let n = match rd.read(&mut buf).await {
                    Ok(n) => n,
                    Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => 0,
                    Err(e) => return Err(e),
                };
                if n == 0 {
                    return Ok(false);
                }
                pending.extend_from_slice(&buf[..n]);
                while pending.len() >= w {
                    let a = if wide {
                        u64::from_be_bytes(pending[..8].try_into().unwrap_or_default())
                    } else {
                        unwrap32(acked.load(Relaxed), u32::from_be_bytes(pending[..4].try_into().unwrap_or_default()))
                    };
                    pending.drain(..w);
                    if a > acked.load(Relaxed) {
                        acked.store(a.min(size), Relaxed);
                    }
                    acks_seen.store(true, Relaxed);
                    touch();
                }
            }
        };
        tokio::pin!(writer, reader);
        let mut writer_done = size <= offset;
        let mut tick = tokio::time::interval(PROGRESS_EVERY);
        let mut shown = offset;
        loop {
            tokio::select! {
                w = &mut writer, if !writer_done => match w {
                    Ok(()) => writer_done = true,
                    Err(e) => break (acked.load(Relaxed), Some(e)),
                },
                r = &mut reader => {
                    let all_sent = writer_done && sent.load(Relaxed) >= size;
                    break match r {
                        Ok(true) => (size, None),
                        // Fermé par l'autre : fini s'il a tout accusé, ou s'il n'accuse pas (ou mal, au-delà de 4 Gio).
                        Ok(false) if all_sent && (acked.load(Relaxed) >= size || !acks_seen.load(Relaxed) || wide) => (size, None),
                        Ok(false) => {
                            let pos = if acks_seen.load(Relaxed) { acked.load(Relaxed) } else { sent.load(Relaxed) };
                            (pos, Some(NetError::PeerClosed { pos, size }))
                        }
                        Err(_) if all_sent && acked.load(Relaxed) >= size => (size, None),
                        Err(e) => (acked.load(Relaxed), Some(NetError::io(&e))),
                    };
                }
                _ = tick.tick() => {
                    let pos = if acks_seen.load(Relaxed) { acked.load(Relaxed) } else { sent.load(Relaxed) };
                    if pos != shown {
                        shown = pos;
                        t.progress(pos);
                    }
                    if base.elapsed().as_millis() as u64 - activity.load(Relaxed) > timeout.as_millis() as u64 {
                        break (pos, Some(timed_out(timeout)));
                    }
                }
                _ = wait_closing(&mut t.cancel) => break (acked.load(Relaxed), Some(NetError::Cancelled)),
            }
        }
    };
    if !t.cancelled() {
        let _ = tokio::time::timeout(Duration::from_secs(2), wr.shutdown()).await;
    }
    result
}

// -------------------------------------------------------------------------------------------------------------
// Réception

/// Accusés de réception à écrire sans jamais bloquer la lecture : seul le dernier compte (les positions sont
/// cumulées) ; un accusé entamé est toujours terminé, pour ne pas décaler les suivants.
struct Acks {
    w: usize,
    buf: Vec<u8>,
    /// Octets déjà écrits de l'accusé en tête de `buf`.
    head_done: usize,
}

impl Acks {
    fn new(wide: bool) -> Acks {
        Acks { w: if wide { 8 } else { 4 }, buf: Vec::new(), head_done: 0 }
    }

    fn push(&mut self, pos: u64) {
        if self.head_done > 0 {
            self.buf.truncate(self.w - self.head_done);
        } else {
            self.buf.clear();
        }
        if self.w == 8 {
            self.buf.extend_from_slice(&pos.to_be_bytes());
        } else {
            self.buf.extend_from_slice(&(pos as u32).to_be_bytes());
        }
    }

    /// Écrit ce que la socket accepte tout de suite.
    fn try_flush(&mut self, wr: &tokio::net::tcp::OwnedWriteHalf) -> io::Result<()> {
        while !self.buf.is_empty() {
            match wr.try_write(&self.buf) {
                Ok(n) => {
                    self.buf.drain(..n);
                    self.head_done = (self.head_done + n) % self.w;
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(()),
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Écrit tout ce qui reste (fin du transfert).
    async fn finish(&mut self, wr: &mut tokio::net::tcp::OwnedWriteHalf) {
        if !self.buf.is_empty() {
            let _ = tokio::time::timeout(Duration::from_secs(2), wr.write_all(&self.buf)).await;
            self.buf.clear();
        }
    }
}

async fn run_receive(t: &mut Task, stream: TcpStream, path: &Path, offset: u64, size: Option<u64>) -> (u64, Option<NetError>) {
    let werr = |e: io::Error| NetError::FileWrite { path: path.to_path_buf(), cause: IoKind::of(&e) };
    let mut opts = tokio::fs::OpenOptions::new();
    opts.write(true).create(true);
    if offset == 0 {
        opts.truncate(true);
    }
    let mut file = match opts.open(path).await {
        Ok(f) => f,
        Err(e) => return (offset, Some(werr(e))),
    };
    if offset > 0 {
        if let Err(e) = file.set_len(offset).await {
            return (offset, Some(werr(e)));
        }
        if let Err(e) = file.seek(SeekFrom::Start(offset)).await {
            return (offset, Some(werr(e)));
        }
    }
    let (mut rd, mut wr) = stream.into_split();
    let wide = size.is_some_and(|s| s > u32::MAX as u64);
    let mut pos = offset;
    let mut buf = vec![0u8; 64 * 1024];
    let mut last_progress = Instant::now();
    let timeout = t.timeout;
    let mut acks = Acks::new(wide);
    let mut acks_ok = true;
    let error = loop {
        if size.is_some_and(|s| pos >= s) {
            break None;
        }
        tokio::select! {
            r = tokio::time::timeout(timeout, rd.read(&mut buf)) => match r {
                Err(_) => break Some(timed_out(timeout)),
                Ok(Err(e)) if e.kind() != io::ErrorKind::UnexpectedEof => break Some(NetError::io(&e)),
                Ok(Err(_)) | Ok(Ok(0)) => {
                    break match size {
                        Some(s) if pos < s => Some(NetError::PeerClosed { pos, size: s }),
                        _ => None,
                    };
                }
                Ok(Ok(n)) => {
                    let n = match size {
                        Some(s) => n.min((s - pos) as usize),
                        None => n,
                    };
                    if let Err(e) = file.write_all(&buf[..n]).await {
                        break Some(werr(e));
                    }
                    pos += n as u64;
                    // Accusé de réception : la position atteinte, en ordre réseau.
                    if acks_ok {
                        acks.push(pos);
                        acks_ok = acks.try_flush(&wr).is_ok();
                    }
                    if last_progress.elapsed() >= PROGRESS_EVERY {
                        last_progress = Instant::now();
                        t.progress(pos);
                    }
                }
            },
            _ = wait_closing(&mut t.cancel) => break Some(NetError::Cancelled),
        }
    };
    if let Err(e) = file.flush().await
        && error.is_none()
    {
        return (pos, Some(werr(e)));
    }
    drop(file);
    if acks_ok && error.is_none() {
        acks.finish(&mut wr).await;
    }
    let _ = tokio::time::timeout(Duration::from_secs(2), wr.shutdown()).await;
    (pos, error)
}

#[cfg(test)]
mod tests {
    use super::unwrap32;

    #[test]
    fn acks_past_4gib() {
        assert_eq!(unwrap32(0, 100), 100);
        assert_eq!(unwrap32(0xFFFF_FF00, 0x10), 0x1_0000_0010);
        assert_eq!(unwrap32(0x1_0000_0010, 0x20), 0x1_0000_0020);
        assert_eq!(unwrap32(5000, 4000), 4000);
    }
}
