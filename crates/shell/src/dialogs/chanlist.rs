//! « Liste des canaux » (Outils › Liste des canaux, bouton de la barre d'outils), comme la boîte de mIRC : le texte
//! cherché (dans les noms, et dans les sujets si on le veut), le nombre de personnes et l'ordre de la liste, avant
//! d'envoyer `/list`. Les choix sont gardés pour la fois suivante (pendant la session).

use alpirc_core::i18n::Lang;
use alpirc_ui::{DialogMsg, DialogSpec, DialogView};

use super::*;

/// Ce que demande la boîte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListQuery {
    /// Texte cherché (`*` et `?` permis ; vide : tous les canaux).
    pub text: String,
    /// Chercher aussi dans les sujets.
    pub topics: bool,
    pub min: Option<u32>,
    pub max: Option<u32>,
    /// Trier par nombre de personnes (du plus grand au plus petit) plutôt que par nom.
    pub by_users: bool,
}

impl Default for ListQuery {
    fn default() -> ListQuery {
        ListQuery { text: String::new(), topics: true, min: None, max: None, by_users: true }
    }
}

impl ListQuery {
    /// Motif envoyé au moteur : un mot sans joker est cherché partout dans le nom (ou le sujet) ; plusieurs mots
    /// deviennent `*mot1*mot2*` ; un motif avec `*` ou `?` est pris tel quel.
    pub fn pattern(&self) -> String {
        let words: Vec<&str> = self.text.split_whitespace().collect();
        if words.len() > 1 && !self.text.contains(['*', '?']) {
            format!("*{}*", words.join("*"))
        } else {
            words.join("*")
        }
    }
}

const OK: u32 = 1;
const CANCEL: u32 = 2;
const TEXT_L: u32 = 10;
const TEXT: u32 = 11;
const TOPICS: u32 = 12;
const USERS_L: u32 = 13;
const MIN_L: u32 = 14;
const MIN: u32 = 15;
const MAX_L: u32 = 16;
const MAX: u32 = 17;
const SORT_L: u32 = 18;
const BY_NAME: u32 = 19;
const BY_USERS: u32 = 20;
const NOTE: u32 = 21;
const GROUP: u32 = 22;

const W: i32 = 400;
const H: i32 = 262;

/// La boîte.
pub struct ListDialog {
    pub q: ListQuery,
}

impl ListDialog {
    pub fn new(q: ListQuery) -> ListDialog {
        ListDialog { q }
    }

    fn collect(&self, view: &DialogView) -> ListQuery {
        let n = |id| text_of(view, id).trim().parse::<u32>().ok();
        let text = text_of(view, TEXT).trim().to_string();
        ListQuery { text, topics: view.checked(TOPICS), min: n(MIN), max: n(MAX), by_users: view.checked(BY_USERS) }
    }
}

impl DialogCtl for ListDialog {
    fn kind(&self) -> Option<String> {
        Some("list-channels".into())
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let tr = |k: &str| t(lang, k);
        let (x0, x1) = (MARGIN, W - MARGIN);
        let fx = x0 + 150;
        let c = vec![
            group(GROUP, (x0, MARGIN, x1 - x0, 112), &tr("lc.filter")),
            field_label(TEXT_L, x0 + 12, 30, 136, &tr("lc.match")),
            edit(TEXT, (fx, 30, x1 - 12 - fx, EDIT_H), "").style("limit 200"),
            check(TOPICS, (fx, 58, x1 - 12 - fx, CHECK_H), &tr("lc.topics")),
            field_label(USERS_L, x0 + 12, 84, 136, &tr("lc.users")),
            field_label(MIN_L, fx, 84, 30, &tr("lc.min")),
            edit(MIN, (fx + 32, 84, 56, EDIT_H), "").style("limit 6"),
            field_label(MAX_L, fx + 100, 84, 34, &tr("lc.max")),
            edit(MAX, (fx + 136, 84, 56, EDIT_H), "").style("limit 6"),
            text(SORT_L, (x0 + 12, 132, 136, LABEL_H), &tr("lc.sort")),
            radio(BY_NAME, (fx, 130, x1 - fx, CHECK_H), &tr("lc.by_name")).style("group"),
            radio(BY_USERS, (fx, 150, x1 - fx, CHECK_H), &tr("lc.by_users")),
            text(NOTE, (x0 + 4, 178, x1 - x0 - 8, 32), &tr("lc.note")),
            button(OK, (x1 - 2 * BTN_W - 6 - 30, H - MARGIN - BTN_H, BTN_W + 30, BTN_H), &tr("lc.list")).style("default ok"),
            button(CANCEL, (x1 - BTN_W, H - MARGIN - BTN_H, BTN_W, BTN_H), &tr("dlg.cancel")).style("cancel"),
        ];
        spec(&tr("lc.title"), W, H, c)
    }

    fn init(&mut self, view: &mut DialogView, _cx: &mut Cx) {
        view.set_text(TEXT, &self.q.text);
        view.set_checked(TOPICS, self.q.topics);
        view.set_text(MIN, &self.q.min.map(|n| n.to_string()).unwrap_or_default());
        view.set_text(MAX, &self.q.max.map(|n| n.to_string()).unwrap_or_default());
        view.set_checked(BY_NAME, !self.q.by_users);
        view.set_checked(BY_USERS, self.q.by_users);
        view.set_focus(TEXT);
    }

    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, cx: &mut Cx) -> Outcome {
        match msg {
            DialogMsg::Ok(_) => {
                self.q = self.collect(view);
                cx.actions.push(Action::ListChannels(self.q.clone()));
                Outcome::Close
            }
            DialogMsg::Cancel => Outcome::Close,
            _ => Outcome::Keep,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testkit::{Kit, assert_translated};

    #[test]
    fn remembers_and_returns_the_query() {
        let mut k = Kit::new("chanlist");
        let q0 = ListQuery { text: "lyon".into(), topics: false, min: Some(5), max: None, by_users: false };
        let mut d = ListDialog::new(q0.clone());
        assert_translated(&d);
        let (mut v, _) = k.open(&mut d);
        assert_eq!(d.collect(&v), q0, "champs remplis avec la demande précédente");
        v.set_text(TEXT, " rust  lang ");
        v.set_checked(TOPICS, true);
        v.set_text(MAX, "200");
        v.set_text(MIN, "x");
        v.set_checked(BY_NAME, false);
        v.set_checked(BY_USERS, true);
        let (o, a) = k.send(&mut d, &mut v, DialogMsg::Ok(OK));
        assert_eq!(o, Outcome::Close);
        let Some(Action::ListChannels(q)) = a.into_iter().next() else { panic!("pas de demande") };
        assert_eq!(q, ListQuery { text: "rust  lang".into(), topics: true, min: None, max: Some(200), by_users: true });
        assert_eq!(q.pattern(), "*rust*lang*");
        let one = ListQuery { text: "#rust*".into(), ..Default::default() };
        assert_eq!(one.pattern(), "#rust*");
    }
}
