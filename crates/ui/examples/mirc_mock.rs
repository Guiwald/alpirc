//! Maquette d'une fenêtre complète façon mIRC (1024x768) : barre de menus, barre d'outils, switchbar à quatre
//! fenêtres, zone MDI avec un canal agrandi (discussion, liste des pseudos, ligne de saisie), à l'échelle 1,
//! 2 et 1,5, en thème sombre, et avec un menu ouvert.
//!
//! `cargo run -p alpirc-ui --example mirc_mock -- /tmp/mirc.png`
//! (écrit aussi `mirc_2x.png`, `mirc_1_5x.png`, `mirc_dark.png` et `mirc_menu.png` à côté).

use alpirc_ui::mdi::{CaptionButton, paint_caption_button};
use alpirc_ui::*;

const W: i32 = 1024;
const H: i32 = 768;

fn fonts() -> Fonts {
    let f = Fonts::new(
        include_bytes!("../../../assets/fonts/FSEX300.ttf"),
        include_bytes!("../../../assets/fonts/PixelOperator.ttf"),
        include_bytes!("../../../assets/fonts/PixelOperator-Bold.ttf"),
    );
    f.load_system_fallbacks();
    f
}

/// Charge les icônes en pixel art d'`assets/icons` si elles sont là (`toolbar/connect`, `window/channel`…).
fn load_icons(icons: &mut Icons) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/icons");
    for sub in ["toolbar", "window", "ui", "app"] {
        let Ok(rd) = std::fs::read_dir(dir.join(sub)) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "png")
                && let (Some(stem), Ok(bytes)) = (p.file_stem().and_then(|s| s.to_str()), std::fs::read(&p)) {
                    icons.register_png(&format!("{sub}/{stem}"), &bytes);
                }
        }
    }
}

/// Icône réelle si présente, sinon celle de secours.
fn icon(res: &Res, real: &str, fallback: &str) -> Option<IconId> {
    res.icons.get(real).or_else(|| res.icons.get(fallback))
}

