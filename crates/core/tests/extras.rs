//! Compléments du moteur : fenêtre Notify, commandes intégrées sans le moteur de scripts (`Client::builtin`,
//! `Client::raw`), resynchronisation silencieuse de la liste Notify, sorte des fichiers de scripts retenue au
//! redémarrage, avertissement au `/load`, `on KEYDOWN`/`KEYUP`, `/closemsg`, `/timestamp`, `/window`, `/ial`,
//! complétion des commandes des scripts, absence automatique, dates dans la langue de l'interface, scripts par
//! défaut traduits.

mod common;

use alpirc_core::settings::NotifyEntry;
use alpirc_core::{Client, Effect, WinKind, WinState};
use alpirc_script::ScriptKind;
use common::*;

fn chan() -> (H, alpirc_core::WinId) {
    let mut h = H::new();
    h.up();
    let w = h.join("#lyon", "Guiwald @Pat +Vic Bob");
    h.clear();
    (h, w)
}

#[test]
fn notify_window_is_created_once_and_titled() {
    let mut s = settings();
    s.language = Some("fr".into());
    s.address_book.notify.push(NotifyEntry { nick: "Bob".into(), note: "copain".into(), network: None });
    let mut h = H::with(s);
    h.up();
    h.input(h.status, "/notify");
    let notify = h.c.windows().iter().filter(|w| w.kind == WinKind::Notify);
    let wins: Vec<_> = notify.map(|w| (w.id, w.title.clone(), w.conn)).collect();
    assert_eq!(wins.len(), 1);
    assert_eq!(wins[0].1, "Liste Notify");
    assert_eq!(wins[0].2, None);
    assert!(h.effects().contains(&Effect::Activate(wins[0].0)));
    h.c.set_active(h.status, h.now);
    h.input(h.status, "/notify");
    assert_eq!(h.c.windows().iter().filter(|w| w.kind == WinKind::Notify).count(), 1);
    assert_eq!(h.c.active(), Some(wins[0].0));
    // `/window -c Notify` la ferme.
    h.input(h.status, "/window -c Notify");
    assert!(!h.c.windows().iter().any(|w| w.kind == WinKind::Notify));
}

#[test]
fn builtin_keeps_pipes_and_braces() {
    let (mut h, w) = chan();
    h.c.builtin(w, "topic", "#lyon a | b { c } $me", h.now).unwrap();
    assert_eq!(h.sends(), vec!["TOPIC #lyon :a | b { c } $me"]);
    h.c.builtin(w, "/msg", "#lyon x | y", h.now).unwrap();
    assert_eq!(h.sends(), vec!["PRIVMSG #lyon :x | y"]);
    assert_eq!(h.last(w), "<Guiwald> x | y");
    // Tapée, la même ligne est coupée par le moteur de scripts.
    h.input(w, "/msg #lyon x | echo -a coupé");
    assert_eq!(h.sends(), vec!["PRIVMSG #lyon :x"]);
    assert_eq!(h.last(w), "coupé");
    // Erreur : affichée et rendue.
    assert!(h.c.builtin(w, "kick", "", h.now).is_err());
    assert_eq!(h.last(w), "* /kick: insufficient parameters");
    // Ligne brute : telle quelle, sans écho.
    let n = h.lines(w).len();
    h.c.raw(h.conn, "PRIVMSG #lyon :brut | {", h.now);
    assert_eq!(h.sends(), vec!["PRIVMSG #lyon :brut | {"]);
    assert_eq!(h.lines(w).len(), n);
}

#[test]
fn notify_changed_resyncs_monitor_silently() {
    let mut s = settings();
    s.address_book.notify.push(NotifyEntry { nick: "Bob".into(), note: String::new(), network: None });
    let mut h = H::with(s);
    h.connect();
    h.register_with("PREFIX=(ov)@+ CHANTYPES=# MONITOR=100");
    h.feed(":irc.test.net 730 Guiwald :Bob!b@h");
    h.clear();
    let before = h.status_lines().len();
    let n = &mut h.c.settings_mut().address_book.notify;
    n.clear();
    n.push(NotifyEntry { nick: "Zed".into(), note: String::new(), network: None });
    n.push(NotifyEntry { nick: "Bob".into(), note: String::new(), network: None });
    h.c.notify_changed(h.now);
    assert_eq!(h.sends(), vec!["MONITOR C", "MONITOR + Zed,Bob"]);
    assert_eq!(h.status_lines().len(), before, "rien d'affiché");
    // Les réponses : Bob déjà en ligne ne s'annonce pas deux fois ; Zed, nouveau, oui.
    h.feed(":irc.test.net 730 Guiwald :Bob!b@h,Zed!z@h");
    let l = h.status_lines();
    assert_eq!(l.iter().filter(|x| x.as_str() == "* Bob is on IRC").count(), 1);
    assert!(l.iter().any(|x| x == "* Zed is on IRC"));
}

