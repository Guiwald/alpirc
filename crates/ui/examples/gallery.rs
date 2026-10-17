//! Planche de tous les widgets et de leurs états, à l'échelle 1 et 2 (et en thème sombre).
//!
//! `cargo run -p alpirc-ui --example gallery -- /tmp/gallery.png`
//! (écrit aussi `gallery_2x.png` et `gallery_dark.png` à côté).

use alpirc_ui::switchbar::Activity;
use alpirc_ui::widgets::listview::ListRow;
use alpirc_ui::widgets::*;
use alpirc_ui::*;

const W: i32 = 980;
const H: i32 = 1250;

fn fonts() -> Fonts {
    let f = Fonts::new(
        include_bytes!("../../../assets/fonts/FSEX300.ttf"),
        include_bytes!("../../../assets/fonts/PixelOperator.ttf"),
        include_bytes!("../../../assets/fonts/PixelOperator-Bold.ttf"),
    );
    f.load_system_fallbacks();
    f
}

/// Charge les icônes en pixel art d'`assets/icons` si elles sont là.
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

fn icon(res: &Res, real: &str, fallback: &str) -> Option<IconId> {
    res.icons.get(real).or_else(|| res.icons.get(fallback))
}

/// Titre de section.
fn caption(c: &mut Canvas, res: &Res, x: i32, y: i32, s: &str) {
    res.fonts.draw_text(c, x, y, s, TextStyle::UI_BOLD, res.theme.text);
}

