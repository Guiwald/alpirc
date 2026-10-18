//! Sons des événements (Options › Sons, comme dans mIRC) et demandes de sons entre utilisateurs (CTCP SOUND,
//! `/sound`).
//!
//! Chaque événement — on vous nomme, message privé, Notify, offre DCC, connexion, déconnexion, expulsion,
//! invitation, message d'un canal inactif — joue rien, le bip du système ou un fichier, selon les réglages
//! (`[sounds]`). Ceux qui tiennent à une fenêtre (surlignage, privé, canal) se taisent quand on la regarde : fenêtre
//! active et appli au premier plan. « Tout couper » fait taire les événements, les bips des alertes et les demandes
//! de sons ; `/splay`, `/beep` et `/sound`, demandés exprès, jouent toujours.
//!
//! Le moteur ne joue rien lui-même : il rend `Effect::PlaySound(chemin)` ou `Effect::Beep` à l'hôte. Les effets
//! d'un événement passent par sa présentation : un script qui l'arrête (`/haltdef`) arrête aussi son son.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use alpirc_proto::ctcp;

use super::state::{PendingLine, Pres, State};
use super::util::split_word;
use super::window::{Activity, LineKind, WinKind};
use super::{ConnId, Effect, WinId};
use crate::i18n::Key;
use crate::session::MsgKind;
use crate::settings::{EventSound, SoundEvent, SoundPlay, sound_path};

/// Écart minimal entre deux sons d'un même événement : une rafale (réseaux d'un bouncer qui se connectent ensemble,
/// canal qui s'emballe, demandes de sons répétées) ne sonne qu'une fois.
const MIN_GAP_MS: i64 = 1500;

/// Un message plus vieux que ça (rejoué par un bouncer, avec `server-time`) ne sonne pas.
const STALE_MS: i64 = 60_000;

/// Extensions qu'une demande de son reçue peut faire jouer.
const REQUEST_EXTS: [&str; 3] = ["wav", "mp3", "ogg"];

/// Ce que les sons retiennent entre deux événements.
#[derive(Debug, Default)]
pub(crate) struct SoundState {
    /// Heure du dernier son joué, par événement (`None` : demandes de sons).
    last: HashMap<Option<SoundEvent>, i64>,
}

/// Nom de fichier d'une demande de son, s'il est acceptable : un nom seul (pas de chemin, pas de fichier caché),
/// avec l'extension d'un son.
pub(crate) fn request_file_name(name: &str) -> Option<&str> {
    let name = name.trim();
    if name.is_empty() || name.starts_with('.') || name.contains(['/', '\\', ':', '\0']) {
        return None;
    }
    let ext = Path::new(name).extension()?.to_str()?.to_ascii_lowercase();
    REQUEST_EXTS.contains(&ext.as_str()).then_some(name)
}

impl State {
    /// L'effet d'un son choisi : bip du système, ou fichier s'il existe (rien sinon).
    pub(crate) fn sound_effect(&self, s: &EventSound) -> Option<Effect> {
        match s.play {
            SoundPlay::None => None,
            SoundPlay::Beep => Some(Effect::Beep),
            SoundPlay::File => sound_path(&self.data_dir, &s.file).filter(|p| p.is_file()).map(Effect::PlaySound),
        }
    }

    /// Vrai si le son `key` peut jouer maintenant (pas déjà joué il y a moins de [`MIN_GAP_MS`]) ; le retient.
    fn sound_slot(&mut self, key: Option<SoundEvent>) -> bool {
        let now = self.now;
        if self.sounds.last.get(&key).is_some_and(|t| now >= *t && now - *t < MIN_GAP_MS) {
            return false;
        }
        self.sounds.last.insert(key, now);
        true
    }

    /// Son d'un événement d'après les réglages ; rien si tout est coupé, si l'événement est muet, si son fichier
    /// manque ou si le même son vient de jouer.
    pub(crate) fn event_sound(&mut self, ev: SoundEvent) -> Option<Effect> {
        if self.settings.sounds.mute {
            return None;
        }
        let fx = self.sound_effect(self.settings.sounds.get(ev))?;
        self.sound_slot(Some(ev)).then_some(fx)
    }

