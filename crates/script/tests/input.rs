//! `$?` et `$input(…)` : l'hôte répond tout de suite, ou demande d'attendre (arrêt silencieux) puis la commande
//! est relancée avec la réponse ; entrées de menu gardées pour être relancées.

mod common;

use std::collections::VecDeque;

use alpirc_script::{Ctx, Engine, Host, InputRequest, PopupKind};
use common::{FakeHost, tmpdir};

/// Hôte qui rend les réponses données d'avance, puis demande d'attendre en notant la question.
struct AskHost {
    inner: FakeHost,
    answers: VecDeque<String>,
    asked: Vec<InputRequest>,
}

impl AskHost {
    fn new() -> AskHost {
        AskHost { inner: FakeHost::new(), answers: VecDeque::new(), asked: Vec::new() }
    }
}

impl Host for AskHost {
    fn command(&mut self, ctx: &Ctx, name: &str, args: &str) -> Result<(), String> {
        self.inner.command(ctx, name, args)
    }
    fn identifier(&mut self, ctx: &Ctx, name: &str, args: &[String], prop: Option<&str>) -> Option<String> {
        self.inner.identifier(ctx, name, args, prop)
    }
    fn member_modes(&mut self, ctx: &Ctx, chan: &str, nick: &str) -> Option<String> {
        self.inner.member_modes(ctx, chan, nick)
    }
    fn is_channel(&mut self, ctx: &Ctx, name: &str) -> bool {
        self.inner.is_channel(ctx, name)
    }
    fn script_error(&mut self, ctx: &Ctx, text: &str) {
        self.inner.script_error(ctx, text)
    }
    fn now_ms(&self) -> i64 {
        self.inner.now_ms()
    }
    fn input(&mut self, _ctx: &Ctx, req: &InputRequest) -> Option<String> {
        match self.answers.pop_front() {
            Some(a) => Some(a),
            None => {
                self.asked.push(req.clone());
                None
            }
        }
    }
}

#[test]
fn default_host_answers_with_its_identifier() {
    let mut e = Engine::new(tmpdir());
    let mut h = FakeHost::new();
    e.run_line(&mut h, &Ctx::default(), "//echo -a $?=\"Nom ?\"").unwrap();
    assert_eq!(h.says(), vec!["answer"]);
}

#[test]
fn waiting_halts_silently_then_replays_with_answers() {
    let mut e = Engine::new(tmpdir());
    let mut h = AskHost::new();
    let line = "//echo -a avant | kick #c Bob $$?=\"Raison :\" | echo -a $?*=\"Mot de passe\" $?!=\"Sûr ?\"";
    e.run_line(&mut h, &Ctx::default(), line).unwrap();
    // Le premier `echo` passe, puis tout s'arrête sur la question, sans erreur.
    assert_eq!(h.inner.says(), vec!["avant"]);
    assert!(h.inner.errors.is_empty());
    assert_eq!(h.asked.len(), 1);
    assert_eq!((h.asked[0].prompt.as_str(), h.asked[0].flags.as_str()), ("Raison :", "e"));
    // Relance avec la première réponse : la question suivante (mot de passe).
    h.inner.clear();
    h.answers = VecDeque::from(vec!["flood".to_string()]);
    e.run_line(&mut h, &Ctx::default(), line).unwrap();
    assert_eq!(h.inner.find("kick"), vec!["#c Bob flood"]);
    assert_eq!(h.asked.last().map(|r| r.flags.as_str()), Some("ep"));
    // Toutes les réponses : la ligne entière.
    h.inner.clear();
    h.answers = VecDeque::from(vec!["flood".to_string(), "secret".to_string(), "$true".to_string()]);
    e.run_line(&mut h, &Ctx::default(), line).unwrap();
    assert_eq!(h.inner.says(), vec!["avant", "secret $true"]);
    // `$$?` vide : arrêt, comme mIRC (Annuler).
    h.inner.clear();
    h.answers = VecDeque::from(vec![String::new()]);
    e.run_line(&mut h, &Ctx::default(), line).unwrap();
    assert!(h.inner.find("kick").is_empty());
}

#[test]
fn input_identifier_passes_its_parameters() {
    let mut e = Engine::new(tmpdir());
    let mut h = AskHost::new();
    e.run_line(&mut h, &Ctx::default(), "//echo -a $input(Ton pseudo ?,ep,-1,Premier essai,Guiwald)").unwrap();
    assert_eq!(
        h.asked,
        vec![InputRequest { prompt: "Ton pseudo ?".into(), flags: "ep".into(), title: "Premier essai".into(), default: "Guiwald".into() }]
    );
    h.answers = VecDeque::from(vec!["Elisa".to_string()]);
    e.run_line(&mut h, &Ctx::default(), "//echo -a $input(Ton pseudo ?,ep,-1,Premier essai,Guiwald)").unwrap();
    assert_eq!(h.inner.says(), vec!["Elisa"]);
}

#[test]
fn popup_entries_can_be_replayed_later() {
    let mut e = Engine::new(tmpdir());
    e.load_text("p.mrc", alpirc_script::ScriptKind::Remote, "menu channel {\n  Rejoindre:join $$?=\"Canal :\"\n}\n").unwrap();
    let mut h = AskHost::new();
    let ctx = Ctx::default();
    let menu = e.popup(&mut h, &ctx, PopupKind::Channel, None);
    let action = menu.iter().find(|m| m.label.contains("Rejoindre")).and_then(|m| m.action).unwrap();
    let token = e.popup_token(action).unwrap();
    e.popup_run(&mut h, &ctx, action).unwrap();
    assert_eq!(h.asked.len(), 1);
    assert!(h.inner.find("join").is_empty());
    // Un autre menu construit entre-temps ne change rien à l'entrée gardée.
    let _ = e.popup(&mut h, &ctx, PopupKind::Status, None);
    h.answers = VecDeque::from(vec!["#lyon".to_string()]);
    e.popup_replay(&mut h, &ctx, &token).unwrap();
    assert_eq!(h.inner.find("join"), vec!["#lyon"]);
}
