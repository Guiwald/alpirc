//! Couche réseau d'AlpIRC : connexions TCP et TLS côté client (réseaux IRC) et côté serveur (relais).
//!
//! Tout passe par [`Net`], qui fait tourner un petit environnement tokio (2 fils) et prévient l'application
//! par un rappel ([`NetEvent`]) appelé depuis les fils réseau. Les méthodes de [`Net`] ne bloquent pas
//! et s'appellent depuis n'importe quel fil, en général celui de l'interface, y compris depuis le rappel ;
//! seules [`Net::listen`], [`Net::stop_listener`] et [`Net::shutdown`] attendent brièvement (une seconde ou
//! deux au plus) et ne doivent pas être appelées depuis le rappel.
//!
//! TLS : rustls avec la cryptographie graviola (Rust pur, ADR 0003), racines de Mozilla (`webpki-roots`) et
//! du système (hors Android : `rustls-native-certs` ; sur Android : dossiers de certificats du système).
//!
//! Garanties sur les événements d'une même connexion (même identifiant) :
//! - ils arrivent dans l'ordre, depuis une seule tâche à la fois ;
//! - chaque tentative se termine par exactement un événement final : [`NetEvent::Failed`] (échec avant
//!   [`NetEvent::Connected`]) ou [`NetEvent::Closed`] ; rien n'arrive ensuite pour cet identifiant, sauf
//!   nouvelle connexion avec le même identifiant ;
//! - quand [`Net::connect`] remplace une connexion existante, l'événement final de l'ancienne arrive avant
//!   tout événement de la nouvelle.

mod cert;
mod conn;
mod dcc;
mod tls;

use std::collections::HashMap;
use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

pub use cert::generate_self_signed;
pub use dcc::DccJob;
pub use tls::fingerprint;

/// Longueur maximale d'une ligne reçue (sans CR/LF) ; au-delà, la ligne est tronquée.
pub const MAX_LINE: usize = 16 * 1024;

/// Premier identifiant attribué aux connexions acceptées par un serveur d'écoute.
pub const FIRST_ACCEPTED_ID: u32 = 1_000_000;

/// Délai maximal pour vider les écritures en attente lors d'une fermeture.
const CLOSE_FLUSH: Duration = Duration::from_secs(2);

/// Comment vérifier le certificat du serveur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verify {
    /// Vérification complète : chaîne jusqu'à une racine de confiance, dates, nom du serveur.
    Full,
    /// Tout certificat est accepté (l'empreinte est tout de même calculée et rapportée).
    AcceptInvalid,
    /// Seul le certificat dont l'empreinte SHA-256 (du certificat final, en DER) est donnée est accepté,
    /// sans vérifier la chaîne ni le nom. Hexadécimal, casse indifférente, séparateurs `:` permis.
    Pinned(String),
}

/// Où et comment se connecter.
#[derive(Clone)]
pub struct Target {
    pub host: String,
    pub port: u16,
    pub tls: bool,
    pub verify: Verify,
    /// Certificat client facultatif pour SASL EXTERNAL / CertFP : chaîne PEM et clé privée PEM (PKCS#8,
    /// SEC1 pour les courbes P-256/P-384, PKCS#1 ou PKCS#8 pour RSA, PKCS#8 pour Ed25519).
    pub client_cert: Option<(Vec<u8>, Vec<u8>)>,
    /// Adresse locale à utiliser (vhost) ; seules les adresses distantes de la même famille sont essayées.
    pub bind: Option<IpAddr>,
    /// Délai global (résolution, connexion TCP et négociation TLS).
    pub connect_timeout: Duration,
}

impl Target {
    /// Cible avec vérification complète, sans certificat client ni adresse locale, délai de 30 s.
    pub fn new(host: impl Into<String>, port: u16, tls: bool) -> Target {
        Target {
            host: host.into(),
            port,
            tls,
            verify: Verify::Full,
            client_cert: None,
            bind: None,
            connect_timeout: Duration::from_secs(30),
        }
    }
}

impl fmt::Debug for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Target")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("tls", &self.tls)
            .field("verify", &self.verify)
            .field("client_cert", &self.client_cert.as_ref().map(|_| "…"))
            .field("bind", &self.bind)
            .field("connect_timeout", &self.connect_timeout)
            .finish()
    }
}

/// Informations sur la session TLS d'une connexion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TlsInfo {
    /// « TLSv1.3 » ou « TLSv1.2 ».
    pub protocol: String,
    /// Suite de chiffrement, par exemple « TLS13_AES_256_GCM_SHA384 ».
    pub cipher: String,
    /// Empreinte SHA-256 du certificat du serveur (hexadécimal minuscule, sans séparateur).
    pub cert_sha256: String,
    /// Chaîne vérifiée jusqu'à une racine de confiance (faux en mode `AcceptInvalid` ou `Pinned`).
    pub verified: bool,
}

