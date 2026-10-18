//! Scripts écrits au premier démarrage dans `<données>/scripts` : popups à la manière de mIRC 6 et quelques alias
//! classiques. Ce sont des fichiers « remote » ordinaires : l'utilisateur peut les modifier.
//!
//! Les libellés des menus et les questions viennent de [`TEXTS`], dans la langue de l'interface. Un fichier que
//! l'utilisateur n'a pas modifié (identique à celui qu'une version d'AlpIRC a écrit, dans l'une des sept langues)
//! suit la langue de l'interface et les menus de la version en cours ([`is_generated`]) ; un fichier modifié n'est
//! jamais réécrit.
//!
//! Garder les anciens modèles dans [`OLD_POPUPS`] et [`OLD_ALIASES`] quand on change un modèle ; ne pas changer le
//! texte d'une clé de [`TEXTS`] déjà publiée (en ajouter une autre), sinon les fichiers écrits avant ne seraient
//! plus reconnus.

use crate::i18n::Lang;

/// Textes des scripts par défaut : (clé, [en, fr, es, de, it, va, ga]) ; une langue vide retombe sur l'anglais.
/// Un libellé de menu ne doit pas contenir `:` (il sépare le libellé de la commande).
pub const TEXTS: &[(&str, [&str; 7])] = &[
    (
        "popups.comment",
        [
            "Default AlpIRC menus, in the style of mIRC 6.",
            "Menus par défaut d'AlpIRC, à la manière de mIRC 6.",
            "Menús predeterminados de AlpIRC, al estilo de mIRC 6.",
            "Standardmenüs von AlpIRC im Stil von mIRC 6.",
            "Menu predefiniti di AlpIRC, nello stile di mIRC 6.",
            "Menús predeterminats d'AlpIRC, a l'estil de mIRC 6.",
            "Roghchláir réamhshocraithe AlpIRC, i stíl mIRC 6.",
        ],
    ),
    (
        "aliases.comment",
        [
            "Default AlpIRC aliases.",
            "Alias par défaut d'AlpIRC.",
            "Alias predeterminados de AlpIRC.",
            "Standard-Aliases von AlpIRC.",
            "Alias predefiniti di AlpIRC.",
            "Àlies predeterminats d'AlpIRC.",
            "Ailiasanna réamhshocraithe AlpIRC.",
        ],
    ),
    ("lusers", ["Lusers", "Utilisateurs", "Usuarios", "Benutzer", "Utenti", "Usuaris", "Úsáideoirí"]),
    (
        "motd",
        [
            "Motd",
            "Message du jour",
            "Mensaje del día",
            "Nachricht des Tages",
            "Messaggio del giorno",
            "Missatge del dia",
            "Teachtaireacht an lae",
        ],
    ),
    ("time", ["Time", "Heure", "Hora", "Uhrzeit", "Ora", "Hora", "Am"]),
    (
        "join",
        [
            "Join channel",
            "Rejoindre un canal",
            "Entrar en un canal",
            "Channel betreten",
            "Entra in un canale",
            "Entrar en un canal",
            "Téigh isteach i gcainéal",
        ],
    ),
    (
        "join.ask",
        [
            "Enter channel name:",
            "Nom du canal :",
            "Nombre del canal:",
            "Channelname:",
            "Nome del canale:",
            "Nom del canal:",
            "Ainm an chainéil:",
        ],
    ),
    ("quit", ["Quit", "Quitter", "Salir", "Beenden", "Esci", "Eixir", "Scoir"]),
    (
        "modes",
        [
            "Channel Modes",
            "Modes du canal",
            "Modos del canal",
            "Channel-Modi",
            "Modi del canale",
            "Modes del canal",
            "Móid an chainéil",
        ],
    ),
    (
        "folder",
        [
            "Channels Folder",
            "Dossier des canaux",
            "Carpeta de canales",
            "Channel-Ordner",
            "Cartella dei canali",
            "Carpeta de canals",
            "Fillteán cainéal",
        ],
    ),
    (
        "part",
        [
            "Part",
            "Quitter le canal",
            "Salir del canal",
            "Channel verlassen",
            "Esci dal canale",
            "Eixir del canal",
            "Fág an cainéal",
        ],
    ),
    ("info", ["Info", "Infos", "Info", "Info", "Info", "Info", "Eolas"]),
    ("whois", ["Whois", "Whois", "Whois", "Whois", "Whois", "Whois", "Whois"]),
    ("query", ["Query", "Privé", "Privado", "Query", "Query", "Privat", "Comhrá"]),
    ("dcc", ["DCC", "DCC", "DCC", "DCC", "DCC", "DCC", "DCC"]),
    (
        "dcc.send",
        ["Send", "Envoyer un fichier", "Enviar archivo", "Datei senden", "Invia file", "Enviar fitxer", "Seol comhad"],
    ),
    ("dcc.chat", ["Chat", "Discussion", "Chat", "Chat", "Chat", "Xat", "Comhrá"]),
    ("ignore", ["Ignore", "Ignorer", "Ignorar", "Ignorieren", "Ignora", "Ignorar", "Déan neamhaird"]),
    ("close", ["Close", "Fermer", "Cerrar", "Schließen", "Chiudi", "Tancar", "Dún"]),
    ("control", ["Control", "Contrôle", "Control", "Control", "Controllo", "Control", "Rialú"]),
    ("op", ["Op", "Op", "Dar op", "Op", "Op", "Donar op", "Tabhair op"]),
    ("deop", ["Deop", "Deop", "Quitar op", "Deop", "Deop", "Llevar op", "Bain op"]),
    ("voice", ["Voice", "Voice", "Dar voz", "Voice", "Voice", "Donar veu", "Tabhair guth"]),
    ("devoice", ["Devoice", "Devoice", "Quitar voz", "Devoice", "Devoice", "Llevar veu", "Bain guth"]),
    ("kick", ["Kick", "Expulser", "Expulsar", "Kicken", "Espelli", "Expulsar", "Caith amach"]),
    (
        "kick.why",
        [
            "Kick (why)",
            "Expulser (raison)",
            "Expulsar (motivo)",
            "Kicken (Grund)",
            "Espelli (motivo)",
            "Expulsar (motiu)",
            "Caith amach (cúis)",
        ],
    ),
    ("reason.ask", ["Reason:", "Raison :", "Motivo:", "Grund:", "Motivo:", "Motiu:", "Cúis:"]),
    ("ban", ["Ban", "Bannir", "Banear", "Bannen", "Banna", "Bandejar", "Toirmisc"]),
    (
        "ban.kick",
        [
            "Ban, Kick",
            "Bannir, expulser",
            "Banear, expulsar",
            "Bannen, kicken",
            "Banna, espelli",
            "Bandejar, expulsar",
            "Toirmisc, caith amach",
        ],
    ),
    (
        "ban.kick.why",
        [
            "Ban, Kick (why)",
            "Bannir, expulser (raison)",
            "Banear, expulsar (motivo)",
            "Bannen, kicken (Grund)",
            "Banna, espelli (motivo)",
            "Bandejar, expulsar (motiu)",
            "Toirmisc, caith amach (cúis)",
        ],
    ),
    ("ctcp", ["CTCP", "CTCP", "CTCP", "CTCP", "CTCP", "CTCP", "CTCP"]),
    ("ping", ["Ping", "Ping", "Ping", "Ping", "Ping", "Ping", "Ping"]),
    ("ctcp.time", ["Time", "Heure", "Hora", "Uhrzeit", "Ora", "Hora", "Am"]),
    ("version", ["Version", "Version", "Versión", "Version", "Versione", "Versió", "Leagan"]),
    ("slap", ["Slap!", "Claque !", "¡Bofetón!", "Ohrfeige!", "Schiaffo!", "Bufetada!", "Leadóg!"]),
    (
        "slap.text",
        [
            "slaps $$1 around a bit with a large trout",
            "donne une grande claque à $$1 avec une truite",
            "le da a $$1 un buen bofetón con una trucha enorme",
            "haut $$1 eine große Forelle um die Ohren",
            "schiaffeggia $$1 con una grossa trota",
            "pega a $$1 una bufetada amb una truita enorme",
            "ag bualadh $$1 le breac mór",
        ],
    ),
];