#[test]
fn notify_changed_with_ison() {
    let mut h = H::new();
    h.up();
    h.clear();
    h.c.settings_mut().address_book.notify.push(NotifyEntry { nick: "Ann".into(), note: String::new(), network: None });
    h.c.notify_changed(h.now);
    h.advance(10);
    assert!(h.sends().contains(&"ISON Ann".to_string()));
    assert!(!h.status_lines().iter().any(|l| l.contains("Added")));
}

#[test]
fn script_kinds_survive_a_restart() {
    let dir = tmpdir();
    {
        let mut h = H::with_dir(settings(), dir.clone());
        let scripts = dir.join("scripts");
        std::fs::write(scripts.join("mes-alias.txt"), "bonjour echo -a salut $1\n").unwrap();
        std::fs::write(scripts.join("menu.txt"), "Coucou:echo -a coucou\n").unwrap();
        h.input(h.status, "/load -a mes-alias.txt");
        h.input(h.status, "/load -pc menu.txt");
        h.c.save_scripts().unwrap();
        let order = std::fs::read_to_string(scripts.join("order.txt")).unwrap();
        assert!(order.lines().any(|l| l == "mes-alias.txt\taliases"), "{order}");
        assert!(order.lines().any(|l| l == "menu.txt\tpopups"), "{order}");
        assert!(order.lines().any(|l| l == "popups.mrc"), "les remotes restent sans sorte : {order}");
    }
    let mut c = Client::new(settings(), dir.clone(), T0);
    let kinds: Vec<(String, ScriptKind)> = c.engine().scripts().iter().map(|s| (s.name.clone(), s.kind)).collect();
    assert!(kinds.contains(&("mes-alias.txt".into(), ScriptKind::Aliases)), "{kinds:?}");
    assert!(kinds.contains(&("menu.txt".into(), ScriptKind::Popups)), "{kinds:?}");
    let st = c.windows()[0].id;
    c.input(st, "/bonjour toi", T0);
    assert_eq!(c.window(st).unwrap().lines.back().unwrap().text, "salut toi");
    // Ancien order.txt (sans sorte) : toujours lu, sorte devinée.
    std::fs::write(dir.join("scripts/order.txt"), "aliases.mrc\npopups.mrc\n").unwrap();
    let c = Client::new(settings(), dir, T0);
    assert_eq!(c.engine().scripts().len(), 2);
}

#[test]
fn load_asks_before_running_on_load() {
    let mut h = H::new();
    let scripts = h.dir.join("scripts");
    std::fs::write(scripts.join("auto.mrc"), "on *:LOAD:echo -s auto chargé\nalias autox echo -s x\n").unwrap();
    std::fs::write(scripts.join("calme.mrc"), "alias calme echo -s calme\n").unwrap();
    h.clear();
    h.input(h.status, "/load -rs auto.mrc");
    let fx = h.effects();
    let path = fx
        .iter()
        .find_map(|e| match e {
            Effect::ConfirmScriptLoad { name, path } if name == "auto.mrc" => Some(path.clone()),
            _ => None,
        })
        .expect("question attendue");
    assert!(path.ends_with("auto.mrc"));
    assert!(!h.c.engine().scripts().iter().any(|s| s.name == "auto.mrc"), "pas chargé avant la réponse");
    assert!(!h.has_line(h.status, "auto chargé"));
    // Oui : l'interface le charge et lance on LOAD.
    h.c.engine_mut().load_file(&path, true).unwrap();
    h.c.run_pending_scripts(h.now);
    assert!(h.has_line(h.status, "auto chargé"));
    // Sans on LOAD/START : aucune question.
    h.input(h.status, "/load -rs calme.mrc");
    assert!(!h.effects().iter().any(|e| matches!(e, Effect::ConfirmScriptLoad { .. })));
    assert!(h.c.engine().scripts().iter().any(|s| s.name == "calme.mrc"));
    // Avertissement désactivé : exécuté tout de suite.
    h.c.engine_mut().unload("auto.mrc");
    h.c.settings_mut().scripts.warn_on_load = false;
    h.input(h.status, "/load -rs auto.mrc");
    assert_eq!(h.status_lines().iter().filter(|l| l.as_str() == "auto chargé").count(), 2);
}

