//! Premier lancement (ADR 0010, README « Premiers pas ») : sans fichier de réglages, AlpIRC demande le pseudo
//! (Guiwald par défaut), le pseudo de secours, le nom complet et, si on veut, la langue ; puis propose de se
//! connecter à Undernet en rejoignant les canaux favoris, ou d'ouvrir le choix du serveur. Les champs sont ceux de
//! la boîte que mIRC ouvre à son premier lancement (Options › Connexion).

use alpirc_core::i18n::Lang;
use alpirc_core::settings::Favorite;
use alpirc_ui::{ControlKind, ControlSpec, DialogMsg, DialogSpec, DialogView};

use super::*;

const CONNECT: u32 = 1;
const SELECT: u32 = 2;
const LATER: u32 = 3;
const INTRO: u32 = 4;
const ICON: u32 = 5;
const GROUP: u32 = 6;
const NICK_L: u32 = 10;
const NICK: u32 = 11;
const ALT_L: u32 = 12;
const ALT: u32 = 13;
const REAL_L: u32 = 14;
const REAL: u32 = 15;
const LANG_L: u32 = 16;
const LANG: u32 = 17;
const WHERE: u32 = 18;

const W: i32 = 460;
const H: i32 = 290;

/// Langues proposées : celle du système, puis chacune.
fn lang_codes() -> Vec<Option<&'static str>> {
    std::iter::once(None).chain(Lang::ALL.iter().map(|l| Some(l.code()))).collect()
}

/// Le réseau proposé.
const NETWORK: &str = "Undernet";

/// Pseudo acceptable par les serveurs : pas vide, sans espace, ne commence ni par un chiffre ni par `-`.
pub fn valid_nick(n: &str) -> bool {
    let ok = |c: char| c.is_ascii_alphanumeric() || "[]\\`_^{|}-".contains(c);
    n.chars().next().is_some_and(|c| !c.is_ascii_digit() && c != '-') && n.chars().all(ok) && n.chars().count() <= 30
}

/// La boîte du premier lancement.
#[derive(Clone)]
pub struct Welcome {
    nick: String,
    alt: String,
    real: String,
    /// Rang dans [`lang_codes`].
    lang: usize,
}

impl Welcome {
    pub fn new(client: &Client) -> Welcome {
        let s = client.settings();
        let codes = lang_codes();
        let is = |l: &str| codes.iter().position(|c| c.is_some_and(|c| c.eq_ignore_ascii_case(l)));
        let lang = s.language.as_deref().and_then(is).unwrap_or(0);
        Welcome { nick: s.identity.nick.clone(), alt: s.identity.alt_nick.clone(), real: s.identity.realname.clone(), lang }
    }

    fn read(&mut self, view: &DialogView) {
        self.nick = text_of(view, NICK).trim().to_string();
        self.alt = text_of(view, ALT).trim().to_string();
        self.real = text_of(view, REAL).trim().to_string();
        self.lang = sel(view, LANG).unwrap_or(0);
    }

    /// Favoris du réseau proposé.
    fn favorites(client: &Client) -> Vec<Favorite> {
        client.settings().favorites.iter().filter(|f| f.network.eq_ignore_ascii_case(NETWORK)).cloned().collect()
    }

    /// Écrit l'identité et la langue dans les réglages (enregistrés par l'application).
    fn save(&self, cx: &mut Cx) {
        let s = cx.client.settings_mut();
        s.identity.nick = self.nick.clone();
        let same = self.alt.is_empty() || self.alt.eq_ignore_ascii_case(&self.nick);
        s.identity.alt_nick = if same { format!("{}_", self.nick) } else { self.alt.clone() };
        if !self.real.is_empty() {
            s.identity.realname = self.real.clone();
        }
        s.language = lang_codes().get(self.lang).copied().flatten().map(str::to_string);
        cx.actions.push(Action::SettingsChanged);
    }
}