    /// Son d'un message reçu dans `win` : seulement si la fenêtre est hors de vue (pas active, ou appli sans le
    /// focus) et si le message n'est pas un vieux message rejoué.
    pub(crate) fn message_sound(&mut self, ev: SoundEvent, win: WinId, time: i64) -> Option<Effect> {
        if !self.unseen(win) || time < self.now - STALE_MS {
            return None;
        }
        self.event_sound(ev)
    }

    /// CTCP SOUND reçu (`fichier [message]`) : le message comme une action ; le son joue si les demandes sont
    /// acceptées, que rien n'est coupé et que le fichier est dans le dossier des sons (un nom seul, jamais un
    /// chemin) ; sinon « veut vous faire écouter… ».
    pub(crate) fn ctcp_sound(&mut self, conn: ConnId, from: &str, chan: Option<&str>, params: &str, time: i64, mut p: Pres) -> Pres {
        let (file, text) = split_word(params);
        let win = chan
            .and_then(|c| self.find_kind(conn, WinKind::Channel, c))
            .or_else(|| self.find_kind(conn, WinKind::Query, from))
            .unwrap_or_else(|| self.active_for(conn));
        let line = |s: &Self, kind: LineKind, t: String| {
            let mut l = s.pl(win, kind, t);
            l.time_ms = time;
            l
        };
        if !text.is_empty() {
            let mut l = line(self, LineKind::Action, format!("* {from} {text}"));
            l.nick = Some(from.to_string());
            l.activity = Activity::Message;
            p = p.line(l);
        }
        let tag = match chan {
            Some(c) => format!("[{from}:{c} SOUND]"),
            None => format!("[{from} SOUND]"),
        };
        if file.is_empty() {
            return p.line(line(self, LineKind::Ctcp, tag));
        }
        let s = &self.settings.sounds;
        let path = request_file_name(file)
            .filter(|_| s.accept_requests && !s.mute)
            .map(|n| self.data_dir.join("sounds").join(n))
            .filter(|p| p.is_file());
        match path {
            Some(path) => {
                if self.sound_slot(None) {
                    p = p.effect(Effect::PlaySound(path));
                }
                if text.is_empty() {
                    p = p.line(line(self, LineKind::Ctcp, format!("{tag} {file}")));
                }
            }
            None => {
                let t = self.t(Key::SoundWants, &[("nick", from), ("file", file)]);
                p = p.line(line(self, LineKind::Ctcp, t));
            }
        }
        p
    }

    /// Lignes de notre propre demande de son (écho du serveur, ou envoi sans echo-message) : le message comme une
    /// action, ou `-> [cible] SOUND fichier` sans message.
    pub(crate) fn own_sound_lines(&self, conn: ConnId, target: &str, target_win: Option<WinId>, params: &str, time: i64) -> Vec<PendingLine> {
        let me = self.me(conn);
        let (file, text) = split_word(params);
        let mk = |win: WinId, kind: LineKind, t: String| {
            let mut l = self.pl(win, kind, t);
            l.time_ms = time;
            l.nick = Some(me.clone());
            l.activity = Activity::None;
            l
        };
        let l = match (target_win, text.is_empty()) {
            (Some(w), false) => mk(w, LineKind::Action, format!("* {me} {text}")),
            (None, false) => {
                let t = self.t(Key::MsgSent, &[("nick", target), ("text", &format!("* {me} {text}"))]);
                mk(self.active_for(conn), LineKind::Action, t)
            }
            (w, true) => {
                let t = self.t(Key::CtcpSent, &[("nick", target), ("cmd", &format!("SOUND {file}"))]);
                mk(w.unwrap_or_else(|| self.active_for(conn)), LineKind::Ctcp, t)
            }
        };
        vec![l]
    }

