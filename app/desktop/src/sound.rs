//! Les sons : fichiers wav, mp3, ogg (vorbis) et flac, et bip synthétisé, joués sans bloquer l'interface.
//!
//! Un fil « alpirc-son » décode le fichier au fil de la lecture (symphonia, Rust pur), le met au format de la sortie
//! (canaux, fréquence) et remplit une file que vide le rappel de la sortie audio (cpal : CoreAudio sur Mac, ALSA sous
//! Linux). Comme dans mIRC, un nouveau son remplace celui qui joue. Les bips, eux, se suivent (`/beep 3`), dix au plus
//! en attente, et se mêlent au son en cours. La sortie n'est ouverte que le temps de jouer, et quelques secondes
//! après : rien ne garde la carte son occupée entre deux sons. Un fichier absent ou illisible est ignoré (une ligne
//! dans le journal).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
use symphonia::core::errors::Error as DecodeError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Avance du décodage : la file de la sortie garde à peu près ce temps de son d'avance.
const AHEAD_MS: u32 = 300;
/// La sortie se ferme après ce temps sans rien à jouer.
const IDLE_CLOSE: Duration = Duration::from_secs(3);
/// Bip : durée du son, silence qui le suit (pour que `/beep 3` fasse trois bips distincts), fréquence, volume.
const BEEP_MS: u32 = 140;
const BEEP_GAP_MS: u32 = 110;
const BEEP_HZ: f32 = 880.0;
const BEEP_LEVEL: f32 = 0.3;
/// Bips en attente, au plus (les suivants sont oubliés).
const BEEP_MAX: usize = 10;

enum Cmd {
    File(PathBuf),
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    Beep,
}

/// Le lecteur de sons de l'application (le fil de lecture démarre au premier son).
pub struct Sound {
    tx: Option<Sender<Cmd>>,
}

impl Sound {
    pub fn new() -> Sound {
        Sound { tx: None }
    }

    /// Joue un fichier (wav, mp3, ogg, flac) à la place du son en cours.
    pub fn play_file(&mut self, path: &Path) {
        self.send(Cmd::File(path.to_path_buf()));
    }

    /// Un bip court synthétisé (Linux ; le Mac a le bip du système).
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub fn beep(&mut self) {
        self.send(Cmd::Beep);
    }

