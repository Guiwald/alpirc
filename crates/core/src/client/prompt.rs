//! Questions des scripts à l'utilisateur : `$?` (`$$?="Canal :"` des menus par défaut) et `$input(…)`.
//!
//! Le moteur de scripts ne sait pas attendre. Quand une question n'a pas encore de réponse, l'exécution s'arrête
//! sans bruit et le client demande à l'interface de la poser ([`super::Effect::AskInput`]) ; à la réponse
//! ([`super::Client::answer_input`]), ce qui avait été lancé (ligne tapée, commande d'un bouton, entrée de menu)
//! est relancé, et les réponses déjà données sont rendues dans l'ordre aux questions rencontrées. Une ligne qui pose
//! deux questions est donc relancée deux fois ; ce qui précède la première question dans la même ligne est refait.
//! Dans un événement ou un timer (rien à relancer), la réponse est vide, comme une question annulée.

use std::collections::VecDeque;

use alpirc_script::{InputRequest, PopupToken};

use super::WinId;

/// Ce qui est relancé après une réponse.
#[derive(Debug, Clone)]
pub(crate) enum Replay {
    /// Ligne de commande (`/join $$?`) tapée ou lancée par un bouton.
    Line { win: WinId, line: String },
    /// Entrée d'un menu de script.
    Popup { win: WinId, selected: Vec<String>, token: PopupToken },
}

/// Boutons d'une question.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputButtons {
    /// OK et Annuler (champ de saisie).
    OkCancel,
    /// OK seul.
    Ok,
    /// Oui et Non.
    YesNo,
    /// Oui, Non et Annuler.
    YesNoCancel,
}

/// Réponse de l'utilisateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputAnswer {
    /// OK, avec le texte du champ (vide sans champ).
    Ok(String),
    Yes,
    No,
    Cancel,
}

/// Question à poser par l'interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputPrompt {
    pub id: u64,
    /// Fenêtre d'où vient la question.
    pub win: Option<WinId>,
    pub prompt: String,
    /// Drapeaux de `$input` (voir [`alpirc_script::InputRequest`]).
    pub flags: String,
    /// Titre de la boîte (vide : celui de l'application).
    pub title: String,
    /// Texte de départ du champ.
    pub default: String,
}

impl InputPrompt {
    /// La question a un champ de saisie (`e`, `p`, ou aucun bouton demandé).
    pub fn has_edit(&self) -> bool {
        self.flags.contains(['e', 'p']) || !self.flags.contains(['o', 'y', 'n'])
    }

    /// Champ masqué (mot de passe).
    pub fn password(&self) -> bool {
        self.flags.contains('p')
    }

    pub fn buttons(&self) -> InputButtons {
        if self.flags.contains('n') {
            InputButtons::YesNoCancel
        } else if self.flags.contains('y') {
            InputButtons::YesNo
        } else if self.flags.contains('o') && !self.has_edit() {
            InputButtons::Ok
        } else {
            InputButtons::OkCancel
        }
    }

    /// Icône demandée : `i` information, `q` question, `w` avertissement, `h` erreur.
    pub fn icon(&self) -> Option<char> {
        self.flags.chars().find(|c| matches!(c, 'i' | 'q' | 'w' | 'h'))
    }

    /// Ce que rendent `$?` ou `$input(…)` pour cette réponse, et le texte gardé pour `$!`.
    pub fn value(&self, answer: &InputAnswer) -> (String, String) {
        let v = self.flags.contains('v');
        let text = match answer {
            InputAnswer::Ok(t) => t.clone(),
            _ => String::new(),
        };
        let value = match answer {
            InputAnswer::Ok(t) if self.has_edit() && !v => t.clone(),
            InputAnswer::Ok(_) => if v { "$ok" } else { "$true" }.to_string(),
            InputAnswer::Yes => if v { "$yes" } else { "$true" }.to_string(),
            InputAnswer::No => if v { "$no" } else { "$false" }.to_string(),
            InputAnswer::Cancel if v => "$cancel".to_string(),
            InputAnswer::Cancel => String::new(),
        };
        let shown = if self.has_edit() { text } else { value.clone() };
        (value, shown)
    }
}

/// Une question posée, en attente de sa réponse.
#[derive(Debug, Clone)]
struct Pending {
    id: u64,
    replay: Replay,
    /// Réponses déjà données avant cette question, pendant ce passage.
    used: Vec<String>,
}

/// L'état des questions.
#[derive(Debug, Default)]
pub(crate) struct InputState {
    /// Ce qui s'exécute en ce moment et pourrait être relancé.
    current: Option<Replay>,
    /// Réponses à rendre, dans l'ordre, pendant ce passage.
    pub(crate) answers: VecDeque<String>,
    used: Vec<String>,
    pending: Option<Pending>,
    next_id: u64,
    /// Texte de la dernière réponse (`$!`).
    pub(crate) last: String,
}