#[test]
fn keydown_and_keyup_events() {
    let mut h = H::new();
    h.c.load_script_text(
        "keys.mrc",
        ScriptKind::Remote,
        // Comme dans mIRC, seul le premier gestionnaire qui correspond, dans un script, réagit.
        "on ^*:KEYDOWN:@jeu:27:haltdef\n\
         on *:KEYDOWN:@jeu:*:echo -s down $keyval $keychar\n\
         on *:KEYUP:@jeu:*:echo -s up $keyval\n\
         on *:KEYDOWN:#:*:echo -s chan $keyval",
        h.now,
    )
    .unwrap();
    h.input(h.status, "/window @jeu");
    let w = h.c.find_window(None, "@jeu").unwrap();
    h.clear();
    assert!(!h.c.key_event(w, 65, Some('a'), false, h.now));
    assert!(h.has_line(h.status, "down 65 a"));
    h.c.key_event(w, 65, Some('a'), true, h.now);
    assert!(h.has_line(h.status, "up 65"));
    assert!(h.c.key_event(w, 27, None, false, h.now), "/haltdef : la touche n'est pas traitée");
    // Pas dans une autre sorte de fenêtre.
    let n = h.status_lines().len();
    h.c.key_event(h.status, 66, Some('b'), false, h.now);
    assert_eq!(h.status_lines().len(), n);
}

#[test]
fn closemsg_timestamp_ial() {
    let (mut h, w) = chan();
    h.input(w, "/query Pat");
    h.input(w, "/query Bob");
    assert!(h.has_win("Pat") && h.has_win("Bob"));
    h.input(w, "/closemsg Pat");
    assert!(!h.has_win("Pat") && h.has_win("Bob"));
    h.input(w, "/closemsg");
    assert!(!h.has_win("Bob"));
    h.clear();
    h.input(w, "/timestamp on");
    assert!(h.c.settings().irc.timestamps);
    assert!(h.effects().contains(&Effect::SaveSettings));
    assert!(h.last(w).ends_with("* Timestamping is on"));
    h.input(w, "/timestamp off");
    assert!(!h.c.settings().irc.timestamps);
    assert_eq!(h.last(w), "* Timestamping is off");
    h.input(w, "/timestamp");
    assert_eq!(h.last(w), "* Timestamping is off");
    h.input(w, "/ial on");
    assert_eq!(h.last(w), "* The internal address list (IAL) is always on in AlpIRC");
}

#[test]
fn window_switches_for_any_window() {
    let (mut h, w) = chan();
    h.input(w, "/query Pat");
    let q = h.win("Pat");
    h.clear();
    h.input(q, "/window -a #lyon");
    assert!(h.effects().contains(&Effect::Activate(w)));
    h.input(w, "/window -n #lyon");
    assert!(h.effects().contains(&Effect::WindowState { win: w, state: WinState::Minimize }));
    h.input(w, "/window -x Pat");
    assert!(h.effects().contains(&Effect::WindowState { win: q, state: WinState::Maximize }));
    h.input(w, "/window -r Status");
    assert!(h.effects().contains(&Effect::WindowState { win: h.status, state: WinState::Restore }));
    h.input(w, "/window -c Pat");
    assert!(!h.has_win("Pat"));
    h.input(w, "/window -a #nulle-part");
    assert_eq!(h.last(w), "* /window: no such window '#nulle-part'");
    // Fenêtres @ : création réduite.
    h.input(w, "/window -n @x");
    let x = h.c.find_window(None, "@x").unwrap();
    assert!(h.effects().contains(&Effect::WindowState { win: x, state: WinState::Minimize }));
}

#[test]
fn tab_completion_of_script_commands_and_aliases() {
    let (mut h, w) = chan();
    h.input(w, "/alias bonsoir echo -a bonsoir");
    let all: Vec<String> = (0..20).filter_map(|i| h.c.tab_complete(w, "/se", i)).collect();
    for c in ["/set", "/server", "/setname"] {
        assert!(all.iter().any(|a| a == c), "{c} : {all:?}");
    }
    assert_eq!(h.c.tab_complete(w, "/bons", 0).as_deref(), Some("/bonsoir"));
    assert_eq!(h.c.tab_complete(w, "/time", 0).as_deref(), Some("/time"));
    assert_eq!(h.c.tab_complete(w, "/time", 1).as_deref(), Some("/timer"));
    assert_eq!(h.c.tab_complete(w, "/time", 2).as_deref(), Some("/timers"));
    // Alias des scripts par défaut.
    assert_eq!(h.c.tab_complete(w, "/kb", 0).as_deref(), Some("/kb"));
}