impl DialogCtl for Welcome {
    fn kind(&self) -> Option<String> {
        Some("welcome".into())
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let tr = |k: &str| t(lang, k);
        let mut icon = ControlSpec::new(ICON, ControlKind::Icon, (MARGIN + 6, MARGIN + 6, 32, 32), "app/alpirc-32");
        icon.styles.push("noborder".into());
        let (x0, x1) = (MARGIN, W - MARGIN);
        let tx = MARGIN + 6 + 32 + 14;
        let gy = MARGIN + 58;
        let (lx, fx) = (x0 + 12, x0 + 152);
        let row = |k: i32| gy + 22 + k * ROW;
        let mut langs = vec![tr("opt.disp.lang_auto")];
        langs.extend(Lang::ALL.iter().map(|l| l.native_name().to_string()));
        let by = H - MARGIN - BTN_H;
        let c = vec![
            icon,
            text(INTRO, (tx, MARGIN + 6, x1 - tx, 48), &tr("wel.intro")),
            group(GROUP, (x0, gy, x1 - x0, 4 * ROW + 26), &tr("wel.identity")),
            field_label(NICK_L, lx, row(0), 138, &tr("opt.nick")),
            edit(NICK, (fx, row(0), 200, EDIT_H), "").style("limit 30"),
            field_label(ALT_L, lx, row(1), 138, &tr("opt.alt")),
            edit(ALT, (fx, row(1), 200, EDIT_H), "").style("limit 30"),
            field_label(REAL_L, lx, row(2), 138, &tr("opt.realname")),
            edit(REAL, (fx, row(2), x1 - 12 - fx, EDIT_H), "").style("limit 100"),
            field_label(LANG_L, lx, row(3), 138, &tr("wel.language")),
            drop_list(LANG, (fx, row(3), 200, EDIT_H + 8 * 16 + 4), &langs),
            text(WHERE, (x0 + 4, gy + 4 * ROW + 34, x1 - x0 - 8, 32), &tr("wel.where")),
            button(CONNECT, (x0, by, 180, BTN_H), &tr("wel.connect")).style("default"),
            button(SELECT, (x0 + 186, by, 150, BTN_H), &tr("wel.select")),
            button(LATER, (x1 - BTN_W, by, BTN_W, BTN_H), &tr("wel.later")),
        ];
        spec(&tr("wel.title"), W, H, c)
    }