/// Première moitié : les widgets de base.
fn draw_widgets(c: &mut Canvas, res: &Res, p: &mut TestPlatform) -> ComboBox {
    let b = c.bounds();

    // --- Boutons ---
    caption(c, res, 8, 6, "Boutons");
    Button::new("&OK").default_button().paint(c, res, Rect::new(8, 26, 75, 23));
    Button::new("&Annuler").paint(c, res, Rect::new(90, 26, 75, 23));
    let mut bt = Button::new("Appli&quer");
    bt.enabled = false;
    bt.paint(c, res, Rect::new(172, 26, 75, 23));
    let mut bt = Button::new("Focus");
    bt.focused = true;
    bt.paint(c, res, Rect::new(254, 26, 75, 23));
    let mut bt = Button::new("Enfoncé");
    let r = Rect::new(336, 26, 75, 23);
    bt.event(&Input::click(340, 30), r, res, p);
    bt.paint(c, res, r);
    let mut bt = Button::new("").flat().with_icon(icon(res, "toolbar/connect", "connect"));
    let r = Rect::new(418, 26, 24, 23);
    bt.event(&Input::moved(420, 30), r, res, p);
    bt.paint(c, res, r);
    let mut bt = Button::new("").flat().with_icon(icon(res, "toolbar/options", "options"));
    bt.toggled = true;
    bt.paint(c, res, Rect::new(444, 26, 24, 23));
    Button::new("").flat().with_icon(icon(res, "toolbar/chanlist", "list")).paint(c, res, Rect::new(470, 26, 24, 23));
    Button::new("Icône").with_icon(icon(res, "window/status", "status")).paint(c, res, Rect::new(500, 26, 90, 23));

    // --- Cases et radios ---
    caption(c, res, 8, 58, "Cases et boutons radio");
    CheckBox::new("&Gras", true).paint(c, res, Rect::new(8, 78, 120, 18));
    CheckBox::new("Souligné", false).paint(c, res, Rect::new(8, 98, 120, 18));
    let mut cb = CheckBox::new("Grisée", true);
    cb.enabled = false;
    cb.paint(c, res, Rect::new(8, 118, 120, 18));
    let mut cb = CheckBox::new("Focus", false);
    cb.focused = true;
    cb.paint(c, res, Rect::new(8, 138, 120, 18));
    RadioButton::new("&Undernet", true).paint(c, res, Rect::new(140, 78, 120, 18));
    RadioButton::new("EFnet", false).paint(c, res, Rect::new(140, 98, 120, 18));
    let mut rb = RadioButton::new("IRCnet", true);
    rb.enabled = false;
    rb.paint(c, res, Rect::new(140, 118, 120, 18));
    let mut rb = RadioButton::new("Libera", false);
    rb.focused = true;
    rb.paint(c, res, Rect::new(140, 138, 120, 18));

    // --- Cadre, étiquettes, lien ---
    let gb = GroupBox::new("&Cadre de groupe");
    let gr = Rect::new(270, 60, 210, 100);
    gb.paint(c, res, gr);
    let inner = gb.content_rect(res, gr);
    Label::new("Une étiquette sur plusieurs lignes, coupée aux espaces comme sous Windows.").paint(c, res, Rect::new(inner.x, inner.y, inner.w, 50));
    Link::new("http://www.alpirc.org").paint(c, res, Rect::new(inner.x, inner.y + 54, 180, 16));

    // --- Champs ---
    caption(c, res, 490, 58, "Champs de saisie");
    TextEdit::new("Guiwald").paint(c, res, Rect::new(490, 78, 160, 22));
    let mut e = TextEdit::new("secret");
    e.password = true;
    e.paint(c, res, Rect::new(490, 104, 160, 22));
    let mut e = TextEdit::new("sélection active");
    e.focused = true;
    e.core.select(0..10);
    e.paint(c, res, Rect::new(490, 130, 160, 22));
    let mut e = TextEdit::new("lecture seule");
    e.core.read_only = true;
    e.paint(c, res, Rect::new(660, 78, 120, 22));
    let mut e = TextEdit::new("grisé");
    e.enabled = false;
    e.paint(c, res, Rect::new(660, 104, 120, 22));
    let mut e = TextEdit::new("");
    let er = Rect::new(660, 130, 120, 22);
    e.event(&Input::FocusIn, er, res, p);
    e.event(&Input::ImePreedit { text: "にほん".into(), cursor: None }, er, res, p);
    e.paint(c, res, er);
    caption(c, res, 790, 58, "Barre de progression");
    ProgressBar::new(45, 100).paint(c, res, Rect::new(790, 78, 180, 18));
    ProgressBar { value: 80, max: 100, smooth: true }.paint(c, res, Rect::new(790, 102, 180, 18));
    let tip = Tooltip::new("Info-bulle");
    tip.paint(c, res, Rect::new(790, 130, tip.size(res).w, tip.size(res).h));

    // --- Listes ---
    caption(c, res, 8, 168, "Listes, liste à colonnes, arbre, listes déroulantes");
    let mut lb = ListBox::new(SelectMode::Extended);
    lb.set_items(&["#lyon", "#grenoble", "#chambery", "#annecy", "#paris", "#marseille", "#toulouse", "#bordeaux", "#nantes"]);
    lb.items[4].color = Some(0xFF0000);
    lb.sel.click(1, Mods::NONE);
    lb.sel.click(3, Mods::SHIFT);
    lb.focused = true;
    lb.paint(c, res, Rect::new(8, 188, 130, 120));
    let mut lv = ListView::new(vec![Column::new("Canal", 90), Column { title: "Util.".into(), width: 50, align: Align::Right }, Column::new("Sujet", 160)]);
    lv.set_rows(vec![
        ListRow::new(&["#lyon", "42", "Bienvenue sur #lyon !"]),
        ListRow::new(&["#grenoble", "17", "Montagnes et IRC"]),
        ListRow::new(&["#chambery", "8", "Savoie"]),
        ListRow::new(&["#mirc", "230", "Aide mIRC"]),
    ]);
    lv.sort = Some((1, false));
    lv.sel.click(1, Mods::NONE);
    lv.focused = true;
    lv.paint(c, res, Rect::new(146, 188, 260, 120));
    let mut tv = TreeView::new();
    tv.roots = vec![
        TreeNode::new(1, "Undernet").with_icon(icon(res, "window/network_connected", "status")).with_children(vec![
            TreeNode::new(2, "#lyon").with_icon(icon(res, "window/channel", "channel")),
            TreeNode::new(3, "#grenoble").with_icon(icon(res, "window/channel", "channel")),
            TreeNode::new(4, "Elisa").with_icon(icon(res, "window/query", "query")),
        ]),
        TreeNode { expanded: false, ..TreeNode::new(5, "EFnet").with_icon(icon(res, "window/network_disconnected", "status")).with_children(vec![TreeNode::new(6, "#x")]) },
    ];
    tv.selected = Some(3);
    tv.focused = true;
    tv.paint(c, res, Rect::new(414, 188, 160, 120));
    let mut cbx = ComboBox::new(false);
    cbx.set_items(&["Undernet", "EFnet", "IRCnet", "Libera.Chat"]);
    cbx.select(Some(0));
    cbx.paint(c, res, Rect::new(582, 188, 150, 22));
    let mut cbx2 = ComboBox::new(true);
    cbx2.set_items(&["irc.undernet.org", "eu.undernet.org", "us.undernet.org"]);
    cbx2.select(Some(1));
    let cr2 = Rect::new(582, 216, 150, 22);
    cbx2.event(&Input::click(cr2.right() - 6, cr2.y + 6), cr2, b, res, p);
    cbx2.event(&Input::moved(cr2.x + 20, cr2.bottom() + 20), cr2, b, res, p);
    cbx2.paint(c, res, cr2);

    // --- Défilement, onglets, zone de texte ---
    caption(c, res, 8, 316, "Défilement, onglets, zone de texte");
    let mut sb = ScrollBar::new(Orientation::Vertical);
    sb.set_range(0, 100, 25);
    sb.pos = 30;
    sb.paint(c, res, Rect::new(8, 336, 16, 120));
    let mut sb = ScrollBar::new(Orientation::Vertical);
    sb.set_range(0, 10, 20);
    sb.paint(c, res, Rect::new(28, 336, 16, 120));
    let mut sb = ScrollBar::new(Orientation::Horizontal);
    sb.set_range(0, 100, 40);
    sb.pos = 60;
    let sr = Rect::new(52, 336, 200, 16);
    sb.event(&Input::click(sr.right() - 30, sr.y + 4), sr, res, p);
    sb.paint(c, res, sr);
    let mut tabs = Tabs::new(&["&Général", "Connexion", "IRC", "DCC"]);
    tabs.selected = 1;
    tabs.focused = true;
    let tr = Rect::new(52, 360, 260, 98);
    tabs.paint(c, res, tr);
    let pr = tabs.page_rect(res, tr);
    CheckBox::new("Se reconnecter", true).paint(c, res, Rect::new(pr.x + 4, pr.y + 4, 150, 18));
    let mut ta = TextArea::new("; Script AlpIRC\nalias salut {\n\techo -a Salut $1 !\n  msg $chan Bonjour\n}\non *:JOIN:#lyon:{ notice $nick Bienvenue }");
    ta.focused = true;
    ta.core.select(16..30);
    ta.paint(c, res, Rect::new(320, 336, 260, 122));
    cbx2
}