#[test]
fn kb_default_alias() {
    let (mut h, w) = chan();
    h.feed(":irc.test.net 302 Guiwald :Pat=+p@pat.host");
    h.clear();
    h.input(w, "/kb Pat ouste");
    let s = h.sends();
    assert!(s.iter().any(|l| l.starts_with("USERHOST Pat") || l.starts_with("MODE #lyon +b")), "{s:?}");
}

#[test]
fn auto_away_after_idle_and_back_on_input() {
    let mut s = settings();
    s.away.auto_away_minutes = 5;
    s.away.default_message = "Parti marcher".into();
    let mut h = H::with(s);
    h.up();
    h.clear();
    let next = h.advance(1000).unwrap();
    assert!(next <= h.now + 5 * 60_000);
    // Le serveur parle (pas de délai de ping), l'utilisateur ne tape rien.
    let idle = |h: &mut H, mins: i64| -> Vec<String> {
        let mut away = Vec::new();
        for _ in 0..mins {
            h.feed(":irc.test.net NOTICE Guiwald :tic");
            h.advance(60_000);
            away.extend(h.sends().into_iter().filter(|l| l.starts_with("AWAY")));
        }
        away
    };
    assert!(idle(&mut h, 4).is_empty());
    assert_eq!(idle(&mut h, 1), vec!["AWAY :Parti marcher"]);
    h.feed(":irc.test.net 306 Guiwald :You have been marked as being away");
    assert!(idle(&mut h, 2).is_empty(), "une seule fois");
    h.input(h.status, "/echo -s me revoilà");
    assert_eq!(h.sends(), vec!["AWAY"]);
    // Absence choisie : la saisie suivante ne la retire pas.
    h.input(h.status, "/away réunion");
    h.feed(":irc.test.net 306 Guiwald :You have been marked as being away");
    h.clear();
    idle(&mut h, 10);
    h.input(h.status, "/echo -s toujours en réunion");
    assert!(!h.sends().contains(&"AWAY".to_string()));
}

#[test]
fn dates_follow_the_interface_language() {
    let mut s = settings();
    s.language = Some("fr".into());
    let mut h = H::with(s);
    h.up();
    let w = h.join("#lyon", "Guiwald Pat");
    // 17/10/2026 09:05:03 UTC (décalage imposé : 0).
    h.feed(":irc.test.net 333 Guiwald #lyon Pat!p@h 1792227903");
    assert_eq!(h.last(w), "* Posé par Pat!p@h le sam. 17 oct. 2026 09:05:03");
    // La réponse CTCP TIME reste en anglais.
    h.clear();
    h.feed(":Pat!p@h PRIVMSG Guiwald :\x01TIME\x01");
    let s = h.sends();
    assert!(s.iter().any(|l| l.starts_with("NOTICE Pat :\x01TIME ") && l.contains(" 2025")), "réponse à TIME : {s:?}");
}

#[test]
fn default_scripts_in_the_interface_language_and_never_overwritten() {
    let dir = tmpdir();
    let c = Client::new_localized(settings_lang(None), dir.clone(), T0, Some("fr_FR.UTF-8"));
    drop(c);
    let popups = std::fs::read_to_string(dir.join("scripts/popups.mrc")).unwrap();
    assert!(popups.contains("Rejoindre un canal:join $$?=\"Nom du canal :\""), "{popups}");
    let aliases = std::fs::read_to_string(dir.join("scripts/aliases.mrc")).unwrap();
    assert!(aliases.contains("alias kb ban -k # $$1 2 $2-"));
    // Fichier modifié par l'utilisateur : jamais réécrit, quelle que soit la langue.
    let mine = "menu status {\n  Mien:echo -a mien\n}\n";
    std::fs::write(dir.join("scripts/popups.mrc"), mine).unwrap();
    let _c = Client::new_localized(settings_lang(None), dir.clone(), T0, Some("en_US"));
    assert_eq!(std::fs::read_to_string(dir.join("scripts/popups.mrc")).unwrap(), mine);
    // Langue des réglages prioritaire sur celle du système.
    let d2 = tmpdir();
    let _c = Client::new_localized(settings_lang(Some("en")), d2.clone(), T0, Some("fr_FR"));
    assert!(std::fs::read_to_string(d2.join("scripts/popups.mrc")).unwrap().contains("Join channel:join"));
}

fn settings_lang(l: Option<&str>) -> alpirc_core::Settings {
    let mut s = settings();
    s.language = l.map(str::to_string);
    s
}
