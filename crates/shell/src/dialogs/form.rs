//! Petite boîte de saisie commune : une étiquette et un champ par ligne, OK et Annuler. Elle sert à ajouter ou
//! modifier une entrée d'une liste (pseudo de la liste Notify, masque ignoré, favori, mot surligné…) ; la boîte
//! appelante fournit la validation, une fermeture qui reçoit les valeurs et écrit dans son état partagé.

use alpirc_core::i18n::Lang;
use alpirc_ui::{ControlSpec, DialogMsg, DialogSpec, DialogView};

use super::*;

/// Sorte d'un champ.
#[derive(Clone, Debug, PartialEq)]
pub enum FieldKind {
    Text,
    Password,
    /// Nombre entier (vérifié à la validation par la fermeture).
    Number,
    Check,
    /// Liste déroulante non modifiable.
    Choice(Vec<String>),
    /// Liste déroulante modifiable (propositions).
    Combo(Vec<String>),
}

/// Valeur d'un champ.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Val {
    Text(String),
    Bool(bool),
    Index(usize),
}

impl Val {
    /// Texte (vide pour une case ou une liste).
    pub fn text(&self) -> String {
        match self {
            Val::Text(s) => s.trim().to_string(),
            _ => String::new(),
        }
    }

    /// Texte, `None` s'il est vide.
    pub fn opt(&self) -> Option<String> {
        let s = self.text();
        (!s.is_empty()).then_some(s)
    }

    pub fn on(&self) -> bool {
        matches!(self, Val::Bool(true))
    }

    pub fn index(&self) -> usize {
        match self {
            Val::Index(i) => *i,
            _ => 0,
        }
    }
}

/// Un champ : étiquette (avec raccourci `&`), sorte, valeur de départ.
#[derive(Clone, Debug)]
pub struct Field {
    pub label: String,
    pub kind: FieldKind,
    pub value: Val,
}

impl Field {
    pub fn text(label: &str, value: &str) -> Field {
        Field { label: label.into(), kind: FieldKind::Text, value: Val::Text(value.into()) }
    }

    pub fn password(label: &str, value: &str) -> Field {
        Field { label: label.into(), kind: FieldKind::Password, value: Val::Text(value.into()) }
    }

    pub fn number(label: &str, value: i64) -> Field {
        Field { label: label.into(), kind: FieldKind::Number, value: Val::Text(value.to_string()) }
    }

    pub fn check(label: &str, on: bool) -> Field {
        Field { label: label.into(), kind: FieldKind::Check, value: Val::Bool(on) }
    }

    pub fn choice(label: &str, items: Vec<String>, index: usize) -> Field {
        Field { label: label.into(), kind: FieldKind::Choice(items), value: Val::Index(index) }
    }

    pub fn combo(label: &str, items: Vec<String>, value: &str) -> Field {
        Field { label: label.into(), kind: FieldKind::Combo(items), value: Val::Text(value.into()) }
    }
}

/// Validation : reçoit les valeurs dans l'ordre des champs ; `Err(message)` garde la boîte ouverte.
pub type OnOk = Box<dyn FnMut(&[Val], &mut Cx) -> Result<(), String>>;

/// La boîte de saisie.
pub struct Form {
    pub title: String,
    /// Genre (une seule boîte de ce genre à la fois).
    pub kind: String,
    pub fields: Vec<Field>,
    /// Largeur de la zone cliente.
    pub width: i32,
    on_ok: OnOk,
}

/// Premier identifiant des champs (étiquette `ID + 2i`, champ `ID + 2i + 1`).
const FIRST: u32 = 10;
const OK: u32 = 1;
const CANCEL: u32 = 2;

impl Form {
    pub fn new(title: &str, kind: &str, fields: Vec<Field>, on_ok: OnOk) -> Form {
        Form { title: title.into(), kind: kind.into(), fields, width: 360, on_ok }
    }

    pub fn width(mut self, w: i32) -> Form {
        self.width = w;
        self
    }

    fn field_id(i: usize) -> u32 {
        FIRST + 2 * i as u32 + 1
    }

    /// Valeurs actuelles des champs.
    pub fn values(&self, view: &DialogView) -> Vec<Val> {
        self.fields
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let id = Self::field_id(i);
                match &f.kind {
                    FieldKind::Check => Val::Bool(view.checked(id)),
                    FieldKind::Choice(_) => Val::Index(sel(view, id).unwrap_or(0)),
                    _ => Val::Text(text_of(view, id)),
                }
            })
            .collect()
    }
}