fn lines() -> VecSource {
    let mut s = VecSource::new();
    s.nicks = ["Kaelis", "Nemo", "Guiwald", "Elisa", "Max", "Bibi", "Marmotte", "Tomtom", "Zaza", "Pepito", "Fred"]
        .iter()
        .map(|n| n.to_string())
        .collect();
    let t = |s: &mut VecSource, txt: &str, fg: u8| {
        s.push_colored(txt, fg, false);
    };
    t(&mut s, "[21:02] * Now talking in #lyon", 3);
    t(&mut s, "[21:02] * Topic is 'Bienvenue sur #lyon ! \x02Soirée IRL\x02 vendredi 20h place des Terreaux - \x0312http://lyon-irc.fr\x03'", 3);
    t(&mut s, "[21:02] * Set by Kaelis on Fri Oct 16 22:14:03", 3);
    t(&mut s, "[21:02] <@Kaelis> salut Guiwald :)", 1);
    t(&mut s, "[21:03] <@Guiwald> lu tout le monde !", 1);
    t(&mut s, "[21:03] * Joins: Elisa (~elisa@ppp-42.lyon.wanadoo.fr)", 3);
    t(&mut s, "[21:03] * Nemo sets mode: +v Elisa", 3);
    t(&mut s, "[21:04] <+Elisa> coucou ! quelqu'un a des nouvelles de Tomtom ?", 1);
    t(&mut s, "[21:04] <@Nemo> il est parti à Grenoble pour le week-end", 1);
    t(&mut s, "[21:05] * Guiwald boit un café ☕", 6);
    t(&mut s, "[21:05] <Bibi> \x02\x034,1 ROUGE SUR NOIR \x03\x02 comme au bon vieux temps de mIRC 5.91", 1);
    t(&mut s, "[21:06] <Marmotte> \x0312b\x034l\x038a\x033b\x0313l\x037a\x02 arc-en-ciel\x02", 1);
    t(&mut s, "[21:06] -Nemo- n'oubliez pas : pas de flood sur le canal svp", 5);
    t(&mut s, "[21:07] <+Max> who's coming to the meetup on friday? I can bring the \x1Fold laptop\x1F with the CRT", 1);
    t(&mut s, "[21:07] <@Kaelis> moi ! je ramène des bugnes et la rosette de Lyon, comme d'hab', et on finit au bouchon de la rue Mercière si ça vous tente", 1);
    t(&mut s, "[21:08] * Quits: Tomtom (Ping timeout: 240 seconds)", 2);
    t(&mut s, "[21:08] <Zaza> lol le timing", 1);
    t(&mut s, "[21:09] * Joins: Pepito (pepito@82-64-12-7.subs.proxad.net)", 3);
    t(&mut s, "[21:09] <Pepito> bonsoir #lyon !", 1);
    t(&mut s, "[21:09] <Pepito> quelqu'un connait un bon client IRC pour Mac ?", 1);
    s.push_colored("[21:10] <@Nemo> Guiwald en code un justement, demande-lui", 1, true);
    t(&mut s, "[21:10] <@Guiwald> AlpIRC : un clone de mIRC en Rust, Mac + Android + Linux", 1);
    t(&mut s, "[21:10] <@Guiwald> le dépôt : https://github.com/Guiwald/alpirc (MIT)", 1);
    t(&mut s, "[21:11] <Pepito> trop bien, avec les couleurs Ctrl+K et tout ?", 1);
    t(&mut s, "[21:11] <@Guiwald> \x02oui\x02, même les scripts mSL et les \x1Dalias\x1D", 1);
    t(&mut s, "[21:12] * Elisa danse la macarena", 6);
    t(&mut s, "[21:12] <Fred> il y a aussi #grenoble et #chambery qui revivent sur Undernet", 1);
    t(&mut s, "[21:13] * Fred is now known as Fred_afk", 3);
    t(&mut s, "[21:13] <Bibi> ^^", 1);
    t(&mut s, "[21:14] * Kaelis changes topic to 'Soirée IRL vendredi 20h - \x034nouveau lieu\x03 : Ninkasi Gerland'", 3);
    t(&mut s, "[21:14] <+Max> \x16inverse video\x16 still works too", 1);
    t(&mut s, "[21:15] <Marmotte> ~~barré~~ \x1Evraiment barré\x1E", 1);
    t(&mut s, "[21:15] * Parts: Zaza (Zaza@lyon.fr) (bonne nuit !)", 3);
    t(&mut s, "[21:16] <Pepito> je teste : 😀 🎉 et un peu de 日本語", 1);
    t(&mut s, "[21:16] <@Nemo> l'emoji passe nickel", 1);
    t(&mut s, "[21:17] <+Elisa> on se retrouve sur #grenoble demain ?", 1);
    t(&mut s, "[21:17] * Nemo sets mode: +b *!*@flood.example.net", 3);
    t(&mut s, "[21:18] <@Kaelis> Guiwald: tu nous fais une démo vendredi ?", 1);
    t(&mut s, "[21:18] <@Guiwald> si ça compile d'ici là :p", 1);
    t(&mut s, "[21:19] <+Max> \x0303,09 green on lime \x03 \x0302,15 navy on grey \x03 \x0F\x02bold\x0F normal", 1);
    t(&mut s, "[21:19] * Joins: Tomtom (tomtom@grenoble.example.org)", 3);
    t(&mut s, "[21:19] <Tomtom> re !", 1);
    s
}

fn nicks() -> Vec<NickEntry> {
    let mut v = vec![NickEntry::new("@", "Guiwald"), NickEntry::new("@", "Kaelis"), NickEntry::new("@", "Nemo"), NickEntry::new("+", "Elisa"), NickEntry::new("+", "Max")];
    for n in ["Bibi", "Chambé", "Dédé", "Fred_afk", "Gros_Minet", "Jojo", "Lulu", "Marmotte", "Pepito", "Rémi", "Sab", "Tomtom", "Vévé", "Zézette"] {
        v.push(NickEntry::new("", n));
    }
    v[8].away = true;
    v
}

fn menus() -> Vec<(String, Vec<MenuItem>)> {
    let m = |id: u32, l: &str| MenuItem::new(id, l);
    vec![
        ("&Fichier".into(), vec![m(1, "&Connecter").shortcut("Ctrl+N"), m(2, "&Déconnecter"), MenuItem::separator(), m(3, "&Quitter")]),
        ("&Affichage".into(), vec![m(10, "&Barre d'outils").checked(true), m(11, "&Switchbar").checked(true), m(12, "&Treebar")]),
        ("Fa&voris".into(), vec![m(20, "&Ajouter")]),
        ("&Outils".into(), vec![m(30, "&Options...").shortcut("Alt+O"), m(31, "&Scripts...").shortcut("Alt+R"), m(32, "&Couleurs...").shortcut("Alt+K")]),
        ("&Commandes".into(), vec![m(40, "&Rejoindre un canal...")]),
        ("Fe&nêtre".into(), vec![m(50, "&Cascade"), m(51, "Mosaïque &horizontale"), m(52, "Mosaïque &verticale")]),
        ("&Aide".into(), vec![m(60, "À &propos d'AlpIRC...")]),
    ]
}