    fn send(&mut self, cmd: Cmd) {
        if self.tx.is_none() {
            let (tx, rx) = mpsc::channel();
            match std::thread::Builder::new().name("alpirc-son".into()).spawn(move || run(rx)) {
                Ok(_) => self.tx = Some(tx),
                Err(e) => {
                    log::warn!("son : fil de lecture impossible ({e})");
                    return;
                }
            }
        }
        // Fil arrêté (panique) : il sera relancé au son suivant.
        if self.tx.as_ref().is_some_and(|tx| tx.send(cmd).is_err()) {
            self.tx = None;
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Le fil de lecture.

fn run(rx: Receiver<Cmd>) {
    let mut out: Option<Output> = None;
    let mut playing: Option<Playback> = None;
    let mut pending: Option<Cmd> = None;
    let mut idle_since: Option<Instant> = None;
    loop {
        let cmd = match pending.take() {
            Some(c) => Some(c),
            None if playing.is_some() => match rx.try_recv() {
                Ok(c) => Some(c),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => return,
            },
            None if out.is_some() => match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(c) => Some(c),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return,
            },
            None => match rx.recv() {
                Ok(c) => Some(c),
                Err(_) => return,
            },
        };
        if let Some(cmd) = cmd {
            if out.as_ref().is_some_and(Output::failed) {
                out = None;
            }
            if out.is_none() {
                match Output::open() {
                    Ok(o) => out = Some(o),
                    Err(e) => {
                        log::warn!("son : pas de sortie audio ({e})");
                        playing = None;
                        continue;
                    }
                }
            }
            let o = out.as_ref().expect("sortie");
            match cmd {
                Cmd::File(path) => {
                    o.queues().sound.clear();
                    playing = match Decoder::open_file(&path) {
                        Ok(d) => Some(Playback { decoder: d, conv: Converter::new(o.rate, o.channels) }),
                        Err(e) => {
                            log::warn!("son ignoré : {} ({e})", path.display());
                            None
                        }
                    };
                }
                Cmd::Beep => {
                    let one = beep_samples(o.rate, o.channels);
                    let mut q = o.queues();
                    if q.beeps.len() + one.len() <= one.len() * BEEP_MAX {
                        q.beeps.extend(one);
                    }
                }
            }
            idle_since = None;
        }
        // Décodage, avec un peu d'avance sur la lecture.
        if let (Some(o), Some(p)) = (&out, &mut playing) {
            if o.failed() {
                playing = None;
                continue;
            }
            let ahead = (o.rate * AHEAD_MS / 1000) as usize * o.channels;
            if o.queues().sound.len() < ahead {
                match p.next() {
                    Some(samples) => o.queues().sound.extend(samples),
                    None => playing = None,
                }
            } else {
                match rx.recv_timeout(Duration::from_millis(20)) {
                    Ok(c) => pending = Some(c),
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
            continue;
        }
        // Plus rien à jouer : la sortie se ferme un peu plus tard.
        if let Some(o) = &out {
            if o.failed() {
                out = None;
                idle_since = None;
            } else if o.queues().is_empty() {
                let since = *idle_since.get_or_insert_with(Instant::now);
                if since.elapsed() >= IDLE_CLOSE {
                    out = None;
                    idle_since = None;
                }
            } else {
                idle_since = None;
            }
        }
    }
}

/// Un fichier en cours de lecture, converti au fur et à mesure pour la sortie.
struct Playback {
    decoder: Decoder,
    conv: Converter,
}

impl Playback {
    /// Les échantillons suivants (au format de la sortie) ; `None` à la fin du fichier.
    fn next(&mut self) -> Option<Vec<f32>> {
        let chunk = self.decoder.next()?;
        Some(self.conv.convert(&chunk))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// La sortie audio.

/// Ce que lit le rappel de la sortie : le son en cours et les bips, en échantillons entrelacés (trames entières).
#[derive(Default)]
struct Queues {
    sound: VecDeque<f32>,
    beeps: VecDeque<f32>,
}

impl Queues {
    fn is_empty(&self) -> bool {
        self.sound.is_empty() && self.beeps.is_empty()
    }

    /// Remplit un tampon de la sortie : son et bips mêlés, silence quand il n'y a rien.
    fn fill<T: cpal::Sample + cpal::FromSample<f32>>(&mut self, out: &mut [T]) {
        for s in out {
            let a = self.sound.pop_front().unwrap_or(0.0);
            let b = self.beeps.pop_front().unwrap_or(0.0);
            *s = T::from_sample((a + b).clamp(-1.0, 1.0));
        }
    }
}

struct Output {
    _stream: cpal::Stream,
    queues: Arc<Mutex<Queues>>,
    failed: Arc<AtomicBool>,
    rate: u32,
    channels: usize,
}

impl Output {
    /// Ouvre la sortie par défaut du système, à son format préféré.
    fn open() -> Result<Output, String> {
        use cpal::SampleFormat as F;
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or("aucun périphérique de sortie")?;
        let supported = device.default_output_config().map_err(|e| e.to_string())?;
        let config = supported.config();
        if config.channels == 0 || config.sample_rate == 0 {
            return Err(format!("format de sortie inattendu ({} canaux, {} Hz)", config.channels, config.sample_rate));
        }
        let queues = Arc::new(Mutex::new(Queues::default()));
        let failed = Arc::new(AtomicBool::new(false));
        let stream = match supported.sample_format() {
            F::F32 => build::<f32>(&device, &config, &queues, &failed),
            F::F64 => build::<f64>(&device, &config, &queues, &failed),
            F::I16 => build::<i16>(&device, &config, &queues, &failed),
            F::I32 => build::<i32>(&device, &config, &queues, &failed),
            F::I24 => build::<cpal::I24>(&device, &config, &queues, &failed),
            F::I8 => build::<i8>(&device, &config, &queues, &failed),
            F::U16 => build::<u16>(&device, &config, &queues, &failed),
            F::U8 => build::<u8>(&device, &config, &queues, &failed),
            other => return Err(format!("format d'échantillons non pris en charge : {other}")),
        }?;
        stream.play().map_err(|e| e.to_string())?;
        let (ch, rate) = (config.channels, config.sample_rate);
        log::info!("son : sortie ouverte ({ch} canaux, {rate} Hz, {})", supported.sample_format());
        Ok(Output { _stream: stream, queues, failed, rate: config.sample_rate, channels: config.channels as usize })
    }

    fn queues(&self) -> MutexGuard<'_, Queues> {
        self.queues.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn failed(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }
}

fn build<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    queues: &Arc<Mutex<Queues>>,
    failed: &Arc<AtomicBool>,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    use cpal::traits::DeviceTrait;
    let q = queues.clone();
    let f = failed.clone();
    device
        .build_output_stream::<T, _, _>(
            *config,
            move |data: &mut [T], _| q.lock().unwrap_or_else(|e| e.into_inner()).fill(data),
            move |e: cpal::Error| {
                // Seule la perte du périphérique arrête la sortie (elle sera rouverte au son suivant, par exemple sur
                // un nouveau casque). Le reste (décrochages, changement de route, erreurs passagères du greffon
                // PulseAudio d'ALSA au démarrage…) n'empêche pas cpal de continuer : on le note sans plus.
                match e.kind() {
                    cpal::ErrorKind::DeviceNotAvailable | cpal::ErrorKind::StreamInvalidated => {
                        if !f.swap(true, Ordering::Relaxed) {
                            log::warn!("son : sortie audio perdue ({e})");
                        }
                    }
                    _ => log::debug!("son : {e}"),
                }
            },
            None,
        )
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------------------------------------------
// Décodage.

/// Des échantillons décodés, entrelacés.
struct Chunk {
    samples: Vec<f32>,
    channels: usize,
    rate: u32,
}

struct Decoder {
    format: Box<dyn FormatReader>,
    codec: Box<dyn symphonia::core::codecs::Decoder>,
    track: u32,
    buf: Option<SampleBuffer<f32>>,
}

impl Decoder {
    fn open_file(path: &Path) -> Result<Decoder, String> {
        let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
        Decoder::open(Box::new(file), path.extension().and_then(|e| e.to_str()))
    }

    /// Reconnaît le format (d'après le contenu, l'extension aidant) et prépare le décodeur de la première piste.
    fn open(source: Box<dyn MediaSource>, ext: Option<&str>) -> Result<Decoder, String> {
        let mss = MediaSourceStream::new(source, Default::default());
        let mut hint = Hint::new();
        if let Some(ext) = ext {
            hint.with_extension(ext);
        }
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
            .map_err(|e| e.to_string())?;
        let format = probed.format;
        let track = format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL).ok_or("aucune piste audio")?;
        let codec = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()).map_err(|e| e.to_string())?;
        let track = track.id;
        Ok(Decoder { format, codec, track, buf: None })
    }

    /// Le paquet suivant, décodé ; `None` à la fin du fichier ou sur une erreur qui empêche de continuer.
    fn next(&mut self) -> Option<Chunk> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(p) => p,
                Err(DecodeError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => return None,
                Err(DecodeError::ResetRequired) => return None,
                Err(e) => {
                    log::warn!("son : lecture interrompue ({e})");
                    return None;
                }
            };
            if packet.track_id() != self.track {
                continue;
            }
            let decoded = match self.codec.decode(&packet) {
                Ok(d) => d,
                // Un paquet abîmé est sauté, comme le font les lecteurs.
                Err(DecodeError::DecodeError(e)) => {
                    log::debug!("son : paquet illisible ({e})");
                    continue;
                }
                Err(DecodeError::IoError(_)) => continue,
                Err(e) => {
                    log::warn!("son : décodage interrompu ({e})");
                    return None;
                }
            };
            let spec = *decoded.spec();
            let frames = decoded.capacity() as u64;
            let channels = spec.channels.count();
            if channels == 0 || spec.rate == 0 || decoded.frames() == 0 {
                continue;
            }
            if self.buf.as_ref().is_none_or(|b| b.capacity() < decoded.capacity() * channels) {
                self.buf = Some(SampleBuffer::new(frames, spec));
            }
            let buf = self.buf.as_mut().expect("tampon");
            buf.copy_interleaved_ref(decoded);
            return Some(Chunk { samples: buf.samples().to_vec(), channels, rate: spec.rate });
        }
    }
}

/// Met les échantillons décodés au format de la sortie : nombre de canaux, puis fréquence (interpolation linéaire,
/// qui suffit pour des sons d'événements), d'un paquet à l'autre sans raccord audible.
struct Converter {
    out_rate: u32,
    out_ch: usize,
    in_rate: u32,
    /// Position de lecture, en trames, dans [dernière trame du paquet précédent, trames du paquet courant…].
    pos: f64,
    /// Dernière trame du paquet précédent (déjà au bon nombre de canaux).
    prev: Vec<f32>,
}

impl Converter {
    fn new(out_rate: u32, out_ch: usize) -> Converter {
        Converter { out_rate, out_ch, in_rate: 0, pos: 1.0, prev: vec![0.0; out_ch] }
    }

    fn convert(&mut self, chunk: &Chunk) -> Vec<f32> {
        let frames = map_channels(&chunk.samples, chunk.channels, self.out_ch);
        if chunk.rate != self.in_rate {
            self.in_rate = chunk.rate;
            self.pos = 1.0;
        }
        if self.in_rate == self.out_rate {
            if let Some(last) = frames.chunks_exact(self.out_ch).last() {
                self.prev.copy_from_slice(last);
            }
            return frames;
        }
        self.resample(&frames)
    }

    fn resample(&mut self, frames: &[f32]) -> Vec<f32> {
        let ch = self.out_ch;
        let n = frames.len() / ch;
        if n == 0 {
            return Vec::new();
        }
        let step = self.in_rate as f64 / self.out_rate as f64;
        let mut out = Vec::with_capacity(((n as f64 / step) as usize + 2) * ch);
        // Trame k : 0 = la dernière du paquet précédent, 1..=n = celles de ce paquet.
        let frame = |k: usize| if k == 0 { &self.prev[..] } else { &frames[(k - 1) * ch..k * ch] };
        while self.pos < n as f64 {
            let i = self.pos as usize;
            let t = (self.pos - i as f64) as f32;
            let (a, b) = (frame(i), frame(i + 1));
            out.extend((0..ch).map(|c| a[c] + (b[c] - a[c]) * t));
            self.pos += step;
        }
        self.pos -= n as f64;
        self.prev.copy_from_slice(&frames[(n - 1) * ch..n * ch]);
        out
    }
}

/// Passe d'un nombre de canaux à un autre : mono vers tous, plusieurs vers mono par moyenne, sinon canal par canal
/// (les canaux en trop sont laissés, ceux qui manquent restent muets).
fn map_channels(input: &[f32], in_ch: usize, out_ch: usize) -> Vec<f32> {
    if in_ch == out_ch {
        return input.to_vec();
    }
    let mut out = Vec::with_capacity(input.len() / in_ch * out_ch);
    for f in input.chunks_exact(in_ch) {
        if in_ch == 1 {
            out.extend(std::iter::repeat_n(f[0], out_ch));
        } else if out_ch == 1 {
            out.push(f.iter().sum::<f32>() / in_ch as f32);
        } else {
            out.extend((0..out_ch).map(|c| f.get(c).copied().unwrap_or(0.0)));
        }
    }
    out
}

/// Un bip, déjà au format de la sortie : la note, montée et descente douces (pas de clic), puis un court silence.
fn beep_samples(rate: u32, channels: usize) -> Vec<f32> {
    let n = (rate * BEEP_MS / 1000) as usize;
    let gap = (rate * BEEP_GAP_MS / 1000) as usize;
    let attack = (rate as usize / 200).max(1);
    let mut out = Vec::with_capacity((n + gap) * channels);
    for i in 0..n {
        let t = i as f32 / rate as f32;
        let env = (i as f32 / attack as f32).min(1.0) * (1.0 - i as f32 / n as f32).powi(2);
        let w = std::f32::consts::TAU * BEEP_HZ * t;
        let v = BEEP_LEVEL * env * (w.sin() + 0.25 * (2.0 * w).sin()) / 1.25;
        out.extend(std::iter::repeat_n(v, channels));
    }
    out.extend(std::iter::repeat_n(0.0, gap * channels));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un fichier WAV PCM 16 bits.
    fn wav(rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
        let data_len = (samples.len() * 2) as u32;
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36 + data_len).to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes());
        v.extend_from_slice(&channels.to_le_bytes());
        v.extend_from_slice(&rate.to_le_bytes());
        v.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
        v.extend_from_slice(&(channels * 2).to_le_bytes());
        v.extend_from_slice(&16u16.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&data_len.to_le_bytes());
        for s in samples {
            v.extend_from_slice(&s.to_le_bytes());
        }
        v
    }