/// Ce que la couche réseau signale à l'application.
#[derive(Debug, Clone)]
pub enum NetEvent {
    /// Connexion établie (TLS négocié s'il est demandé).
    Connected { conn: u32, peer: SocketAddr, local: SocketAddr, tls: Option<TlsInfo> },
    /// Une ligne reçue, sans CR/LF (octets bruts : le décodage revient au client). Les lignes vides sont
    /// ignorées ; au-delà de [`MAX_LINE`] octets la ligne est tronquée (la suite est jetée jusqu'au LF
    /// suivant). Une dernière ligne sans LF avant la fin de connexion est livrée aussi.
    Line { conn: u32, bytes: Vec<u8> },
    /// Connexion terminée : fermée par le pair, erreur, ou [`Net::close`]. `error` vaut `None` pour une
    /// fermeture demandée par nous ou une fin de flux normale. Arrive aussi (avec `None`) quand
    /// [`Net::close`] interrompt une tentative pas encore aboutie.
    Closed { conn: u32, error: Option<String> },
    /// Échec de la tentative avant `Connected` (résolution, refus, délai, TLS). Pour un certificat refusé,
    /// `cert_sha256` donne l'empreinte du certificat présenté, pour proposer « faire confiance à ce
    /// certificat » ([`Verify::Pinned`]).
    Failed { conn: u32, error: String, cert_sha256: Option<String> },
    /// Côté serveur : un client s'est connecté à `listener` (négociation TLS terminée s'il y a lieu).
    /// L'identifiant `conn` est attribué par [`Net`] (à partir de [`FIRST_ACCEPTED_ID`]) et s'utilise avec
    /// [`Net::send`] et [`Net::close`] ; la connexion produit ensuite les mêmes `Line` et `Closed`.
    Accepted { listener: u32, conn: u32, peer: SocketAddr, tls: bool },
    /// Côté serveur : l'écoute s'est arrêtée, sur erreur (`Some`) ou par [`Net::stop_listener`] (`None`).
    ListenerClosed { listener: u32, error: Option<String> },
    /// DCC : la connexion directe `id` est établie (sortante, ou pair accepté par [`Net::dcc_listen`]).
    DccConnected { id: u32, peer: SocketAddr },
    /// DCC CHAT : une ligne reçue, sans CR/LF (octets bruts).
    DccLine { id: u32, bytes: Vec<u8> },
    /// DCC SEND/GET : position atteinte (accusée par le destinataire, ou reçue et écrite), départ compris ;
    /// 5 fois par seconde au plus.
    DccProgress { id: u32, bytes: u64 },
    /// DCC : fin, toujours exactement une par identifiant : `error` vaut `None` pour un transfert complet ou
    /// une discussion terminée normalement ; `bytes` : position finale.
    DccEnded { id: u32, bytes: u64, error: Option<String> },
}

impl NetEvent {
    /// Identifiant de connexion concerné (aucun pour `ListenerClosed`).
    pub fn conn(&self) -> Option<u32> {
        match self {
            NetEvent::Connected { conn, .. }
            | NetEvent::Line { conn, .. }
            | NetEvent::Closed { conn, .. }
            | NetEvent::Failed { conn, .. }
            | NetEvent::Accepted { conn, .. } => Some(*conn),
            NetEvent::ListenerClosed { .. } => None,
            // Les identifiants DCC sont à part (voir `Net::dcc_listen`).
            NetEvent::DccConnected { .. } | NetEvent::DccLine { .. } | NetEvent::DccProgress { .. } | NetEvent::DccEnded { .. } => None,
        }
    }
}

/// Certificat et clé (PEM) d'un serveur d'écoute TLS.
#[derive(Clone)]
pub struct ServerTls {
    pub cert_chain_pem: Vec<u8>,
    pub key_pem: Vec<u8>,
}

impl fmt::Debug for ServerTls {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerTls").field("cert_chain_pem", &self.cert_chain_pem.len()).finish_non_exhaustive()
    }
}

/// Construit la configuration client rustls (graviola ; racines webpki-roots et système ; vérificateurs
/// pour `AcceptInvalid` et `Pinned` ; certificat client facultatif). Les signatures de la négociation sont
/// vérifiées dans tous les modes.
pub fn client_config(
    verify: &Verify,
    client_cert: Option<(&[u8], &[u8])>,
) -> Result<Arc<rustls::ClientConfig>, String> {
    tls::build_client_config(verify, client_cert, None)
}

