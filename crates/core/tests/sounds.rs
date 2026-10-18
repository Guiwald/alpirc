//! Sons des événements (Options › Sons) et demandes de sons (CTCP SOUND, `/sound`).

mod common;

use std::path::PathBuf;

use alpirc_core::Effect;
use alpirc_core::settings::{EventSound, NotifyEntry, SoundPlay, Settings};
use alpirc_script::ScriptKind;
use common::*;

/// Un client dont le dossier des sons contient les sons fournis (de faux fichiers suffisent : le moteur ne les lit
/// pas, il vérifie qu'ils existent).
fn with_sounds(s: Settings) -> H {
    let dir = tmpdir();
    std::fs::create_dir_all(dir.join("sounds")).unwrap();
    for f in alpirc_core::settings::BUILTIN_SOUNDS {
        std::fs::write(dir.join("sounds").join(f), b"RIFF").unwrap();
    }
    H::with_dir(s, dir)
}

fn snd(h: &H, f: &str) -> Effect {
    Effect::PlaySound(h.dir.join("sounds").join(f))
}

fn sounds(fx: &[Effect]) -> Vec<Effect> {
    fx.iter().filter(|e| matches!(e, Effect::PlaySound(_) | Effect::Beep)).cloned().collect()
}

fn on(s: &mut Settings, f: impl Fn(&mut Settings) -> &mut EventSound) {
    f(s).play = SoundPlay::File;
}

#[test]
fn highlight_and_query_sound_only_when_unseen() {
    let mut h = with_sounds(settings());
    h.up();
    let _a = h.join("#a", "Guiwald Pat");
    let b = h.join("#b", "Guiwald Pat");
    // Fenêtre active et appli au premier plan : rien.
    h.feed(":Pat!p@h PRIVMSG #b :guiwald, regarde");
    assert!(sounds(&h.effects()).is_empty());
    // Autre canal : le son du surlignage, pas de bip.
    h.feed(":Pat!p@h PRIVMSG #a :guiwald, ici");
    assert_eq!(sounds(&h.effects()), vec![snd(&h, "alpirc-highlight.wav")]);
    // Message privé (le privé s'ouvre sans devenir actif) : son du privé.
    h.advance(2000);
    h.feed(":Pat!p@h PRIVMSG Guiwald :psst");
    assert_eq!(sounds(&h.effects()), vec![snd(&h, "alpirc-query.wav")]);
    assert_eq!(h.c.active(), Some(b));
    // Un message ordinaire d'un canal inactif : rien par défaut.
    h.advance(2000);
    h.feed(":Pat!p@h PRIVMSG #a :bonjour");
    assert!(sounds(&h.effects()).is_empty());
    // Appli sans le focus : même la fenêtre active sonne.
    h.c.set_app_focused(false);
    h.feed(":Pat!p@h PRIVMSG #b :guiwald ?");
    assert_eq!(sounds(&h.effects()), vec![snd(&h, "alpirc-highlight.wav")]);
}

#[test]
fn sound_replaces_alert_beep_and_mute_silences_both() {
    let mut s = settings();
    s.highlight.beep = true;
    s.highlight.beep_on_query = true;
    let mut h = with_sounds(s);
    h.up();
    h.join("#a", "Guiwald Pat");
    h.join("#b", "Guiwald");
    h.feed(":Pat!p@h PRIVMSG #a :guiwald!");
    assert_eq!(sounds(&h.effects()), vec![snd(&h, "alpirc-highlight.wav")], "un seul son, pas de bip en plus");
    // Son supprimé du dossier : le bip des alertes revient.
    std::fs::remove_file(h.dir.join("sounds/alpirc-highlight.wav")).unwrap();
    h.advance(2000);
    h.feed(":Pat!p@h PRIVMSG #a :guiwald!!");
    assert_eq!(sounds(&h.effects()), vec![Effect::Beep]);
    // Tout couper : ni son ni bip ; la notification reste.
    h.c.settings_mut().sounds.mute = true;
    h.advance(2000);
    h.feed(":Pat!p@h PRIVMSG Guiwald :psst");
    let fx = h.effects();
    assert!(sounds(&fx).is_empty(), "{fx:?}");
    assert!(fx.iter().any(|e| matches!(e, Effect::Notify { .. })));
}

#[test]
fn beep_choice_and_bursts() {
    let mut s = settings();
    s.sounds.channel_message.play = SoundPlay::Beep;
    let mut h = with_sounds(s);
    h.up();
    h.join("#a", "Guiwald Pat Bob");
    h.join("#b", "Guiwald");
    h.feed(":Pat!p@h PRIVMSG #a :un");
    h.feed(":Bob!b@h PRIVMSG #a :deux");
    assert_eq!(sounds(&h.effects()), vec![Effect::Beep], "rafale : un seul son");
    h.advance(1600);
    h.feed(":Pat!p@h PRIVMSG #a :\x01ACTION trois\x01");
    assert_eq!(sounds(&h.effects()), vec![Effect::Beep]);
    // Les avis ne sont pas des messages du canal.
    h.advance(1600);
    h.feed(":Pat!p@h NOTICE #a :avis");
    assert!(sounds(&h.effects()).is_empty());
}