fn column(lang: Lang) -> usize {
    match lang {
        Lang::En => 0,
        Lang::Fr => 1,
        Lang::Es => 2,
        Lang::De => 3,
        Lang::It => 4,
        Lang::Va => 5,
        Lang::Ga => 6,
    }
}

/// Texte d'une clé de [`TEXTS`] (anglais si la langue n'est pas encore traduite).
fn tx(lang: Lang, key: &str) -> &'static str {
    match TEXTS.iter().find(|(k, _)| *k == key) {
        Some((_, v)) => {
            let s = v[column(lang)];
            if s.is_empty() { v[0] } else { s }
        }
        None => "",
    }
}

/// Remplace les `{clé}` du modèle par les textes de la langue.
fn fill(template: &str, lang: Lang) -> String {
    let mut out = String::with_capacity(template.len() + 256);
    let mut rest = template;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        match after.find('}') {
            Some(j) if TEXTS.iter().any(|(k, _)| *k == &after[..j]) => {
                out.push_str(tx(lang, &after[..j]));
                rest = &after[j + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Modèle de `popups.mrc`.
const POPUPS: &str = r#"; {popups.comment}
menu status {
  {lusers}:lusers
  {motd}:motd
  {time}:time
  -
  {join}:join $$?="{join.ask}"
  -
  {quit}:quit
}

menu channel {
  {modes}:channel
  {folder}:folder
  -
  {part}:part #
}

menu query {
  {info}:uwho $$1
  {whois}:whois $$1
  {query}:query $$1
  -
  {dcc}
  .{dcc.send}:dcc send $$1
  .{dcc.chat}:dcc chat $$1
  -
  {ignore}:ignore $$1 1 | close -m $$1
  {close}:close -m $$1
}

menu nicklist {
  {info}:uwho $$1
  {whois}:whois $$1
  {query}:query $$1
  -
  {control}
  .{op}:op $$1-
  .{deop}:deop $$1-
  .{voice}:voice $$1-
  .{devoice}:devoice $$1-
  .-
  .{kick}:kick # $$1
  .{kick.why}:kick # $$1 $$?="{reason.ask}"
  .{ban}:ban # $$1 2
  .{ban.kick}:ban -k # $$1 2
  .{ban.kick.why}:ban -k # $$1 2 $$?="{reason.ask}"
  {ctcp}
  .{ping}:ctcp $$1 ping
  .{ctcp.time}:ctcp $$1 time
  .{version}:ctcp $$1 version
  {dcc}
  .{dcc.send}:dcc send $$1
  .{dcc.chat}:dcc chat $$1
  -
  {slap}:me {slap.text}
  -
  {ignore}:ignore $$1 1
}
"#;

/// Modèle de `aliases.mrc`.
const ALIASES: &str = r#"; {aliases.comment}
alias j join $1-
alias p part $1-
alias n names $1-
alias w whois $1-
alias k kick $1-
alias kb ban -k # $$1 2 $2-
alias q query $1-
alias send dcc send $1-
alias chat dcc chat $1-
alias ping ctcp $1 ping
"#;

/// Modèles de `popups.mrc` des versions précédentes (0.9.0 et 0.10.0 : « Infos » faisait un `/whois`).
const OLD_POPUPS: &[&str] = &[POPUPS_0_9];

/// Modèles de `aliases.mrc` des versions précédentes.
const OLD_ALIASES: &[&str] = &[];

const POPUPS_0_9: &str = r#"; {popups.comment}
menu status {
  {lusers}:lusers
  {motd}:motd
  {time}:time
  -
  {join}:join $$?="{join.ask}"
  -
  {quit}:quit
}

menu channel {
  {modes}:channel
  {folder}:folder
  -
  {part}:part #
}

menu query {
  {info}:whois $$1
  {whois}:whois $$1
  {query}:query $$1
  -
  {dcc}
  .{dcc.send}:dcc send $$1
  .{dcc.chat}:dcc chat $$1
  -
  {ignore}:ignore $$1 1 | close -m $$1
  {close}:close -m $$1
}

menu nicklist {
  {info}:whois $$1
  {whois}:whois $$1
  {query}:query $$1
  -
  {control}
  .{op}:op $$1-
  .{deop}:deop $$1-
  .{voice}:voice $$1-
  .{devoice}:devoice $$1-
  .-
  .{kick}:kick # $$1
  .{kick.why}:kick # $$1 $$?="{reason.ask}"
  .{ban}:ban # $$1 2
  .{ban.kick}:ban -k # $$1 2
  .{ban.kick.why}:ban -k # $$1 2 $$?="{reason.ask}"
  {ctcp}
  .{ping}:ctcp $$1 ping
  .{ctcp.time}:ctcp $$1 time
  .{version}:ctcp $$1 version
  {dcc}
  .{dcc.send}:dcc send $$1
  .{dcc.chat}:dcc chat $$1
  -
  {slap}:me {slap.text}
  -
  {ignore}:ignore $$1 1
}
"#;

/// Vrai si `text` est le fichier par défaut `name` tel qu'une version d'AlpIRC l'a écrit (modèle actuel ou ancien,
/// dans n'importe laquelle des sept langues) : l'utilisateur ne l'a pas modifié, on peut le remplacer.
pub fn is_generated(name: &str, text: &str) -> bool {
    let (current, old) = match name {
        n if n == NAMES[0] => (ALIASES, OLD_ALIASES),
        n if n == NAMES[1] => (POPUPS, OLD_POPUPS),
        _ => return false,
    };
    let text = text.replace("\r\n", "\n");
    std::iter::once(current).chain(old.iter().copied()).any(|tpl| Lang::ALL.iter().any(|&l| fill(tpl, l) == text))
}

/// Noms des fichiers par défaut, dans l'ordre de chargement.
pub const NAMES: [&str; 2] = ["aliases.mrc", "popups.mrc"];

/// Fichiers par défaut (nom, texte) dans une langue, dans l'ordre de chargement.
pub fn files(lang: Lang) -> [(&'static str, String); 2] {
    [(NAMES[0], fill(ALIASES, lang)), (NAMES[1], fill(POPUPS, lang))]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_have_no_colon_and_fall_back_to_english() {
        for (k, v) in TEXTS {
            assert!(!v[0].is_empty() && !v[1].is_empty(), "{k} : anglais et français obligatoires");
            if !k.ends_with(".ask") && !k.ends_with(".comment") && !k.ends_with(".text") {
                for s in v {
                    assert!(!s.contains(':') && !s.contains('\n'), "{k} : « {s} »");
                }
            }
        }
        let [(_, a), (_, p)] = files(Lang::En);
        assert!(a.contains("alias kb ban -k # $$1 2 $2-"));
        assert!(p.contains("  Join channel:join $$?=\"Enter channel name:\""));
        assert!(p.contains("  Slap!:me slaps $$1 around a bit with a large trout"));
        for (k, _) in TEXTS {
            assert!(!p.contains(&format!("{{{k}}}")) && !a.contains(&format!("{{{k}}}")), "{k} non remplacé");
        }
        let [(_, _), (_, fr)] = files(Lang::Fr);
        assert!(fr.contains("  .Bannir, expulser (raison):ban -k # $$1 2 $$?=\"Raison :\""), "{fr}");
        assert!(fr.contains("menu nicklist {"));
        let [(_, _), (_, de)] = files(Lang::De);
        assert!(de.contains("  .Bannen, kicken (Grund):ban -k # $$1 2 $$?=\"Grund:\""), "{de}");
        // Les identifiants ($$1…) restent dans chaque traduction.
        for (k, v) in TEXTS {
            for s in v.iter().filter(|s| !s.is_empty()) {
                assert_eq!(v[0].matches('$').count(), s.matches('$').count(), "{k} : « {s} »");
            }
        }
    }

    #[test]
    fn generated_files_are_recognised_in_every_language_and_version() {
        for l in Lang::ALL {
            let [(an, a), (pn, p)] = files(l);
            assert!(is_generated(an, &a) && is_generated(pn, &p), "{l:?}");
            assert!(is_generated(pn, &p.replace('\n', "\r\n")), "fins de ligne Windows");
            // Le fichier des versions 0.9 et 0.10 (Infos faisait un /whois).
            assert!(is_generated(pn, &fill(POPUPS_0_9, l)), "{l:?}");
            assert!(p.contains(":uwho $$1") && !p.contains(&format!("{}:whois", tx(l, "info"))));
        }
        let [_, (pn, p)] = files(Lang::Fr);
        assert!(!is_generated(pn, &p.replace("Claque", "Baffe")), "modifié par l'utilisateur");
        assert!(!is_generated("autre.mrc", &p));
    }

    /// Libellés qui restent ceux de l'anglais dans toutes les langues (commandes IRC).
    const SAME_AS_ENGLISH: &[&str] = &["whois", "dcc", "ctcp", "ping"];

    // Toute nouvelle clé doit être traduite dans les 7 langues (voir l'ADR 0012).
    #[test]
    fn every_text_in_seven_languages() {
        for (k, v) in TEXTS {
            assert!(v.iter().all(|s| !s.is_empty()), "{k} : traduction manquante");
            let copied = v[2..].iter().all(|s| *s == v[0]);
            assert!(SAME_AS_ENGLISH.contains(k) || !copied, "{k} : identique à l'anglais partout");
        }
    }
}
