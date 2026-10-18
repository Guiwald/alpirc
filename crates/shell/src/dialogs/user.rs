//! User Central (`/uwho pseudo`, entrée « Infos » des menus de la liste des pseudos et des privés), comme dans
//! mIRC : en haut, la fiche whois de la personne telle que le serveur l'a donnée (adresse, nom, serveur, canaux,
//! compte, inactivité, absence…), qui se remplit au fil des réponses ; en bas, ce que le carnet d'adresses garde
//! sur elle (nom réel, e-mail, site, notes), modifiable. OK enregistre la fiche du carnet ; « Actualiser » refait la
//! recherche, « Copier tout » copie la fiche whois, « Privé » ouvre une discussion privée.

use alpirc_core::i18n::Lang;
use alpirc_core::settings::PersonEntry;
use alpirc_core::{ConnId, WhoisInfo, WhoisState};
use alpirc_ui::{ControlSpec, DialogMsg, DialogSpec, DialogView};

use super::*;

const W: i32 = 460;
const H: i32 = 497;

const OK: u32 = 1;
const CANCEL: u32 = 2;
/// Lignes de la fiche whois : (clé du libellé, lignes de texte).
const ROWS: [(&str, i32); 9] = [
    ("uc.address", 1),
    ("uc.name", 1),
    ("uc.server", 1),
    ("uc.channels", 2),
    ("uc.account", 1),
    ("uc.idle", 1),
    ("uc.signon", 1),
    ("uc.away", 1),
    ("uc.other", 2),
];
/// Premier libellé et première valeur des lignes de la fiche (un par ligne, dans l'ordre de [`ROWS`]).
const LABEL0: u32 = 20;
const VALUE0: u32 = 30;
const STATE: u32 = 40;
const REALNAME: u32 = 51;
const EMAIL: u32 = 52;
const WEBSITE: u32 = 53;
const NOTES: u32 = 54;
const EMAIL_BTN: u32 = 72;
const VIEW_BTN: u32 = 73;
const REFRESH: u32 = 80;
const COPY: u32 = 81;
const QUERY: u32 = 82;

/// Valeurs de la fiche whois montrées, dans l'ordre de [`ROWS`], et la ligne d'état.
fn whois_texts(info: Option<&WhoisInfo>, nick: &str, connected: bool, lang: Lang, offset: i32) -> ([String; 9], String) {
    let mut v: [String; 9] = Default::default();
    let Some(i) = info else {
        let state = if connected { tf(lang, "uc.pending", &[("nick", nick)]) } else { t(lang, "uc.not_connected") };
        return (v, state);
    };
    v[0] = i.address();
    v[1] = i.realname.clone();
    v[2] = match (i.server.is_empty(), i.server_info.is_empty()) {
        (true, _) => String::new(),
        (false, true) => i.server.clone(),
        (false, false) => format!("{} ({})", i.server, i.server_info),
    };
    v[3] = i.channels.clone();
    v[4] = i.account.clone().unwrap_or_default();
    v[5] = i.idle_secs.map(|s| alpirc_core::timeutil::duration(s as i64)).unwrap_or_default();
    v[6] = i.signon.map(|s| alpirc_core::timeutil::date_text(s * 1000, offset, lang)).unwrap_or_default();
    v[7] = i.away.clone().unwrap_or_default();
    let mut other: Vec<String> = Vec::new();
    if let Some(o) = &i.operator {
        other.push(o.clone());
    }
    if i.secure {
        other.push(t(lang, "uc.secure"));
    }
    other.extend(i.extra.iter().cloned());
    v[8] = other.join(" ; ");
    let state = match i.state {
        WhoisState::Pending if i.address().is_empty() => tf(lang, "uc.pending", &[("nick", nick)]),
        WhoisState::Pending => String::new(),
        WhoisState::NoSuchNick => tf(lang, "uc.nosuch", &[("nick", nick)]),
        WhoisState::Done => {
            let at = alpirc_core::timeutil::format_time(i.updated_ms, offset, "HH:nn:ss");
            tf(lang, "uc.updated", &[("time", &at)])
        }
    };
    (v, state)
}