impl InputState {
    /// Début d'une exécution relançable ; vrai si c'est la plus externe (qui devra appeler [`InputState::end`]).
    pub(crate) fn begin(&mut self, r: Replay) -> bool {
        if self.current.is_some() {
            return false;
        }
        self.current = Some(r);
        self.used.clear();
        true
    }

    /// Fin de l'exécution ouverte par [`InputState::begin`] : les réponses qui restent sont oubliées.
    pub(crate) fn end(&mut self, owner: bool) {
        if owner {
            self.current = None;
            self.answers.clear();
            self.used.clear();
        }
    }

    /// Une question : la réponse si elle est connue (`Ok`), sinon la question à poser (`Err(Some)`) ou rien à faire
    /// (`Err(None)` : une question attend déjà sa réponse).
    pub(crate) fn ask(&mut self, req: &InputRequest, win: Option<WinId>) -> Result<String, Option<InputPrompt>> {
        if let Some(a) = self.answers.pop_front() {
            self.used.push(a.clone());
            return Ok(a);
        }
        let Some(replay) = self.current.clone() else {
            // Rien à relancer (événement, timer) : comme une question annulée.
            return Ok(String::new());
        };
        if self.pending.is_some() {
            return Err(None);
        }
        self.next_id += 1;
        let id = self.next_id;
        self.pending = Some(Pending { id, replay, used: self.used.clone() });
        Err(Some(InputPrompt {
            id,
            win,
            prompt: req.prompt.clone(),
            flags: if req.flags.is_empty() { "e".into() } else { req.flags.clone() },
            title: req.title.clone(),
            default: req.default.clone(),
        }))
    }

    /// La réponse à la question `id` : ce qu'il faut relancer et les réponses à rendre. `None` si cette question
    /// n'attend plus (déjà répondue).
    pub(crate) fn answer(&mut self, id: u64, value: String, shown: String) -> Option<(Replay, VecDeque<String>)> {
        if self.pending.as_ref().is_none_or(|p| p.id != id) {
            return None;
        }
        let p = self.pending.take()?;
        self.last = shown;
        let mut answers: VecDeque<String> = p.used.into();
        answers.push_back(value);
        Some((p.replay, answers))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt(flags: &str) -> InputPrompt {
        InputPrompt { id: 1, win: None, prompt: String::new(), flags: flags.into(), title: String::new(), default: String::new() }
    }

    #[test]
    fn values_follow_mirc() {
        assert_eq!(prompt("e").value(&InputAnswer::Ok("#lyon".into())).0, "#lyon");
        assert_eq!(prompt("e").value(&InputAnswer::Cancel).0, "");
        assert_eq!(prompt("ev").value(&InputAnswer::Ok("x".into())), ("$ok".into(), "x".into()));
        assert_eq!(prompt("y").value(&InputAnswer::Yes).0, "$true");
        assert_eq!(prompt("yv").value(&InputAnswer::No).0, "$no");
        assert_eq!(prompt("nv").value(&InputAnswer::Cancel).0, "$cancel");
        assert_eq!(prompt("o").value(&InputAnswer::Ok(String::new())).0, "$true");
        assert_eq!(prompt("o").buttons(), InputButtons::Ok);
        assert_eq!(prompt("eo").buttons(), InputButtons::OkCancel);
        assert_eq!(prompt("qn").buttons(), InputButtons::YesNoCancel);
        assert!(prompt("ep").password() && prompt("").has_edit() && !prompt("y").has_edit());
        assert_eq!(prompt("wy").icon(), Some('w'));
    }

    #[test]
    fn ask_then_answer_replays_with_previous_answers() {
        let mut s = InputState::default();
        let req = InputRequest { prompt: "Canal :".into(), flags: "e".into(), ..Default::default() };
        // Sans exécution relançable : réponse vide.
        assert_eq!(s.ask(&req, None), Ok(String::new()));
        let owner = s.begin(Replay::Line { win: 1, line: "/join $$?".into() });
        let Err(Some(p)) = s.ask(&req, Some(1)) else { panic!() };
        assert_eq!(s.ask(&req, Some(1)), Err(None), "une seule question à la fois");
        s.end(owner);
        let (r, answers) = s.answer(p.id, "#lyon".into(), "#lyon".into()).unwrap();
        assert!(matches!(r, Replay::Line { win: 1, .. }));
        assert_eq!(answers, VecDeque::from(vec!["#lyon".to_string()]));
        assert_eq!(s.last, "#lyon");
        assert!(s.answer(p.id, "x".into(), "x".into()).is_none(), "déjà répondue");
    }
}