    fn sine(rate: u32, hz: f32, n: usize) -> Vec<i16> {
        (0..n).map(|i| ((std::f32::consts::TAU * hz * i as f32 / rate as f32).sin() * 16_000.0) as i16).collect()
    }

    fn decode_all(bytes: Vec<u8>, ext: &str) -> (Vec<f32>, usize, u32) {
        let mut d = Decoder::open(Box::new(std::io::Cursor::new(bytes)), Some(ext)).expect("format reconnu");
        let (mut all, mut ch, mut rate) = (Vec::new(), 0, 0);
        while let Some(c) = d.next() {
            all.extend_from_slice(&c.samples);
            ch = c.channels;
            rate = c.rate;
        }
        (all, ch, rate)
    }

    #[test]
    fn decode_un_wav_genere() {
        let pcm = sine(8000, 440.0, 800);
        let (s, ch, rate) = decode_all(wav(8000, 1, &pcm), "wav");
        assert_eq!((ch, rate), (1, 8000));
        assert_eq!(s.len(), 800);
        for (a, b) in s.iter().zip(&pcm) {
            assert!((a - *b as f32 / 32768.0).abs() < 1e-4);
        }
        // Stéréo : les canaux restent entrelacés.
        let st: Vec<i16> = (0..200).flat_map(|i| [i as i16 * 10, -(i as i16) * 10]).collect();
        let (s, ch, _) = decode_all(wav(22_050, 2, &st), "wav");
        assert_eq!((ch, s.len()), (2, 400));
        assert!(s[2] > 0.0 && s[3] < 0.0);
        // Le contenu compte, pas l'extension.
        let (s, _, _) = decode_all(wav(8000, 1, &pcm), "mp3");
        assert_eq!(s.len(), 800);
    }