/// Adresse d'un site à ouvrir (ajoute `https://` s'il manque le protocole).
fn web_url(s: &str) -> String {
    let s = s.trim();
    if s.contains("://") { s.to_string() } else { format!("https://{s}") }
}

/// User Central.
pub struct UserCentral {
    conn: ConnId,
    nick: String,
    /// Fiche whois montrée (pour ne redessiner que ce qui change).
    shown: Option<WhoisInfo>,
    shown_connected: bool,
    utc_offset: i32,
}

impl UserCentral {
    pub fn new(conn: ConnId, nick: &str) -> UserCentral {
        UserCentral { conn, nick: nick.to_string(), shown: None, shown_connected: false, utc_offset: 0 }
    }

    fn connected(&self, client: &Client) -> bool {
        client.conn_info(self.conn).is_some_and(|c| c.state == alpirc_core::ConnState::Connected)
    }

    fn show_whois(&mut self, view: &mut DialogView, client: &Client, lang: Lang) {
        let info = client.whois_info(self.conn, &self.nick).cloned();
        let connected = self.connected(client);
        let nick = info.as_ref().map(|i| i.nick.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| self.nick.clone());
        let (values, state) = whois_texts(info.as_ref(), &nick, connected, lang, self.utc_offset);
        for (k, v) in values.iter().enumerate() {
            view.set_text(VALUE0 + k as u32, v);
        }
        view.set_text(STATE, &state);
        view.set_enabled(COPY, info.as_ref().is_some_and(|i| !i.lines().is_empty()));
        view.set_enabled(REFRESH, connected);
        view.set_enabled(QUERY, connected);
        self.shown = info;
        self.shown_connected = connected;
    }

    fn person_now(&self, view: &DialogView) -> PersonEntry {
        PersonEntry {
            nick: self.nick.clone(),
            name: text_of(view, REALNAME).trim().to_string(),
            email: text_of(view, EMAIL).trim().to_string(),
            website: text_of(view, WEBSITE).trim().to_string(),
            notes: text_of(view, NOTES).trim_end().to_string(),
        }
    }

    fn update_buttons(&self, view: &mut DialogView) {
        view.set_enabled(EMAIL_BTN, opt_text(view, EMAIL).is_some_and(|e| e.contains('@')));
        view.set_enabled(VIEW_BTN, opt_text(view, WEBSITE).is_some());
    }

    /// OK : enregistre la fiche du carnet d'adresses si elle a changé.
    fn apply(&self, view: &DialogView, cx: &mut Cx) {
        let mut e = self.person_now(view);
        let book = &cx.client.settings().address_book;
        if let Some(old) = book.person(&self.nick) {
            // Garde le pseudo tel qu'il était écrit dans le carnet.
            e.nick = old.nick.clone();
            if *old == e {
                return;
            }
        } else if e.is_blank() {
            return;
        }
        cx.client.settings_mut().address_book.set_person(e);
        cx.actions.push(Action::SettingsChanged);
    }

    fn status_win(&self, client: &Client) -> Option<WinId> {
        client.conn_info(self.conn).map(|c| c.status_window)
    }
}

impl DialogCtl for UserCentral {
    fn kind(&self) -> Option<String> {
        Some(format!("uwho:{}:{}", self.conn, self.nick.to_lowercase()))
    }