type Notify = Box<dyn Fn(NetEvent) + Send + Sync + 'static>;

/// Ordres envoyés à la tâche d'une connexion.
pub(crate) enum Cmd {
    Line(Vec<u8>),
    Close,
}

/// États d'une connexion.
pub(crate) const CONNECTING: u8 = 0;
pub(crate) const CONNECTED: u8 = 1;
pub(crate) const ENDED: u8 = 2;

pub(crate) struct ConnEntry {
    pub(crate) generation: u64,
    cmds: mpsc::UnboundedSender<Cmd>,
    closing: watch::Sender<bool>,
    state: Arc<AtomicU8>,
    /// Octets mis en file par [`Net::send`] et pas encore écrits dans la socket (CRLF compris).
    pending: Arc<AtomicUsize>,
    task: Option<JoinHandle<()>>,
}

impl ConnEntry {
    fn request_close(&self) {
        let _ = self.cmds.send(Cmd::Close);
        self.closing.send_replace(true);
    }
}

/// Ce que reçoit la tâche d'une connexion (l'autre moitié de [`ConnEntry`]).
pub(crate) struct ConnHandles {
    pub(crate) conn: u32,
    pub(crate) generation: u64,
    pub(crate) cmds: mpsc::UnboundedReceiver<Cmd>,
    pub(crate) closing: watch::Receiver<bool>,
    pub(crate) state: Arc<AtomicU8>,
    /// Compteur des octets en attente d'écriture (partagé avec [`ConnEntry`]).
    pub(crate) pending: Arc<AtomicUsize>,
}

struct ListenerEntry {
    generation: u64,
    stop: watch::Sender<bool>,
    done: std::sync::mpsc::Receiver<()>,
}

pub(crate) struct Inner {
    handle: tokio::runtime::Handle,
    runtime: Mutex<Option<tokio::runtime::Runtime>>,
    notify: Notify,
    pub(crate) conns: Mutex<HashMap<u32, ConnEntry>>,
    listeners: Mutex<HashMap<u32, ListenerEntry>>,
    /// Connexions DCC (identifiants à part).
    pub(crate) dcc: Mutex<HashMap<u32, dcc::DccEntry>>,
    next_accepted: AtomicU32,
    next_generation: AtomicU64,
    /// Arrêt commencé : plus de nouvelle connexion ni d'écoute.
    pub(crate) stopping: AtomicBool,
    /// Arrêt terminé : plus aucun événement.
    silent: AtomicBool,
}

pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Inner {
    pub(crate) fn emit(&self, ev: NetEvent) {
        if !self.silent.load(Ordering::Acquire) {
            (self.notify)(ev);
        }
    }

    /// Crée l'entrée d'une connexion (sous le verrou déjà pris) et les poignées de sa tâche.
    fn new_entry(&self, conn: u32) -> (ConnEntry, ConnHandles) {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (close_tx, close_rx) = watch::channel(false);
        let state = Arc::new(AtomicU8::new(CONNECTING));
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        let pending = Arc::new(AtomicUsize::new(0));
        let entry = ConnEntry {
            generation,
            cmds: cmd_tx,
            closing: close_tx,
            state: state.clone(),
            pending: pending.clone(),
            task: None,
        };
        (entry, ConnHandles { conn, generation, cmds: cmd_rx, closing: close_rx, state, pending })
    }

    /// Fin d'une tâche de connexion : état final, événement final éventuel, retrait du registre.
    pub(crate) fn finish(&self, h: &ConnHandles, ev: Option<NetEvent>) {
        h.state.store(ENDED, Ordering::Release);
        if let Some(ev) = ev {
            self.emit(ev);
        }
        let mut conns = lock(&self.conns);
        if conns.get(&h.conn).is_some_and(|e| e.generation == h.generation) {
            conns.remove(&h.conn);
        }
    }

    /// Enregistre une connexion acceptée par un serveur d'écoute et rend ses poignées.
    pub(crate) fn register_accepted(&self) -> Option<ConnHandles> {
        if self.stopping.load(Ordering::Acquire) {
            return None;
        }
        let mut conns = lock(&self.conns);
        let conn = loop {
            let id = self
                .next_accepted
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                    Some(if v == u32::MAX { FIRST_ACCEPTED_ID } else { v + 1 })
                })
                .unwrap_or(FIRST_ACCEPTED_ID);
            if !conns.contains_key(&id) {
                break id;
            }
        };
        let (entry, handles) = self.new_entry(conn);
        conns.insert(conn, entry);
        Some(handles)
    }

    /// Retient la tâche d'une connexion acceptée (pour l'attendre à l'arrêt).
    pub(crate) fn set_task(&self, conn: u32, generation: u64, task: JoinHandle<()>) {
        if let Some(e) = lock(&self.conns).get_mut(&conn).filter(|e| e.generation == generation) {
            e.task = Some(task);
        }
    }

    pub(crate) fn remove_listener_if(&self, listener: u32, generation: u64) {
        let mut listeners = lock(&self.listeners);
        if listeners.get(&listener).is_some_and(|l| l.generation == generation) {
            listeners.remove(&listener);
        }
    }
}