impl DialogCtl for Form {
    fn kind(&self) -> Option<String> {
        Some(format!("form:{}", self.kind))
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let w = self.width;
        // Colonne des étiquettes : assez large pour la plus longue (7 px par caractère environ).
        let longest = self.fields.iter().filter(|f| f.kind != FieldKind::Check).map(|f| plain(&f.label).chars().count()).max().unwrap_or(0) as i32;
        let lw = (longest * 7 + 6).clamp(60, w / 2);
        let fx = MARGIN + 4 + lw + 6;
        let fw = w - fx - MARGIN - 4;
        let mut c: Vec<ControlSpec> = Vec::new();
        let mut y = MARGIN + 4;
        for (i, f) in self.fields.iter().enumerate() {
            let lid = FIRST + 2 * i as u32;
            let id = lid + 1;
            match &f.kind {
                FieldKind::Check => {
                    c.push(check(id, (fx, y, fw, CHECK_H), &f.label));
                    y += CHECK_H + 6;
                }
                kind => {
                    c.push(field_label(lid, MARGIN + 4, y, lw, &f.label));
                    let v = match &f.value {
                        Val::Text(s) => s.clone(),
                        _ => String::new(),
                    };
                    c.push(match kind {
                        FieldKind::Password => password(id, (fx, y, fw, EDIT_H), &v),
                        FieldKind::Number => edit(id, (fx, y, fw.min(90), EDIT_H), &v),
                        FieldKind::Choice(items) => drop_list(id, (fx, y, fw, EDIT_H + 8 * 16), items),
                        FieldKind::Combo(items) => combo_edit(id, (fx, y, fw, EDIT_H + 8 * 16), items, &v),
                        _ => edit(id, (fx, y, fw, EDIT_H), &v),
                    });
                    y += ROW;
                }
            }
        }
        let h = y + 8 + BTN_H + MARGIN;
        c.extend(ok_cancel(lang, w, h, OK, CANCEL, None));
        spec(&self.title, w, h, c)
    }

    fn init(&mut self, view: &mut DialogView, _cx: &mut Cx) {
        for (i, f) in self.fields.iter().enumerate() {
            let id = Self::field_id(i);
            match (&f.kind, &f.value) {
                (FieldKind::Check, Val::Bool(on)) => view.set_checked(id, *on),
                (FieldKind::Choice(_), Val::Index(k)) => view.set_selection(id, &[*k]),
                _ => {}
            }
        }
        if !self.fields.is_empty() {
            view.set_focus(Self::field_id(0));
        }
    }

    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, cx: &mut Cx) -> Outcome {
        match msg {
            DialogMsg::Ok(_) | DialogMsg::Clicked(OK) => {
                let vals = self.values(view);
                match (self.on_ok)(&vals, cx) {
                    Ok(()) => Outcome::Close,
                    Err(e) => {
                        cx.actions.push(Action::Open(Box::new(MessageBox::info(&self.title, &e, cx.lang))));
                        Outcome::Keep
                    }
                }
            }
            DialogMsg::Cancel => Outcome::Close,
            _ => Outcome::Keep,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testkit::Kit;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn values_reach_the_callback() {
        let mut k = Kit::new("form");
        let got: Rc<RefCell<Vec<Val>>> = Rc::default();
        let g = got.clone();
        let mut f = Form::new(
            "Entrée",
            "test",
            vec![Field::text("&Pseudo :", "Elisa"), Field::check("&Bip", true), Field::choice("&Couleur :", vec!["a".into(), "b".into()], 1)],
            Box::new(move |v, _cx| {
                if v[0].text().is_empty() {
                    return Err("vide".into());
                }
                *g.borrow_mut() = v.to_vec();
                Ok(())
            }),
        );
        let (mut view, _) = k.open(&mut f);
        view.set_text(11, "Kaelis");
        let (o, _) = k.send(&mut f, &mut view, DialogMsg::Ok(OK));
        assert_eq!(o, Outcome::Close);
        assert_eq!(*got.borrow(), vec![Val::Text("Kaelis".into()), Val::Bool(true), Val::Index(1)]);
        // Refus : la boîte reste ouverte et un message s'ouvre.
        view.set_text(11, " ");
        let (o, actions) = k.send(&mut f, &mut view, DialogMsg::Ok(OK));
        assert_eq!(o, Outcome::Keep);
        assert!(matches!(actions.first(), Some(Action::Open(_))));
    }
}