#[test]
fn replayed_messages_stay_quiet() {
    let mut h = with_sounds(settings());
    h.up();
    h.join("#a", "Guiwald Pat");
    h.join("#b", "Guiwald");
    let old = alpirc_core::timeutil::format_iso8601(T0 - 3_600_000);
    h.feed(&format!("@time={old} :Pat!p@h PRIVMSG #a :guiwald, hier"));
    assert!(sounds(&h.effects()).is_empty());
    h.feed(":Pat!p@h PRIVMSG #a :guiwald, maintenant");
    assert_eq!(sounds(&h.effects()), vec![snd(&h, "alpirc-highlight.wav")]);
}

#[test]
fn connection_kick_invite_notify_sounds() {
    let mut s = settings();
    on(&mut s, |s| &mut s.sounds.connect);
    on(&mut s, |s| &mut s.sounds.disconnect);
    on(&mut s, |s| &mut s.sounds.kick);
    on(&mut s, |s| &mut s.sounds.invite);
    on(&mut s, |s| &mut s.sounds.notify);
    s.address_book.notify.push(NotifyEntry { nick: "Bob".into(), ..Default::default() });
    let mut h = with_sounds(s);
    h.connect();
    h.register_with("PREFIX=(ov)@+ CHANTYPES=# MONITOR=100");
    assert!(sounds(&h.effects()).contains(&snd(&h, "alpirc-connect.wav")));
    h.advance(2000);
    h.feed(":irc.test.net 730 Guiwald :Bob!b@h");
    assert_eq!(sounds(&h.effects()), vec![snd(&h, "alpirc-notify.wav")]);
    h.advance(2000);
    h.feed(":irc.test.net 731 Guiwald :Bob");
    assert_eq!(sounds(&h.effects()), vec![snd(&h, "alpirc-notify.wav")], "départ aussi");
    h.join("#a", "Guiwald @Pat");
    h.feed(":Pat!p@h KICK #a Bob :pas toi");
    assert!(sounds(&h.effects()).is_empty(), "l'expulsion d'un autre ne sonne pas");
    h.feed(":Pat!p@h KICK #a Guiwald :dehors");
    assert_eq!(sounds(&h.effects()), vec![snd(&h, "alpirc-kick.wav")]);
    h.feed(":Pat!p@h INVITE Guiwald #b");
    assert_eq!(sounds(&h.effects()), vec![snd(&h, "alpirc-invite.wav")]);
    // Déconnexion subie : son ; déconnexion demandée : rien.
    h.c.on_closed(h.conn, Some("Connection reset by peer"), h.now);
    assert_eq!(sounds(&h.effects()), vec![snd(&h, "alpirc-disconnect.wav")]);
    h.advance(20_000);
    h.c.tick(h.now);
    h.c.on_connected(h.conn, h.now);
    h.register();
    h.clear();
    h.advance(2000);
    h.cmd(h.status, "/quit bye");
    assert!(!sounds(&h.effects()).contains(&snd(&h, "alpirc-disconnect.wav")));
}

#[test]
fn dcc_offer_sound() {
    let mut s = settings();
    on(&mut s, |s| &mut s.sounds.dcc);
    let mut h = with_sounds(s);
    h.up();
    h.feed(":Pat!p@h PRIVMSG Guiwald :\x01DCC SEND photo.jpg 3232235777 5000 2048\x01");
    let fx = h.effects();
    assert!(fx.iter().any(|e| matches!(e, Effect::DccPrompt(_))));
    assert_eq!(sounds(&fx), vec![snd(&h, "alpirc-dcc.wav")]);
}

#[test]
fn script_haltdef_stops_the_sound() {
    let mut h = with_sounds(settings());
    h.c.engine_mut().load_text("t.mrc", ScriptKind::Remote, "on ^*:TEXT:*:#:{ haltdef }").unwrap();
    h.up();
    h.join("#a", "Guiwald Pat");
    h.join("#b", "Guiwald");
    h.feed(":Pat!p@h PRIVMSG #a :guiwald");
    assert!(sounds(&h.effects()).is_empty());
}