    fn init(&mut self, view: &mut DialogView, _cx: &mut Cx) {
        view.set_text(NICK, &self.nick);
        view.set_text(ALT, &self.alt);
        view.set_text(REAL, &self.real);
        view.set_selection(LANG, &[self.lang]);
        view.set_focus(NICK);
    }

    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, cx: &mut Cx) -> Outcome {
        match msg {
            DialogMsg::SelectionChanged(LANG) => {
                // La boîte (et toute l'interface) dans la langue choisie, en gardant ce qui est tapé.
                self.read(view);
                self.save(cx);
                cx.actions.push(Action::Open(Box::new(self.clone())));
                Outcome::Close
            }
            DialogMsg::Clicked(id @ (CONNECT | SELECT | LATER)) | DialogMsg::Ok(id) => {
                self.read(view);
                if !valid_nick(&self.nick) || (!self.alt.is_empty() && !valid_nick(&self.alt)) {
                    let m = MessageBox::info(&t(cx.lang, "wel.title"), &t(cx.lang, "wel.bad_nick"), cx.lang);
                    cx.actions.push(Action::Open(Box::new(m)));
                    view.set_focus(if valid_nick(&self.nick) { ALT } else { NICK });
                    return Outcome::Keep;
                }
                self.save(cx);
                match id {
                    CONNECT => {
                        if let Some(st) = status_window(cx.client) {
                            cx.client.connect(st, NETWORK, None, cx.now);
                            // Les canaux, une fois la connexion prête.
                            cx.actions.extend(Welcome::favorites(cx.client).into_iter().map(Action::JoinFavorite));
                        }
                    }
                    SELECT => cx.actions.push(Action::OpenUi(alpirc_core::UiDialog::Connect)),
                    _ => {}
                }
                Outcome::Close
            }
            DialogMsg::Cancel => {
                // Fermée sans choisir : on garde ce qui est tapé s'il est valable, sans se connecter.
                self.read(view);
                if valid_nick(&self.nick) {
                    self.save(cx);
                }
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

    #[test]
    fn nicknames() {
        assert!(valid_nick("Guiwald") && valid_nick("[Elisa]") && valid_nick("a-b_c|d"));
        assert!(!valid_nick("") && !valid_nick("4you") && !valid_nick("-x") && !valid_nick("deux mots") && !valid_nick("é"));
    }

    #[test]
    fn identity_then_connect_to_undernet_with_favorites() {
        let mut k = Kit::new("welcome");
        let mut d = Welcome::new(&k.client);
        assert_translated(&d);
        let (mut v, _) = k.open(&mut d);
        assert_eq!(text_of(&v, NICK), "Guiwald");
        assert_eq!(v.focus(), Some(NICK));
        // Pseudo refusé : la boîte reste.
        v.set_text(NICK, "4you");
        let (o, a) = k.send(&mut d, &mut v, DialogMsg::Clicked(CONNECT));
        assert_eq!(o, Outcome::Keep);
        assert!(matches!(a.first(), Some(Action::Open(_))));
        v.set_text(NICK, "Elisa");
        v.set_text(ALT, "");
        v.set_text(REAL, "Élisa de Lyon");
        let (o, a) = k.send(&mut d, &mut v, DialogMsg::Clicked(CONNECT));
        assert_eq!(o, Outcome::Close);
        let id = &k.client.settings().identity;
        assert_eq!((id.nick.as_str(), id.alt_nick.as_str(), id.realname.as_str()), ("Elisa", "Elisa_", "Élisa de Lyon"));
        assert!(matches!(a.first(), Some(Action::SettingsChanged)));
        let joins: Vec<String> = a.iter().filter_map(|x| if let Action::JoinFavorite(f) = x { Some(f.channel.clone()) } else { None }).collect();
        assert_eq!(joins, vec!["#lyon", "#grenoble", "#chambery"]);
        let conn = k.client.conns()[0];
        assert_eq!(k.client.conn_info(conn).unwrap().network, "Undernet");
    }

    #[test]
    fn language_reopens_the_box_and_select_server() {
        let mut k = Kit::new("welcome2");
        let mut d = Welcome::new(&k.client);
        let (mut v, _) = k.open(&mut d);
        v.set_text(NICK, "Nemo");
        let fr = lang_codes().iter().position(|c| *c == Some("fr")).unwrap();
        v.set_selection(LANG, &[fr]);
        let (o, a) = k.send(&mut d, &mut v, DialogMsg::SelectionChanged(LANG));
        assert_eq!(o, Outcome::Close);
        assert_eq!(k.client.settings().language.as_deref(), Some("fr"));
        let Some(Action::Open(mut again)) = a.into_iter().nth(1) else { panic!("boîte rouverte") };
        assert_eq!(again.spec(Lang::Fr).title, "Bienvenue dans AlpIRC");
        let (mut v2, _) = k.open(again.as_mut());
        assert_eq!(text_of(&v2, NICK), "Nemo", "le pseudo tapé reste");
        assert_eq!(sel(&v2, LANG), Some(fr));
        let (_, a) = k.send(again.as_mut(), &mut v2, DialogMsg::Clicked(SELECT));
        assert!(a.iter().any(|x| matches!(x, Action::OpenUi(alpirc_core::UiDialog::Connect))));
        let connecting = k.client.conns().iter().any(|c| k.client.conn_info(*c).is_some_and(|i| i.state != alpirc_core::ConnState::Disconnected));
        assert!(!connecting, "pas de connexion sans le choix du serveur");
    }
}