    fn spec(&self, lang: Lang) -> DialogSpec {
        let tr = |k: &str| t(lang, k);
        let gw = W - 2 * MARGIN;
        let lw = 112;
        let vx = 20 + lw;
        let vw = W - 20 - vx;
        let mut c: Vec<ControlSpec> = Vec::new();
        let mut y = 26;
        for (k, (key, lines)) in ROWS.iter().enumerate() {
            let h = LABEL_H * lines;
            c.push(text(LABEL0 + k as u32, (20, y, lw, LABEL_H), &tr(key)).style("nowrap"));
            let v = text(VALUE0 + k as u32, (vx, y, vw, h), "");
            c.push(if *lines == 1 { v.style("nowrap") } else { v });
            y += h + 2;
        }
        c.push(text(STATE, (20, y + 4, W - 40, LABEL_H), "").style("nowrap"));
        let whois_h = y + 4 + LABEL_H + 8 - MARGIN;
        c.insert(0, group(10, (MARGIN, MARGIN, gw, whois_h), &tr("uc.whois")));
        // Carnet d'adresses.
        let gy = MARGIN + whois_h + 8;
        let ex = 20 + 76;
        let bw = BTN_W;
        let ew = W - 20 - ex - 6 - bw;
        let r1 = gy + 18;
        c.push(group(50, (MARGIN, gy, gw, H - gy - MARGIN - BTN_H - 8 - 8), &tr("uc.abook")));
        c.push(field_label(61, 20, r1, 74, &tr("uc.realname")));
        c.push(edit(REALNAME, (ex, r1, W - 20 - ex, EDIT_H), "").style("limit 100"));
        c.push(field_label(62, 20, r1 + 26, 74, &tr("uc.email")));
        c.push(edit(EMAIL, (ex, r1 + 26, ew, EDIT_H), "").style("limit 200"));
        c.push(button(EMAIL_BTN, (ex + ew + 6, r1 + 26, bw, EDIT_H), &tr("uc.btn_email")));
        c.push(field_label(63, 20, r1 + 52, 74, &tr("uc.website")));
        c.push(edit(WEBSITE, (ex, r1 + 52, ew, EDIT_H), "").style("limit 300"));
        c.push(button(VIEW_BTN, (ex + ew + 6, r1 + 52, bw, EDIT_H), &tr("uc.btn_view")));
        c.push(text(64, (20, r1 + 80, W - 40, LABEL_H), &tr("uc.notes")).style("nowrap"));
        let ny = r1 + 98;
        let nh = H - MARGIN - BTN_H - 8 - 8 - 10 - ny;
        c.push(edit(NOTES, (20, ny, W - 40, nh), "").style("multi limit 4000"));
        // Boutons.
        let by = H - MARGIN - BTN_H;
        c.push(button(REFRESH, (MARGIN, by, BTN_W, BTN_H), &tr("uc.refresh")));
        c.push(button(COPY, (MARGIN + BTN_W + 6, by, BTN_W, BTN_H), &tr("uc.copy")));
        c.push(button(QUERY, (MARGIN + 2 * (BTN_W + 6), by, BTN_W, BTN_H), &tr("uc.query")));
        c.extend(ok_cancel(lang, W, H, OK, CANCEL, None));
        spec(&tf(lang, "uc.title", &[("nick", &self.nick)]), W, H, c)
    }

    fn init(&mut self, view: &mut DialogView, cx: &mut Cx) {
        self.utc_offset = cx.utc_offset;
        self.show_whois(view, cx.client, cx.lang);
        let e = cx.client.settings().address_book.person(&self.nick).cloned().unwrap_or_default();
        view.set_text(REALNAME, &e.name);
        view.set_text(EMAIL, &e.email);
        view.set_text(WEBSITE, &e.website);
        view.set_text(NOTES, &e.notes);
        self.update_buttons(view);
        view.set_focus(NOTES);
    }