    /// `/sound [pseudo|#canal] fichier [message]` : demande de son (CTCP SOUND), le message comme une action ; la
    /// cible par défaut est la fenêtre (canal ou privé). Le son joue aussi chez nous s'il est dans le dossier des
    /// sons. `/sound on|off` : accepter ou refuser les demandes reçues.
    pub(crate) fn cmd_sound(&mut self, conn: Option<ConnId>, win: WinId, args: &str, quiet: bool) -> Result<(), String> {
        let args = args.trim();
        if args.eq_ignore_ascii_case("on") || args.eq_ignore_ascii_case("off") {
            let on = args.eq_ignore_ascii_case("on");
            self.settings.sounds.accept_requests = on;
            self.effects.push(Effect::SaveSettings);
            let t = self.t(if on { Key::SoundRequestsOn } else { Key::SoundRequestsOff }, &[]);
            self.print(win, LineKind::Info, t);
            return Ok(());
        }
        let insufficient = |s: &Self| s.t(Key::InsufficientParams, &[("cmd", "sound")]);
        let (first, rest) = split_word(args);
        if first.is_empty() {
            return Err(insufficient(self));
        }
        let is_chan = conn.and_then(|c| self.conns.get(&c)).map_or(first.starts_with(['#', '&']), |c| c.session.is_channel(first));
        // Une cible est donnée si le premier mot est un canal, ou un pseudo (sans point) suivi d'un fichier.
        let names_target = is_chan || (!rest.is_empty() && !first.contains('.') && split_word(rest).0.contains('.'));
        let (target, file, text) = if names_target {
            let (f, t) = split_word(rest);
            (first.to_string(), f, t)
        } else {
            match self.win(win).filter(|w| matches!(w.kind, WinKind::Channel | WinKind::Query)) {
                Some(w) => (w.name.clone(), first, rest),
                None => return Err(self.t(Key::NotOnChannel, &[])),
            }
        };
        if file.is_empty() {
            return Err(insufficient(self));
        }
        let c = match conn.and_then(|c| self.conns.get(&c)) {
            Some(c) if c.connected() => c.id,
            _ => return Err(self.t(Key::NotConnected, &[])),
        };
        // Le nom seul : l'autre le cherche dans son propre dossier des sons.
        let name = Path::new(file).file_name().and_then(|n| n.to_str()).unwrap_or(file).to_string();
        let params = if text.is_empty() { name.clone() } else { format!("{name} {text}") };
        let body = ctcp::encode("SOUND", &params);
        self.send_line(c, &super::commands::text_line("PRIVMSG", &target, &body));
        let local = if Path::new(file).is_absolute() {
            Some(PathBuf::from(file))
        } else {
            request_file_name(&name).map(|n| self.data_dir.join("sounds").join(n))
        };
        if let Some(path) = local.filter(|p| p.is_file()) {
            self.effects.push(Effect::PlaySound(path));
        }
        let echo = self.conns.get(&c).is_some_and(|x| x.session.cap("echo-message"));
        if !echo && !quiet {
            for l in self.own_lines(c, MsgKind::Privmsg, &target, &body, self.now) {
                self.add_line(l);
            }
        }
        Ok(())
    }
}

impl super::Client {
    /// Joue un son choisi dans les Options (bouton « Essayer »), même quand les sons sont coupés. Faux s'il n'y a
    /// rien à jouer (aucun son, fichier introuvable).
    pub fn test_sound(&mut self, s: &EventSound) -> bool {
        match self.st.sound_effect(s) {
            Some(e) => {
                self.st.effects.push(e);
                true
            }
            None => false,
        }
    }

    /// Dossier des sons (`<données>/sounds`) : sons fournis, sons de l'utilisateur, seuls fichiers qu'une demande
    /// de son reçue peut faire jouer.
    pub fn sounds_dir(&self) -> PathBuf {
        self.st.data_dir.join("sounds")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_names() {
        assert_eq!(request_file_name("ding.wav"), Some("ding.wav"));
        assert_eq!(request_file_name(" Tada.MP3 "), Some("Tada.MP3"));
        assert_eq!(request_file_name("x.ogg"), Some("x.ogg"));
        for bad in ["", "../x.wav", "/etc/x.wav", "a/b.wav", "a\\b.wav", "C:x.wav", ".hidden.wav", "script.mrc", "noext"] {
            assert_eq!(request_file_name(bad), None, "{bad}");
        }
    }
}