    #[test]
    fn decode_flac_ogg_mp3() {
        // La 440 Hz d'un quart de seconde, à 22 050 Hz, mono, au huitième du maximum (testdata/, fabriqués par
        // ffmpeg).
        let files: [(&str, &[u8]); 3] = [
            ("flac", include_bytes!("../testdata/la-440.flac")),
            ("ogg", include_bytes!("../testdata/la-440.ogg")),
            ("mp3", include_bytes!("../testdata/la-440.mp3")),
        ];
        for (ext, bytes) in files {
            let (s, ch, rate) = decode_all(bytes.to_vec(), ext);
            assert_eq!((ch, rate), (1, 22_050), "{ext}");
            // À peu près un quart de seconde (les formats avec pertes ajoutent un peu de silence au début et à la fin).
            assert!((5000..=7500).contains(&s.len()), "{ext} : {} échantillons", s.len());
            let peak = s.iter().fold(0f32, |m, v| m.max(v.abs()));
            assert!((0.1..0.16).contains(&peak), "{ext} : crête {peak}");
            // Passages par zéro (avec hystérésis, contre le bruit du silence ajouté par les formats avec pertes) :
            // environ 2 × 440 × 0,25 = 220.
            let mut zc = 0;
            let mut last: Option<bool> = None;
            for v in s.iter().filter(|v| v.abs() > 0.03) {
                let pos = *v > 0.0;
                if last.is_some_and(|l| l != pos) {
                    zc += 1;
                }
                last = Some(pos);
            }
            assert!((200..=240).contains(&zc), "{ext} : {zc} passages par zéro");
        }
    }