    fn on(&mut self, msg: DialogMsg, view: &mut DialogView, cx: &mut Cx) -> Outcome {
        match msg {
            DialogMsg::Ok(_) => {
                self.apply(view, cx);
                Outcome::Close
            }
            DialogMsg::Cancel => Outcome::Close,
            DialogMsg::Changed(EMAIL | WEBSITE) => {
                self.update_buttons(view);
                Outcome::Keep
            }
            DialogMsg::Clicked(EMAIL_BTN) => {
                if let Some(e) = opt_text(view, EMAIL) {
                    cx.actions.push(Action::OpenUrl(format!("mailto:{e}")));
                }
                Outcome::Keep
            }
            DialogMsg::Clicked(VIEW_BTN) => {
                if let Some(u) = opt_text(view, WEBSITE) {
                    cx.actions.push(Action::OpenUrl(web_url(&u)));
                }
                Outcome::Keep
            }
            DialogMsg::Clicked(REFRESH) => {
                if let Some(w) = self.status_win(cx.client) {
                    // Deux fois le pseudo : le serveur de la personne donne aussi l'inactivité et l'absence.
                    let _ = cx.client.builtin(w, "uwho", &format!("{0} {0}", self.nick), cx.now);
                }
                self.show_whois(view, cx.client, cx.lang);
                Outcome::Keep
            }
            DialogMsg::Clicked(COPY) => {
                if let Some(i) = &self.shown {
                    cx.actions.push(Action::CopyText(i.lines().join("\n")));
                }
                Outcome::Keep
            }
            DialogMsg::Clicked(QUERY) => {
                if let Some(w) = self.status_win(cx.client) {
                    cx.actions.push(Action::Command(w, format!("/query {}", self.nick)));
                }
                Outcome::Keep
            }
            _ => Outcome::Keep,
        }
    }