/// Seconde moitié : menus, MDI, boîte de dialogue, barres, discussion.
fn draw_components(c: &mut Canvas, res: &Res, p: &mut TestPlatform) -> (Menu, Editbox, Rect, Toolbar) {
    let t = &res.theme;
    let b = c.bounds();
    let y0 = 470;
    c.etched_hline(0, y0 - 6, W, t);

    // --- Barre de menus, barre d'outils, switchbar ---
    caption(c, res, 8, y0, "Barre de menus, barre d'outils, switchbar, treebar");
    let bar_r = Rect::new(8, y0 + 20, 470, MenuBar::height(res));
    let mut bar = MenuBar::new(vec![
        ("&Fichier".into(), vec![MenuItem::new(1, "&Connecter")]),
        ("&Outils".into(), vec![MenuItem::new(2, "&Options")]),
        ("Fe&nêtre".into(), vec![MenuItem::new(3, "&Cascade")]),
        ("&Aide".into(), vec![MenuItem::new(4, "À &propos")]),
    ]);
    bar.alt_held = true;
    let tr0 = bar.title_rects(res, bar_r)[1];
    bar.event(&Input::moved(tr0.x + 3, tr0.y + 3), bar_r, b, res, p);
    bar.paint(c, res, bar_r);
    let tb_r = Rect::new(8, bar_r.bottom(), 470, Toolbar::height());
    let ti = |id, real: &str, fb: &str, tip: &str| ToolItem::button(id, icon(res, real, fb), tip);
    let mut tb = Toolbar::new(vec![
        ti(1, "toolbar/connect", "connect", "Connecter"),
        ti(2, "toolbar/disconnect", "disconnect", "Déconnecter"),
        ToolItem::separator(),
        ti(3, "toolbar/options", "options", "Options"),
        ToolItem::toggle(4, icon(res, "toolbar/away", "notify"), "Absent", true),
        ti(5, "toolbar/chanlist", "list", "Liste des canaux"),
        ToolItem::separator(),
        ti(6, "toolbar/dcc_send", "dcc", "Envoyer"),
        ti(7, "toolbar/notify", "notify", "Notification"),
        ti(8, "toolbar/urls", "url", "Adresses"),
        ToolItem::separator(),
        ti(9, "toolbar/cascade", "cascade", "Cascade"),
        ti(10, "toolbar/tile_h", "tile", "Mosaïque"),
    ]);
    tb.set_enabled(2, false);
    let hv = tb.item_rects(tb_r)[3].center();
    tb.event(&Input::moved(hv.x, hv.y), tb_r, res, p);
    p.now += 1000;
    tb.event(&Input::Tick, tb_r, res, p);
    tb.paint(c, res, tb_r);
    let mut sw = Switchbar::new();
    sw.items = vec![
        SwitchItem::new(1, "Status", icon(res, "window/status", "status")),
        SwitchItem::new(2, "#lyon", icon(res, "window/channel", "channel")),
        SwitchItem { activity: Activity::Highlight, ..SwitchItem::new(3, "#grenoble", icon(res, "window/channel", "channel")) },
        SwitchItem { activity: Activity::Message, ..SwitchItem::new(4, "Elisa", icon(res, "window/query", "query")) },
        SwitchItem { activity: Activity::Event, ..SwitchItem::new(5, "#chambery", icon(res, "window/channel", "channel")) },
        SwitchItem::new(6, "Liste des canaux", icon(res, "window/chanlist", "list")),
    ];
    sw.active = Some(2);
    let sw_h = sw.height_for(res, 470);
    let sw_r = Rect::new(8, tb_r.bottom(), 470, sw_h);
    sw.paint(c, res, sw_r);
    // Switchbar verticale et treebar.
    let mut swv = sw.clone();
    swv.vertical = true;
    swv.paint(c, res, Rect::new(486, y0 + 20, 130, 150));
    let mut trb = Treebar::new();
    trb.roots = vec![
        TreebarNode::new(1, "Undernet", icon(res, "window/network_connected", "status")).with_children(vec![
            TreebarNode::new(2, "#lyon", icon(res, "window/channel", "channel")),
            TreebarNode { activity: Activity::Highlight, ..TreebarNode::new(3, "#grenoble", icon(res, "window/channel", "channel")) },
            TreebarNode { activity: Activity::Message, ..TreebarNode::new(4, "Elisa", icon(res, "window/query", "query")) },
        ]),
        TreebarNode::new(5, "EFnet", icon(res, "window/network_disconnected", "status")).with_children(vec![TreebarNode {
            activity: Activity::Event,
            ..TreebarNode::new(6, "#chambery", icon(res, "window/channel", "channel"))
        }]),
    ];
    trb.active = Some(2);
    trb.paint(c, res, Rect::new(622, y0 + 20, 150, 150));

    // --- Liste des pseudos ---
    let mut nl = NickList::new();
    let mut entries = vec![NickEntry::new("@", "Guiwald"), NickEntry::new("@", "Kaelis"), NickEntry::new("+", "Elisa"), NickEntry::new("", "Bibi"), NickEntry::new("", "Fred_afk"), NickEntry::new("", "Marmotte"), NickEntry::new("", "Pepito")];
    entries[4].away = true;
    entries[6].color = Some(0x009300);
    nl.set_entries(entries);
    nl.select_nick("Elisa");
    nl.focused = true;
    nl.paint(c, res, Rect::new(780, y0 + 20, 190, 150));

    // --- Zone MDI ---
    let y1 = y0 + 180;
    caption(c, res, 8, y1, "Fenêtres MDI, boîte de dialogue, discussion");
    let area = Rect::new(8, y1 + 20, 420, 300);
    let mut mdi = MdiArea::new();
    mdi.add(MdiChild::new(1, "Status: Guiwald [+i] on Undernet", icon(res, "window/status", "status"), Rect::new(10, 10, 260, 140)));
    mdi.add(MdiChild::new(3, "Elisa", icon(res, "window/query", "query"), Rect::new(0, 0, 200, 100)));
    mdi.add(MdiChild::new(4, "#chambery", icon(res, "window/channel", "channel"), Rect::new(0, 0, 200, 100)));
    mdi.add(MdiChild::new(2, "#lyon [+nt]: Soirée IRL vendredi 20h - nouveau lieu", icon(res, "window/channel", "channel"), Rect::new(120, 70, 280, 170)));
    mdi.minimize(3);
    mdi.minimize(4);
    mdi.activate(2);
    let mut chat = ChatView::new();
    chat.copy_on_select = false;
    let mut src = VecSource::new();
    src.nicks = vec!["Kaelis".into(), "Elisa".into()];
    for (txt, fg) in [
        ("[21:03] * Joins: Elisa (~elisa@lyon.fr)", 3u8),
        ("[21:04] <+Elisa> coucou !", 1),
        ("[21:04] <@Kaelis> salut Elisa, tu viens vendredi ? on sera au Ninkasi vers 20h", 1),
        ("[21:05] * Guiwald boit un café", 6),
        ("[21:05] <Bibi> \x02gras\x02 \x034rouge\x03 \x1Fsouligné\x1F \x1Ditalique\x1D", 1),
        ("[21:06] <Pepito> http://alpirc.org et #grenoble", 1),
        ("[21:06] <@Kaelis> trouvé : le mot cherché", 1),
        ("[21:07] <+Elisa> à plus !", 1),
    ] {
        src.push_colored(txt, fg, false);
    }
    let marker_seq = src.lines[6].0;
    chat.marker = Some(marker_seq);
    mdi.paint(c, res, area, &mut |id, c, r| match id {
        2 => {
            let rr = r;
            chat.set_search(Some("cherché"));
            chat.find(false, res, rr, &src);
            // Sélection à la souris sur la deuxième rangée visible.
            let vis = chat.visible_rows(res, rr, &src);
            if let Some(v) = vis.get(1) {
                let y = v.y + 4;
                chat.event(&Input::click(rr.x + 3 + 8 * 8, y), rr, res, &mut TestPlatform::new(), &src);
                chat.event(&Input::moved(rr.x + 3 + 8 * 15, y + 16), rr, res, &mut TestPlatform::new(), &src);
            }
            chat.paint(c, res, rr, &src);
        }
        1 => {
            c.fill(r, res.theme.chat_background());
            res.fonts.draw_text(c, r.x + 3, r.y + 2, "*** Connecting to irc.undernet.org", TextStyle::CHAT, res.theme.color(2));
            res.fonts.draw_text(c, r.x + 3, r.y + 18, "-irc.undernet.org- *** Found your hostname", TextStyle::CHAT, res.theme.color(5));
        }
        _ => {}
    });

    // --- Boîte de dialogue ---
    use ControlKind::*;
    let spec = DialogSpec {
        title: "Options de connexion".into(),
        width: 250,
        height: 162,
        units: DialogUnits::Dbu,
        controls: vec![
            ControlSpec::new(10, Tab, (4, 4, 242, 136), "&Connexion"),
            ControlSpec::new(11, Tab, (4, 4, 242, 136), "&Identité"),
            ControlSpec::new(12, Tab, (4, 4, 242, 136), "&DCC"),
            ControlSpec::new(1, Text, (12, 26, 40, 9), "&Pseudo :").in_tab(10),
            ControlSpec::new(2, Edit, (56, 24, 80, 12), "Guiwald").in_tab(10),
            ControlSpec::new(3, Text, (12, 42, 40, 9), "&Serveur :").in_tab(10),
            ControlSpec::new(4, Combo, (56, 40, 120, 60), "").items(&["irc.undernet.org", "eu.undernet.org"]).style("drop").in_tab(10),
            ControlSpec::new(5, Box, (12, 58, 110, 46), "Réseau").in_tab(10),
            ControlSpec::new(6, Radio, (18, 68, 90, 10), "&Undernet").style("group").in_tab(10),
            ControlSpec::new(7, Radio, (18, 79, 90, 10), "&EFnet").in_tab(10),
            ControlSpec::new(8, Radio, (18, 90, 90, 10), "&IRCnet").in_tab(10),
            ControlSpec::new(9, Check, (130, 62, 110, 10), "Se &reconnecter").in_tab(10),
            ControlSpec::new(13, Check, (130, 74, 110, 10), "In&visible (+i)").in_tab(10),
            ControlSpec::new(14, Link, (130, 90, 100, 9), "Aide en ligne").in_tab(10),
            ControlSpec::new(15, Progress, (12, 110, 226, 9), "").in_tab(10),
            ControlSpec::new(20, Button, (142, 145, 50, 14), "&OK").style("default ok"),
            ControlSpec::new(21, Button, (196, 145, 50, 14), "Annuler").style("cancel"),
        ],
    };
    let mut dlg = DialogView::new(&spec, res);
    dlg.center_in(Rect::new(436, y1 + 20, 540, 360));
    dlg.set_selection(4, &[0]);
    dlg.set_checked(6, true);
    dlg.set_checked(9, true);
    dlg.set_value(15, 60);
    dlg.paint(c, res);

    // --- Ligne de saisie et palette Ctrl+K ---
    let y2 = y1 + 20 + 360 + 4;
    caption(c, res, 8, y2, "Ligne de saisie (codes de mise en forme, palette Ctrl+K), menu surgissant");
    let mut eb = Editbox::new();
    eb.focused = true;
    eb.set_text("salut \x02tout\x02 le \x1Fmonde\x1F ");
    let eb_r = Rect::new(8, y2 + 90, 420, eb.preferred_height(res));
    eb.event(&Input::key(Key::Char('k'), Mods::CTRL), eb_r, b, res, p);
    eb.paint(c, res, eb_r);

    // --- Menu surgissant avec sous-menu ---
    let items = vec![
        MenuItem::new(1, "&Infos").shortcut("Ctrl+I"),
        MenuItem::new(2, "&Requête"),
        MenuItem::separator(),
        MenuItem::submenu(
            "&Contrôle",
            vec![MenuItem::new(10, "&Op"), MenuItem::new(11, "&Deop"), MenuItem::new(12, "&Voice").checked(true), MenuItem::new(13, "&Bannir").disabled()],
        ),
        MenuItem::submenu("C&TCP", vec![MenuItem::new(20, "&Ping")]),
        MenuItem::new(3, "&Ignorer").checked(true),
        MenuItem::new(4, "&Fermer").disabled(),
        MenuItem::separator(),
        MenuItem::new(5, "Mode &rapide").radio(true),
    ];
    let mut menu = Menu::popup(items, Point::new(450, y2 + 20), b, res);
    let ir = menu.level_rects()[0];
    // Survoler « Contrôle » ouvre le sous-menu.
    menu.event(&Input::moved(ir.x + 20, ir.y + 3 + 2 * 18 + 9 + 4), res, p);
    let sub = menu.level_rects();
    if sub.len() > 1 {
        menu.event(&Input::moved(sub[1].x + 20, sub[1].y + 3 + 2 * 18 + 4), res, p);
    }
    (menu, eb, b, tb)
}

fn render(scale: f32, theme: Theme, path: &str) {
    let mut res = Res::new(fonts());
    load_icons(&mut res.icons);
    res.theme = theme;
    let mut c = Canvas::new((W as f32 * scale) as u32, (H as f32 * scale) as u32, scale);
    let mut p = TestPlatform::new();
    let b = c.bounds();
    c.fill(b, res.theme.face);
    let combo = draw_widgets(&mut c, &res, &mut p);
    let (menu, eb, bounds, tb) = draw_components(&mut c, &res, &mut p);
    // Les surcouches, par-dessus tout.
    combo.paint_popup(&mut c, &res);
    eb.paint_popup(&mut c, &res);
    menu.paint(&mut c, &res);
    tb.paint_tooltip(&mut c, &res, bounds);
    c.to_png(path).expect("écriture du PNG");
    println!("{path}");
}

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "/tmp/gallery.png".to_string());
    let stem = out.trim_end_matches(".png");
    render(1.0, Theme::classic(), &out);
    render(2.0, Theme::classic(), &format!("{stem}_2x.png"));
    render(1.0, Theme::dark(), &format!("{stem}_dark.png"));
}