    #[test]
    fn sons_fournis() {
        // Les sons livrés avec AlpIRC (assets/sounds/) se lisent tous.
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/sounds");
        let Ok(entries) = std::fs::read_dir(&dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            let Some(ext) = p.extension().and_then(|x| x.to_str()).map(str::to_ascii_lowercase) else { continue };
            if !matches!(ext.as_str(), "wav" | "mp3" | "ogg" | "flac") {
                continue;
            }
            let (s, ch, rate) = decode_all(std::fs::read(&p).unwrap(), &ext);
            assert!(!s.is_empty() && ch > 0 && rate > 0, "{}", p.display());
        }
    }

    #[test]
    fn fichiers_illisibles() {
        assert!(Decoder::open(Box::new(std::io::Cursor::new(b"pas un son du tout".to_vec())), Some("wav")).is_err());
        assert!(Decoder::open(Box::new(std::io::Cursor::new(Vec::new())), None).is_err());
        // Un WAV tronqué se lit jusqu'où il va.
        let mut w = wav(8000, 1, &sine(8000, 440.0, 800));
        w.truncate(44 + 600);
        let (s, _, _) = decode_all(w, "wav");
        assert!(s.len() <= 300);
        assert!(Decoder::open_file(Path::new("/nulle/part/ding.wav")).is_err());
    }

    #[test]
    fn canaux() {
        assert_eq!(map_channels(&[0.5, -0.5], 1, 2), vec![0.5, 0.5, -0.5, -0.5]);
        assert_eq!(map_channels(&[0.2, 0.4, 1.0, 0.0], 2, 1), vec![0.3, 0.5]);
        assert_eq!(map_channels(&[0.1, 0.2], 2, 4), vec![0.1, 0.2, 0.0, 0.0]);
        assert_eq!(map_channels(&[0.1, 0.2, 0.3, 0.4, 0.5, 0.6], 6, 2), vec![0.1, 0.2]);
        assert_eq!(map_channels(&[0.1, 0.2], 2, 2), vec![0.1, 0.2]);
    }