    fn refresh(&mut self, view: &mut DialogView, cx: &mut Cx) {
        self.utc_offset = cx.utc_offset;
        let info = cx.client.whois_info(self.conn, &self.nick);
        if info != self.shown.as_ref() || self.connected(cx.client) != self.shown_connected {
            self.show_whois(view, cx.client, cx.lang);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::testkit::{Kit, assert_translated};

    #[test]
    fn uwho_fills_the_box_quietly_and_saves_notes() {
        let mut k = Kit::new("uwho");
        let (conn, status) = k.connect();
        let lines_before = k.client.window(status).unwrap().lines.len();
        k.client.command(status, "/uwho Elisa", k.now);
        let fx = k.client.take_effects();
        assert!(fx.iter().any(|e| matches!(e, alpirc_core::Effect::Send { line, .. } if line == "WHOIS Elisa")), "{fx:?}");
        assert!(fx.contains(&alpirc_core::Effect::OpenDialog(alpirc_core::UiDialog::UserCentral { conn, nick: "Elisa".into() })));
        let mut d = UserCentral::new(conn, "Elisa");
        assert_translated(&d);
        let (mut v, _) = k.open(&mut d);
        assert_eq!(text_of(&v, STATE), "Looking up Elisa...");
        for l in [
            ":irc.undernet.org 311 Guiwald Elisa ~elisa lyon.example * :Elisa D.",
            ":irc.undernet.org 319 Guiwald Elisa :@#lyon #grenoble",
            ":irc.undernet.org 312 Guiwald Elisa irc.undernet.org :The Undernet Underworld",
            ":irc.undernet.org 330 Guiwald Elisa elisa :is logged in as",
            ":irc.undernet.org 301 Guiwald Elisa :parti manger",
            ":irc.undernet.org 317 Guiwald Elisa 303 1792250000 :seconds idle, signon time",
            ":irc.undernet.org 318 Guiwald Elisa :End of /WHOIS list.",
        ] {
            k.line(conn, l);
        }
        // Rien ne s'affiche dans les fenêtres : la boîte montre tout.
        for w in k.client.windows() {
            assert!(!w.lines.iter().any(|l| l.text.contains("lyon.example") || l.text.contains("parti manger")), "{}", w.name);
        }
        assert_eq!(k.client.window(status).unwrap().lines.len(), lines_before);
        k.refresh(&mut d, &mut v);
        assert_eq!(text_of(&v, VALUE0), "~elisa@lyon.example");
        assert_eq!(text_of(&v, VALUE0 + 1), "Elisa D.");
        assert_eq!(text_of(&v, VALUE0 + 2), "irc.undernet.org (The Undernet Underworld)");
        assert_eq!(text_of(&v, VALUE0 + 3), "@#lyon #grenoble");
        assert_eq!(text_of(&v, VALUE0 + 4), "elisa");
        assert_eq!(text_of(&v, VALUE0 + 5), "5mins 3secs");
        assert!(!text_of(&v, VALUE0 + 6).is_empty());
        assert_eq!(text_of(&v, VALUE0 + 7), "parti manger");
        assert!(text_of(&v, STATE).starts_with("Updated at "), "{}", text_of(&v, STATE));
        // Copier tout.
        let (_, a) = k.send(&mut d, &mut v, DialogMsg::Clicked(COPY));
        let Some(Action::CopyText(txt)) = a.first() else { panic!() };
        assert!(txt.contains("Elisa ~elisa@lyon.example * Elisa D.") && txt.contains("@#lyon #grenoble"), "{txt}");
        // Notes du carnet d'adresses.
        v.set_text(NOTES, "Rencontrée sur #lyon en 2003.\nAime le ski.");
        v.set_text(EMAIL, "elisa@example.org");
        k.send(&mut d, &mut v, DialogMsg::Changed(EMAIL));
        assert!(v.enabled(EMAIL_BTN));
        let (o, a) = k.send(&mut d, &mut v, DialogMsg::Ok(OK));
        assert_eq!(o, Outcome::Close);
        assert!(a.iter().any(|a| matches!(a, Action::SettingsChanged)));
        let p = k.client.settings().address_book.person("elisa").cloned().unwrap();
        assert_eq!(p.notes, "Rencontrée sur #lyon en 2003.\nAime le ski.");
        assert_eq!(p.email, "elisa@example.org");
        // Rouverte : les notes sont là.
        let mut d2 = UserCentral::new(conn, "ELISA");
        let (v2, _) = k.open(&mut d2);
        assert_eq!(text_of(&v2, NOTES), "Rencontrée sur #lyon en 2003.\nAime le ski.");
        assert_eq!(text_of(&v2, VALUE0), "~elisa@lyon.example", "fiche gardée");
    }

    #[test]
    fn no_such_nick_and_plain_whois_still_prints() {
        let mut k = Kit::new("uwho2");
        let (conn, status) = k.connect();
        k.client.command(status, "/uwho Fantome", k.now);
        k.client.take_effects();
        k.line(conn, ":irc.undernet.org 401 Guiwald Fantome :No such nick");
        k.line(conn, ":irc.undernet.org 318 Guiwald Fantome :End of /WHOIS list.");
        let mut d = UserCentral::new(conn, "Fantome");
        let (v, _) = k.open(&mut d);
        assert_eq!(text_of(&v, STATE), "Fantome is not on IRC.");
        assert!(!k.client.windows().iter().any(|w| w.lines.iter().any(|l| l.text.contains("No such nick"))));
        // Un /whois ordinaire s'affiche toujours, et met la fiche à jour.
        k.client.command(status, "/whois Elisa", k.now);
        k.line(conn, ":irc.undernet.org 311 Guiwald Elisa ~elisa lyon.example * :Elisa D.");
        k.line(conn, ":irc.undernet.org 318 Guiwald Elisa :End of /WHOIS list.");
        assert!(k.client.window(status).unwrap().lines.iter().any(|l| l.text.contains("lyon.example")));
        assert_eq!(k.client.whois_info(conn, "elisa").map(|i| i.state), Some(WhoisState::Done));
        // Un message à un absent hors d'un whois ne touche pas la fiche.
        k.line(conn, ":irc.undernet.org 301 Guiwald Elisa :ailleurs");
        assert_eq!(k.client.whois_info(conn, "elisa").and_then(|i| i.away.clone()), None);
    }

    #[test]
    fn web_addresses() {
        assert_eq!(web_url("alpirc.org"), "https://alpirc.org");
        assert_eq!(web_url(" http://x.example "), "http://x.example");
    }
}
