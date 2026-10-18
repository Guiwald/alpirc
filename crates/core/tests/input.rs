//! Questions des scripts (`$?`, `$input`) : les menus par défaut « Rejoindre un canal » et « Expulser (raison) »
//! posent leur question, puis la commande part avec la réponse ; Annuler avec `$$?` n'envoie rien.

mod common;

use alpirc_core::Effect;
use alpirc_core::client::prompt::{InputAnswer, InputPrompt};
use alpirc_script::{MenuItem, PopupKind};
use common::H;

/// Numéro d'action d'une entrée de menu, par son libellé (dans les sous-menus aussi).
fn action(items: &[MenuItem], label: &str) -> Option<u32> {
    items.iter().find_map(|m| if m.label.replace('&', "") == label { m.action } else { action(&m.children, label) })
}

fn asked(h: &mut H) -> Vec<InputPrompt> {
    h.effects().into_iter().filter_map(|e| if let Effect::AskInput(q) = e { Some(q) } else { None }).collect()
}

#[test]
fn join_channel_popup_asks_then_joins() {
    let mut h = H::new();
    h.up();
    let st = h.status;
    let menu = h.c.popup(PopupKind::Status, st, &[]);
    let a = action(&menu, "Join channel").expect("entrée « Join channel »");
    h.clear();
    h.c.popup_run(st, &[], a, h.now);
    let q = asked(&mut h);
    assert_eq!(q.len(), 1);
    assert_eq!(q[0].prompt, "Enter channel name:");
    assert!(q[0].has_edit() && !q[0].password());
    assert!(h.sends().iter().all(|l| !l.starts_with("JOIN")));
    // Un autre menu entre-temps (barre de menus) ne change pas l'entrée relancée.
    let _ = h.c.popup(PopupKind::Menubar, st, &[]);
    h.c.answer_input(&q[0], &InputAnswer::Ok("#lyon".into()), h.now);
    assert!(h.sends().contains(&"JOIN #lyon".to_string()));
    // La même réponse une seconde fois : ignorée.
    h.c.answer_input(&q[0], &InputAnswer::Ok("#autre".into()), h.now);
    assert!(h.sends().is_empty());
    // `$!` : le texte de la dernière réponse.
    h.cmd(st, "//echo -a dernier: $!");
    assert!(h.has_line(st, "dernier: #lyon"));
}

#[test]
fn kick_why_from_the_nicklist_and_cancel() {
    let mut h = H::new();
    h.up();
    let chan = h.join("#lane-b", "@Guiwald Troll");
    let sel = vec!["Troll".to_string()];
    let menu = h.c.popup(PopupKind::Nicklist, chan, &sel);
    let a = action(&menu, "Kick (why)").expect("entrée « Kick (why) »");
    h.clear();
    h.c.popup_run(chan, &sel, a, h.now);
    let q = asked(&mut h);
    assert_eq!(q.len(), 1);
    h.c.answer_input(&q[0], &InputAnswer::Ok("flood".into()), h.now);
    let sent = h.sends();
    assert!(sent.contains(&"KICK #lane-b Troll flood".to_string()), "{sent:?}");
    // Annuler : `$$?` vide arrête tout.
    let a = action(&h.c.popup(PopupKind::Nicklist, chan, &sel), "Kick (why)").unwrap();
    h.c.popup_run(chan, &sel, a, h.now);
    let q = asked(&mut h);
    h.c.answer_input(&q[0], &InputAnswer::Cancel, h.now);
    assert!(h.sends().iter().all(|l| !l.starts_with("KICK")));
}

#[test]
fn input_identifier_with_buttons_and_two_questions() {
    let mut h = H::new();
    h.up();
    let st = h.status;
    let line = "//echo -a r: $input(Sûr ?,yv,-1,Question) $?*=\"Mot de passe\"";
    h.cmd(st, line);
    let q = asked(&mut h);
    assert_eq!((q[0].prompt.as_str(), q[0].title.as_str()), ("Sûr ?", "Question"));
    assert!(!q[0].has_edit());
    h.c.answer_input(&q[0], &InputAnswer::No, h.now);
    // Seconde question (mot de passe), puis la ligne entière avec les deux réponses.
    let q2 = asked(&mut h);
    assert_eq!(q2.len(), 1);
    assert!(q2[0].password());
    h.c.answer_input(&q2[0], &InputAnswer::Ok("secret".into()), h.now);
    assert!(h.has_line(st, "r: $no secret"), "{:?}", h.status_lines());
    // Dans un événement (rien à relancer) : réponse vide, sans question.
    h.c.load_script_text("t.mrc", alpirc_script::ScriptKind::Remote, "on *:TEXT:pose*:*:{ echo -a ev: [ $?=\"x\" ] fin }", h.now).unwrap();
    h.clear();
    h.feed(":Bob!b@h PRIVMSG Guiwald :pose");
    assert!(asked(&mut h).is_empty());
}