    #[test]
    fn frequence_convertie_sans_raccord() {
        let input: Vec<f32> = (0..1000).map(|i| (i as f32 * 0.05).sin()).collect();
        for (from, to) in [(22_050, 48_000), (48_000, 44_100), (8000, 44_100), (96_000, 48_000)] {
            let chunk = |s: &[f32]| Chunk { samples: s.to_vec(), channels: 1, rate: from };
            let mut whole = Converter::new(to, 1);
            let a = whole.convert(&chunk(&input));
            // Les mêmes échantillons en paquets de tailles diverses donnent la même chose.
            let mut parts = Converter::new(to, 1);
            let mut b = Vec::new();
            let mut i = 0;
            for size in [1, 7, 100, 3, 250, 639].iter().cycle() {
                if i >= input.len() {
                    break;
                }
                let j = (i + size).min(input.len());
                b.extend(parts.convert(&chunk(&input[i..j])));
                i = j;
            }
            assert_eq!(a.len(), b.len(), "{from} → {to}");
            assert!(a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-5), "{from} → {to}");
            // La dernière trame d'entrée attend le paquet suivant : 999 intervalles convertis.
            let expected = 999.0 * to as f64 / from as f64;
            assert!((a.len() as f64 - expected).abs() <= 1.0, "{from} → {to} : {} trames", a.len());
            assert_eq!(a[0], input[0]);
        }
        // Même fréquence : rien ne change.
        let mut c = Converter::new(44_100, 2);
        assert_eq!(c.convert(&Chunk { samples: vec![0.1, 0.2, 0.3, 0.4], channels: 2, rate: 44_100 }), vec![0.1, 0.2, 0.3, 0.4]);
    }

    #[test]
    fn bip() {
        let b = beep_samples(48_000, 2);
        assert_eq!(b.len(), (48_000 * (BEEP_MS + BEEP_GAP_MS) / 1000) as usize * 2);
        assert_eq!(b[0], 0.0, "attaque douce");
        assert!(b.chunks_exact(2).all(|f| f[0] == f[1]));
        let peak = b.iter().fold(0f32, |m, v| m.max(v.abs()));
        assert!(peak > 0.15 && peak <= BEEP_LEVEL);
        let tail = (48_000 * BEEP_MS / 1000) as usize * 2;
        assert!(b[tail..].iter().all(|v| *v == 0.0), "silence après la note");
    }

    /// Lecture par la vraie sortie audio (machine avec une sortie, ou ALSA dirigé vers un fichier) : un son, un
    /// second qui le remplace, trois bips. `cargo test -p alpirc -- --ignored lecture_reelle --nocapture`
    #[test]
    #[ignore]
    fn lecture_reelle() {
        let _ = env_logger::builder().is_test(true).filter_level(log::LevelFilter::Info).try_init();
        let dir = std::env::temp_dir().join(format!("alpirc-son-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (a, b) = (dir.join("a.wav"), dir.join("b.wav"));
        std::fs::write(&a, wav(22_050, 1, &sine(22_050, 440.0, 22_050))).unwrap();
        let stereo: Vec<i16> = sine(44_100, 660.0, 44_100).into_iter().flat_map(|s| [s, s]).collect();
        std::fs::write(&b, wav(44_100, 2, &stereo)).unwrap();
        let mut s = Sound::new();
        s.play_file(&a);
        std::thread::sleep(Duration::from_millis(400));
        s.play_file(&b);
        s.play_file(Path::new("/nulle/part.wav"));
        s.play_file(&b);
        std::thread::sleep(Duration::from_millis(1200));
        for _ in 0..3 {
            s.beep();
        }
        std::thread::sleep(Duration::from_millis(1000));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn melange_dans_la_sortie() {
        let mut q = Queues::default();
        q.sound.extend([0.5, 0.5, 0.9]);
        q.beeps.extend([0.25, 0.75]);
        let mut out = [0f32; 4];
        q.fill(&mut out);
        assert_eq!(out, [0.75, 1.0, 0.9, 0.0]);
        assert!(q.is_empty());
        q.sound.extend([1.0, -1.0, 0.0]);
        let mut out = [0i16; 3];
        q.fill(&mut out);
        assert_eq!(out, [i16::MAX, i16::MIN, 0]);
    }
}