/// La couche réseau : connexions sortantes, serveurs d'écoute et connexions acceptées.
pub struct Net {
    inner: Arc<Inner>,
}

impl Net {
    /// `notify` est appelé depuis les fils réseau pour chaque événement (l'application le pousse dans sa file
    /// et réveille sa boucle). Il ne doit jamais bloquer longtemps ni paniquer.
    pub fn new(notify: Box<dyn Fn(NetEvent) + Send + Sync + 'static>) -> Net {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("alpirc-net")
            .enable_io()
            .enable_time()
            .build()
            .expect("environnement tokio de la couche réseau");
        let inner = Inner {
            handle: runtime.handle().clone(),
            runtime: Mutex::new(Some(runtime)),
            notify,
            conns: Mutex::new(HashMap::new()),
            listeners: Mutex::new(HashMap::new()),
            dcc: Mutex::new(HashMap::new()),
            next_accepted: AtomicU32::new(FIRST_ACCEPTED_ID),
            next_generation: AtomicU64::new(1),
            stopping: AtomicBool::new(false),
            silent: AtomicBool::new(false),
        };
        Net { inner: Arc::new(inner) }
    }

    /// Lance une connexion (sans bloquer). Une connexion existante avec le même identifiant est d'abord
    /// fermée : son `Closed` (ou `Failed`) arrive avant les événements de la nouvelle.
    pub fn connect(&self, conn: u32, target: Target) {
        let inner = &self.inner;
        if inner.stopping.load(Ordering::Acquire) {
            log::warn!("connexion {conn} refusée : couche réseau arrêtée");
            return;
        }
        log::info!("connexion {conn} vers {}:{} (TLS : {})", target.host, target.port, target.tls);
        let mut conns = lock(&inner.conns);
        let previous = conns.remove(&conn).and_then(|mut old| {
            old.request_close();
            old.task.take()
        });
        let (mut entry, handles) = inner.new_entry(conn);
        let task = inner.handle.spawn(conn::client_task(inner.clone(), handles, target, previous));
        entry.task = Some(task);
        conns.insert(conn, entry);
    }

    /// Met une ligne en file (Net ajoute CRLF). Les lignes partent dans l'ordre ; pendant la tentative de
    /// connexion elles attendent, après la fin elles sont ignorées. La ligne est coupée au premier CR ou LF
    /// qu'elle contiendrait (pas d'injection de commande) ; une ligne vide n'est pas envoyée.
    pub fn send(&self, conn: u32, mut line: Vec<u8>) {
        if let Some(cut) = line.iter().position(|&b| b == b'\r' || b == b'\n') {
            if line[cut..].iter().any(|&b| b != b'\r' && b != b'\n') {
                log::warn!("connexion {conn} : ligne coupée à son premier saut de ligne");
            }
            line.truncate(cut);
        }
        if line.is_empty() {
            return;
        }
        if let Some(e) = lock(&self.inner.conns).get(&conn)
            && e.state.load(Ordering::Acquire) != ENDED
        {
            let n = line.len() + 2;
            if e.cmds.send(Cmd::Line(line)).is_ok() {
                e.pending.fetch_add(n, Ordering::Relaxed);
            }
        }
    }

    /// Octets mis en file par [`Net::send`] et pas encore écrits dans la socket (CRLF compris), pour une
    /// connexion ; 0 pour une connexion inconnue ou terminée. Une écriture compte comme faite quand le
    /// système l'a acceptée : le nombre grandit quand le pair ne lit plus (contre-pression). Le relais s'en sert
    /// pour couper un appareil qui ne suit plus.
    pub fn pending_bytes(&self, conn: u32) -> usize {
        lock(&self.inner.conns).get(&conn).map(|e| e.pending.load(Ordering::Relaxed)).unwrap_or(0)
    }

    /// Ferme : vide les écritures en attente (2 s au plus), termine TLS (close_notify) puis TCP ; produit
    /// `Closed { error: None }`. Interrompt une tentative en cours (même événement).
    pub fn close(&self, conn: u32) {
        if let Some(e) = lock(&self.inner.conns).get(&conn) {
            e.request_close();
        }
    }