#[test]
fn ctcp_sound_requests() {
    let mut h = with_sounds(settings());
    h.up();
    let a = h.join("#a", "Guiwald Pat");
    // Refusées par défaut : on le dit, sans jouer.
    h.feed(":Pat!p@h PRIVMSG #a :\x01SOUND alpirc-ding.wav écoutez ça\x01");
    assert_eq!(h.lines(a)[h.lines(a).len() - 2..], ["* Pat écoutez ça".to_string(), "* Pat wants you to hear alpirc-ding.wav".to_string()]);
    assert!(sounds(&h.effects()).is_empty());
    // Acceptées : le fichier du dossier des sons joue.
    h.c.settings_mut().sounds.accept_requests = true;
    h.feed(":Pat!p@h PRIVMSG #a :\x01SOUND alpirc-ding.wav encore\x01");
    assert_eq!(h.last(a), "* Pat encore");
    assert_eq!(sounds(&h.effects()), vec![snd(&h, "alpirc-ding.wav")]);
    // Sans message.
    h.advance(2000);
    h.feed(":Pat!p@h PRIVMSG #a :\x01SOUND alpirc-ding.wav\x01");
    assert_eq!(h.last(a), "[Pat:#a SOUND] alpirc-ding.wav");
    assert_eq!(sounds(&h.effects()).len(), 1);
    // Jamais de chemin, jamais autre chose qu'un son, et seulement ce qui existe.
    for f in ["../alpirc-ding.wav", "/etc/passwd.wav", "sounds/alpirc-ding.wav", "alpirc.mrc", "absent.wav"] {
        h.advance(2000);
        h.feed(&format!(":Pat!p@h PRIVMSG #a :\x01SOUND {f}\x01"));
        assert_eq!(h.last(a), format!("* Pat wants you to hear {f}"));
        assert!(sounds(&h.effects()).is_empty(), "{f}");
    }
    // En privé : dans le privé s'il est ouvert, sinon dans la fenêtre active.
    h.advance(2000);
    h.feed(":Bob!b@h PRIVMSG Guiwald :\x01SOUND alpirc-ding.wav coucou\x01");
    assert_eq!(h.last(a), "* Bob coucou");
    assert!(!h.has_win("Bob"));
    assert_eq!(sounds(&h.effects()), vec![snd(&h, "alpirc-ding.wav")]);
    // Tout couper l'emporte.
    h.c.settings_mut().sounds.mute = true;
    h.advance(2000);
    h.feed(":Pat!p@h PRIVMSG #a :\x01SOUND alpirc-ding.wav\x01");
    assert_eq!(h.last(a), "* Pat wants you to hear alpirc-ding.wav");
    assert!(sounds(&h.effects()).is_empty());
}

#[test]
fn sound_command() {
    let mut h = with_sounds(settings());
    h.up();
    let a = h.join("#a", "Guiwald Pat");
    h.cmd(a, "/sound alpirc-ding.wav écoutez");
    assert_eq!(h.sends(), vec!["PRIVMSG #a :\x01SOUND alpirc-ding.wav écoutez\x01"]);
    assert_eq!(h.last(a), "* Guiwald écoutez");
    assert_eq!(sounds(&h.effects()), vec![snd(&h, "alpirc-ding.wav")], "joué chez nous aussi");
    h.cmd(a, "/sound Pat tada.wav");
    assert_eq!(h.sends(), vec!["PRIVMSG Pat :\x01SOUND tada.wav\x01"]);
    assert_eq!(h.last(a), "-> [Pat] SOUND tada.wav");
    assert!(sounds(&h.effects()).is_empty(), "absent chez nous : rien à jouer");
    h.cmd(a, "/sound #b /chemin/vers/bip.wav salut");
    assert_eq!(h.sends(), vec!["PRIVMSG #b :\x01SOUND bip.wav salut\x01"], "le nom seul");
    // Accepter ou refuser les demandes.
    h.cmd(a, "/sound on");
    assert!(h.c.settings().sounds.accept_requests);
    assert!(h.effects().contains(&Effect::SaveSettings));
    assert_eq!(h.last(a), "* Sound requests are accepted");
    h.cmd(a, "/sound off");
    assert!(!h.c.settings().sounds.accept_requests);
    // Sans fichier, ou hors d'un canal sans cible.
    h.cmd(a, "/sound");
    assert!(h.last(a).contains("sound"), "{}", h.last(a));
    h.cmd(h.status, "/sound alpirc-ding.wav");
    assert!(h.sends().is_empty());
}

#[test]
fn sound_command_with_echo_message() {
    let mut h = with_sounds(settings());
    h.up_with_caps("echo-message");
    let a = h.join("#a", "Guiwald Pat");
    let n = h.lines(a).len();
    h.cmd(a, "/sound alpirc-ding.wav hop");
    assert_eq!(h.lines(a).len(), n, "la ligne attend l'écho");
    h.feed(":Guiwald!alpirc@host.test PRIVMSG #a :\x01SOUND alpirc-ding.wav hop\x01");
    assert_eq!(h.last(a), "* Guiwald hop");
}

#[test]
fn test_button_plays_even_when_muted() {
    let mut s = settings();
    s.sounds.mute = true;
    let mut h = with_sounds(s);
    h.clear();
    assert!(h.c.test_sound(&EventSound::file("alpirc-notify.wav", true)));
    assert_eq!(sounds(&h.effects()), vec![snd(&h, "alpirc-notify.wav")]);
    assert!(h.c.test_sound(&EventSound { play: SoundPlay::Beep, file: String::new() }));
    assert_eq!(sounds(&h.effects()), vec![Effect::Beep]);
    assert!(!h.c.test_sound(&EventSound::file("absent.wav", true)));
    assert!(!h.c.test_sound(&EventSound::file("alpirc-notify.wav", false)));
    assert_eq!(h.c.sounds_dir(), PathBuf::from(&h.dir).join("sounds"));
}