/// Dessine la fenêtre principale (cadre et barre de titre façon Windows 98, comme une capture).
fn draw(c: &mut Canvas, res: &Res, open_menu: bool) {
    let t = &res.theme;
    let mut p = TestPlatform::new();
    let full = Rect::new(0, 0, W, H);
    c.fill(full, t.face);
    c.bevel(full, Bevel::Window, t);
    // Barre de titre de l'application.
    let title = Rect::new(4, 4, W - 8, 18);
    c.gradient_h(title, t.title_active_from, t.title_active_to);
    if let Some(img) = icon(res, "app/alpirc-16", "app").and_then(|i| res.icons.image(i)) {
        c.blit(img, title.x + 1, title.y + 1);
    }
    res.fonts.draw_text(c, title.x + 20, title.y + 1, "AlpIRC - [#lyon [+nt] (19): Soirée IRL vendredi 20h - nouveau lieu : Ninkasi Gerland]", TextStyle::UI_BOLD, t.title_active_text);
    let close = Rect::new(title.right() - 18, title.y + 2, 16, 14);
    paint_caption_button(c, res, close, CaptionButton::Close, false);
    paint_caption_button(c, res, close.offset(-18, 0), CaptionButton::Maximize, false);
    paint_caption_button(c, res, close.offset(-34, 0), CaptionButton::Minimize, false);

    let inner = Rect::new(4, 23, W - 8, H - 27);
    // Zone MDI : un canal agrandi, et trois autres fenêtres derrière.
    let mut mdi = MdiArea::new();
    mdi.add(MdiChild::new(1, "Status: Guiwald [+i] on Undernet (irc.undernet.org:6667)", icon(res, "window/status", "status"), Rect::new(10, 10, 500, 300)));
    mdi.add(MdiChild::new(3, "#grenoble", icon(res, "window/channel", "channel"), Rect::new(60, 60, 500, 300)));
    mdi.add(MdiChild::new(4, "Elisa", icon(res, "window/query", "query"), Rect::new(90, 90, 400, 250)));
    mdi.add(MdiChild::new(2, "#lyon", icon(res, "window/channel", "channel"), Rect::new(30, 30, 600, 400)));
    mdi.maximize(2);

    // Barre de menus (avec les boutons de la fenêtre agrandie).
    let bar_h = MenuBar::height(res);
    let menubar_r = Rect::new(inner.x, inner.y, inner.w, bar_h);
    let mut bar = MenuBar::new(menus());
    let titles_r = Rect::from_corners(menubar_r.x + 20, menubar_r.y, mdi.maximized_controls_rect(menubar_r).map_or(menubar_r.right(), |r| r.x), menubar_r.bottom());
    if open_menu {
        let tr = bar.title_rects(res, titles_r)[3];
        bar.event(&Input::click(tr.x + 3, tr.y + 3), titles_r, full, res, &mut p);
        bar.event(&Input::key(Key::Down, Mods::NONE), titles_r, full, res, &mut p);
    }
    bar.paint(c, res, titles_r);
    c.fill(Rect::new(menubar_r.x, menubar_r.y, 20, bar_h), t.face);
    mdi.paint_maximized_controls(c, res, menubar_r);

    // Barre d'outils.
    let tb_r = Rect::new(inner.x, menubar_r.bottom(), inner.w, Toolbar::height());
    let ti = |id, real: &str, fb: &str, tip: &str| ToolItem::button(id, icon(res, real, fb), tip);
    let mut tb = Toolbar::new(vec![
        ti(1, "toolbar/connect", "connect", "Connecter"),
        ti(2, "toolbar/disconnect", "disconnect", "Déconnecter"),
        ToolItem::separator(),
        ti(3, "toolbar/options", "options", "Options"),
        ti(4, "toolbar/favorites", "folder", "Favoris"),
        ti(5, "toolbar/chanlist", "list", "Liste des canaux"),
        ti(6, "toolbar/scripts", "script", "Scripts"),
        ToolItem::separator(),
        ti(7, "toolbar/dcc_send", "dcc", "Envoyer un fichier"),
        ti(8, "toolbar/dcc_chat", "query", "Discussion DCC"),
        ti(9, "toolbar/transfers", "dcc", "Transferts"),
        ToolItem::separator(),
        ti(10, "toolbar/notify", "notify", "Liste de notification"),
        ti(11, "toolbar/urls", "url", "Adresses"),
        ti(12, "toolbar/abook", "address", "Carnet d'adresses"),
        ti(13, "toolbar/log", "script", "Journaux"),
        ToolItem::separator(),
        ti(14, "toolbar/cascade", "cascade", "Cascade"),
        ti(15, "toolbar/tile_h", "tile", "Mosaïque horizontale"),
        ti(16, "toolbar/tile_v", "tile", "Mosaïque verticale"),
        ToolItem::separator(),
        ti(17, "toolbar/colors", "colors", "Couleurs"),
        ti(18, "toolbar/help", "about", "Aide"),
    ]);
    tb.set_enabled(1, false);
    // Survol du bouton Options (soulevé).
    let hover = tb.item_rects(tb_r)[3].center();
    tb.event(&Input::moved(hover.x, hover.y), tb_r, res, &mut p);
    tb.paint(c, res, tb_r);

    // Switchbar.
    let mut sb = Switchbar::new();
    sb.items = vec![
        SwitchItem::new(1, "Status", icon(res, "window/status", "status")),
        SwitchItem::new(2, "#lyon", icon(res, "window/channel", "channel")),
        SwitchItem { activity: switchbar::Activity::Highlight, ..SwitchItem::new(3, "#grenoble", icon(res, "window/channel", "channel")) },
        SwitchItem { activity: switchbar::Activity::Message, ..SwitchItem::new(4, "Elisa", icon(res, "window/query", "query")) },
    ];
    sb.active = Some(2);
    let sb_h = sb.height_for(res, inner.w);
    let sb_r = Rect::new(inner.x, tb_r.bottom(), inner.w, sb_h);
    sb.paint(c, res, sb_r);

    // Zone MDI.
    let area = Rect::from_corners(inner.x, sb_r.bottom(), inner.right(), inner.bottom());
    let src = lines();
    let mut chat = ChatView::new();
    chat.marker = Some(29);
    let mut nl = NickList::new();
    nl.set_entries(nicks());
    nl.select_nick("Elisa");
    let mut eb = Editbox::new();
    eb.focused = true;
    eb.set_text("j'arrive dans 5 min, \x02promis\x02 \x034,1juré\x0F !");
    mdi.paint(c, res, area, &mut |id, c, r| {
        if id != 2 {
            return;
        }
        let eb_h = eb.preferred_height(res);
        let (eb_r, top) = r.split_bottom(eb_h);
        let (nl_r, chat_r) = top.split_right(130);
        // Séparation entre la discussion et la liste des pseudos.
        let (split, chat_r) = chat_r.split_right(3);
        c.fill(split, res.theme.face);
        chat.paint(c, res, chat_r, &src);
        nl.paint(c, res, nl_r);
        eb.paint(c, res, eb_r);
    });

    bar.paint_popup(c, res);
}

fn render(scale: f32, theme: Theme, path: &str, open_menu: bool) {
    let mut res = Res::new(fonts());
    load_icons(&mut res.icons);
    res.theme = theme;
    let mut c = Canvas::new((W as f32 * scale) as u32, (H as f32 * scale) as u32, scale);
    let t0 = std::time::Instant::now();
    draw(&mut c, &res, open_menu);
    let dt = t0.elapsed();
    c.to_png(path).expect("écriture du PNG");
    println!("{path} ({dt:?})");
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "/tmp/mirc.png".to_string());
    let dir = std::path::Path::new(&out).parent().map(|p| p.to_path_buf()).unwrap_or_default();
    render(1.0, Theme::classic(), &out, false);
    render(2.0, Theme::classic(), dir.join("mirc_2x.png").to_str().unwrap(), false);
    render(1.0, Theme::dark(), dir.join("mirc_dark.png").to_str().unwrap(), false);
    render(1.5, Theme::classic(), dir.join("mirc_1_5x.png").to_str().unwrap(), false);
    render(1.0, Theme::classic(), dir.join("mirc_menu.png").to_str().unwrap(), true);
}