    /// Côté serveur (relais) : écoute sur `addr`, en TLS si `tls` est donné. Rend l'adresse effective (le
    /// port 0 donne un port libre). Un serveur d'écoute de même identifiant est d'abord arrêté.
    pub fn listen(&self, listener: u32, addr: SocketAddr, tls: Option<ServerTls>) -> Result<SocketAddr, String> {
        let inner = &self.inner;
        if inner.stopping.load(Ordering::Acquire) {
            return Err("couche réseau arrêtée".into());
        }
        let acceptor = match tls {
            Some(t) => Some(tokio_rustls::TlsAcceptor::from(tls::server_config(&t)?)),
            None => None,
        };
        self.stop_listener(listener);
        let fail = |e: std::io::Error| format!("écoute sur {addr} : {}", conn::describe_io(&e));
        let std_listener = conn::bind_listener(addr).map_err(fail)?;
        let bound = std_listener.local_addr().map_err(fail)?;
        let tcp = {
            let _guard = inner.handle.enter();
            tokio::net::TcpListener::from_std(std_listener).map_err(fail)?
        };
        let (stop_tx, stop_rx) = watch::channel(false);
        let (done_tx, done_rx) = std::sync::mpsc::sync_channel(1);
        let generation = inner.next_generation.fetch_add(1, Ordering::Relaxed);
        let mut listeners = lock(&inner.listeners);
        inner.handle.spawn(conn::accept_loop(inner.clone(), listener, generation, tcp, acceptor, stop_rx, done_tx));
        if let Some(old) = listeners.insert(listener, ListenerEntry { generation, stop: stop_tx, done: done_rx }) {
            // Appel concurrent avec le même identifiant : l'autre écoute s'arrête aussi.
            old.stop.send_replace(true);
        }
        log::info!("écoute {listener} sur {bound}");
        Ok(bound)
    }

    /// Arrête un serveur d'écoute (son port est libéré au retour, sauf délai d'une seconde dépassé). Les
    /// connexions déjà acceptées continuent. Produit `ListenerClosed { error: None }`.
    pub fn stop_listener(&self, listener: u32) {
        let entry = lock(&self.inner.listeners).remove(&listener);
        if let Some(l) = entry {
            l.stop.send_replace(true);
            let _ = l.done.recv_timeout(Duration::from_secs(1));
        }
    }

    /// Arrête tout (fin de l'application, essais) : écoutes arrêtées, connexions fermées proprement (écritures
    /// en attente vidées, 2 s au plus), puis plus aucun événement. Les appels suivants sont sans effet.
    pub fn shutdown(&self) {
        let inner = &self.inner;
        if inner.stopping.swap(true, Ordering::AcqRel) {
            return;
        }
        log::info!("arrêt de la couche réseau");
        let listeners: Vec<ListenerEntry> = lock(&inner.listeners).drain().map(|(_, l)| l).collect();
        for l in &listeners {
            l.stop.send_replace(true);
        }
        let mut tasks: Vec<JoinHandle<()>> = lock(&inner.conns)
            .values_mut()
            .filter_map(|e| {
                e.request_close();
                e.task.take()
            })
            .collect();
        tasks.extend(dcc::cancel_all(inner));
        let (tx, rx) = std::sync::mpsc::channel();
        inner.handle.spawn(async move {
            for t in tasks {
                let _ = t.await;
            }
            let _ = tx.send(());
        });
        let _ = rx.recv_timeout(CLOSE_FLUSH + Duration::from_millis(500));
        for l in &listeners {
            let _ = l.done.recv_timeout(Duration::from_millis(200));
        }
        inner.silent.store(true, Ordering::Release);
        if let Some(rt) = lock(&inner.runtime).take() {
            if tokio::runtime::Handle::try_current().is_ok() {
                // Appelé depuis un contexte asynchrone : on ne peut pas y attendre.
                rt.shutdown_background();
            } else {
                rt.shutdown_timeout(Duration::from_millis(500));
            }
        }
    }

    /// Vrai entre `Connected` (ou `Accepted`) et l'événement final.
    pub fn is_connected(&self, conn: u32) -> bool {
        lock(&self.inner.conns).get(&conn).is_some_and(|e| e.state.load(Ordering::Acquire) == CONNECTED)
    }
}

impl Drop for Net {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn net_is_send_sync() {
        fn check<T: Send + Sync>() {}
        check::<super::Net>();
        check::<super::NetEvent>();
    }
}
