//! Question d'un script (`$?`, `$input(…)`), comme la boîte « Input Request » de mIRC : le texte de la question,
//! un champ de saisie (masqué pour un mot de passe) et les boutons demandés (OK et Annuler, OK, Oui et Non, Oui,
//! Non et Annuler). La réponse va au moteur, qui relance la commande qui avait posé la question.

use alpirc_core::client::prompt::{InputAnswer, InputButtons, InputPrompt};
use alpirc_core::i18n::Lang;
use alpirc_ui::{ControlKind, ControlSpec, DialogMsg, DialogSpec, DialogView};

use super::*;

const W: i32 = 380;
const EDIT: u32 = 2;
/// Boutons : premier, deuxième, troisième (de gauche à droite).
const B1: u32 = 3;
const B2: u32 = 4;
const B3: u32 = 5;

/// La boîte de question.
pub struct InputDialog {
    pub q: InputPrompt,
}

impl InputDialog {
    pub fn new(q: InputPrompt) -> InputDialog {
        InputDialog { q }
    }

    /// Boutons affichés, de gauche à droite : (identifiant, clé du texte, réponse).
    fn buttons(&self) -> Vec<(u32, &'static str, InputAnswer)> {
        let ok = (B1, "dlg.ok", InputAnswer::Ok(String::new()));
        let (yes, no) = ((B1, "dlg.yes", InputAnswer::Yes), (B2, "dlg.no", InputAnswer::No));
        match self.q.buttons() {
            InputButtons::OkCancel => vec![ok, (B2, "dlg.cancel", InputAnswer::Cancel)],
            InputButtons::Ok => vec![ok],
            InputButtons::YesNo => vec![yes, no],
            InputButtons::YesNoCancel => vec![yes, no, (B3, "dlg.cancel", InputAnswer::Cancel)],
        }
    }

    /// Réponse d'Échap ou de la case de fermeture : Annuler, ou Non sans bouton Annuler.
    fn escape(&self) -> InputAnswer {
        match self.q.buttons() {
            InputButtons::YesNo => InputAnswer::No,
            InputButtons::Ok => InputAnswer::Ok(String::new()),
            _ => InputAnswer::Cancel,
        }
    }

    fn answer(&self, a: InputAnswer, view: &DialogView, cx: &mut Cx) {
        let a = match a {
            InputAnswer::Ok(_) => InputAnswer::Ok(if self.q.has_edit() { text_of(view, EDIT) } else { String::new() }),
            other => other,
        };
        cx.client.answer_input(&self.q, &a, cx.now);
    }
}

impl DialogCtl for InputDialog {
    fn kind(&self) -> Option<String> {
        Some(format!("input:{}", self.q.id))
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let title = if self.q.title.trim().is_empty() { t(lang, "in.title") } else { self.q.title.clone() };
        let icon = self.q.icon().map(|c| match c {
            'i' => "ui/msg_info_32",
            'w' => "ui/msg_warning_32",
            'h' => "ui/msg_error_32",
            _ => "ui/msg_question_32",
        });
        let tx = if icon.is_some() { MARGIN + 4 + 32 + 12 } else { MARGIN + 4 };
        let tw = W - tx - MARGIN - 4;
        let prompt = if self.q.prompt.is_empty() { t(lang, "in.default_prompt") } else { self.q.prompt.clone() };
        let th = text_height(&prompt, tw);
        let mut c: Vec<ControlSpec> = Vec::new();
        if let Some(name) = icon {
            let mut i = ControlSpec::new(9, ControlKind::Icon, (MARGIN + 4, MARGIN + 4, 32, 32), name);
            i.styles.push("noborder".into());
            c.push(i);
        }
        let mut y = MARGIN + 4;
        c.push(text(1, (tx, y, tw, th), &prompt));
        y += th.max(if icon.is_some() { 32 } else { 0 }) + 8;
        if self.q.has_edit() {
            let e = edit(EDIT, (MARGIN + 4, y, W - 2 * MARGIN - 8, EDIT_H), &self.q.default).style("limit 900");
            c.push(if self.q.password() { e.style("password") } else { e });
            y += EDIT_H + 12;
        } else {
            y += 4;
        }
        let bs = self.buttons();
        let n = bs.len() as i32;
        let mut x = W - MARGIN - 4 - n * BTN_W - (n - 1) * 6;
        for (k, (id, key, _)) in bs.iter().enumerate() {
            let mut b = button(*id, (x, y, BTN_W, BTN_H), &t(lang, key));
            if k == 0 {
                b = b.style("default");
            }
            c.push(b);
            x += BTN_W + 6;
        }
        spec(&title, W, y + BTN_H + MARGIN + 2, c)
    }

    fn init(&mut self, view: &mut DialogView, _cx: &mut Cx) {
        if self.q.has_edit() {
            view.set_focus(EDIT);
        } else {
            view.set_focus(B1);
        }
    }

    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, cx: &mut Cx) -> Outcome {
        match msg {
            DialogMsg::Clicked(id) => match self.buttons().into_iter().find(|b| b.0 == id) {
                Some((_, _, a)) => {
                    self.answer(a, view, cx);
                    Outcome::Close
                }
                None => Outcome::Keep,
            },
            DialogMsg::Ok(_) => {
                let a = self.buttons().into_iter().next().map(|b| b.2).unwrap_or(InputAnswer::Cancel);
                self.answer(a, view, cx);
                Outcome::Close
            }
            DialogMsg::Cancel => {
                self.answer(self.escape(), view, cx);
                Outcome::Close
            }
            _ => Outcome::Keep,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testkit::{Kit, assert_translated};
    use alpirc_core::Effect;

    fn asked(k: &mut Kit) -> Vec<InputPrompt> {
        let effects = k.client.take_effects();
        effects.into_iter().filter_map(|e| if let Effect::AskInput(q) = e { Some(q) } else { None }).collect()
    }

    #[test]
    fn join_channel_through_the_box() {
        let mut k = Kit::new("input");
        let (_conn, st) = k.connect();
        k.client.command(st, "//join $$?=\"Enter channel name:\"", k.now);
        let q = asked(&mut k);
        assert_eq!(q.len(), 1);
        let mut d = InputDialog::new(q[0].clone());
        assert_translated(&d);
        let (mut v, _) = k.open(&mut d);
        assert!(v.visible(EDIT) && v.focus() == Some(EDIT));
        v.set_text(EDIT, "#lane-b");
        let (o, _) = k.send(&mut d, &mut v, DialogMsg::Clicked(B1));
        assert_eq!(o, Outcome::Close);
        assert!(k.sent().contains(&"JOIN #lane-b".to_string()));
    }

    #[test]
    fn yes_no_buttons_and_escape() {
        let mut k = Kit::new("input2");
        let (_conn, st) = k.connect();
        k.client.command(st, "//echo -a r: $input(Vraiment ?,qy,-1,Titre)", k.now);
        let q = asked(&mut k);
        let mut d = InputDialog::new(q[0].clone());
        assert_eq!(d.spec(Lang::En).title, "Titre");
        let (mut v, _) = k.open(&mut d);
        assert!(v.control_rect(EDIT).is_none(), "pas de champ");
        // Échap sur Oui/Non : Non.
        k.send(&mut d, &mut v, DialogMsg::Cancel);
        assert!(k.client.window(st).unwrap().lines.iter().any(|l| l.text == "r: $false"));
    }
}
