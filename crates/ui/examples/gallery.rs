//! Planche de tous les widgets et de leurs états, à l'échelle 1 et 2 (et en thème sombre).
//!
//! `cargo run -p alpirc-ui --example gallery -- /tmp/gallery.png`
//! (écrit aussi `gallery_2x.png` et `gallery_dark.png` à côté).

use alpirc_ui::widgets::*;
use alpirc_ui::*;

fn fonts() -> Fonts {
    let f = Fonts::new(
        include_bytes!("../../../assets/fonts/FSEX300.ttf"),
        include_bytes!("../../../assets/fonts/PixelOperator.ttf"),
        include_bytes!("../../../assets/fonts/PixelOperator-Bold.ttf"),
    );
    f.load_system_fallbacks();
    f
}

/// Titre de section.
fn caption(c: &mut Canvas, res: &Res, x: i32, y: i32, s: &str) {
    res.fonts.draw_text(c, x, y, s, TextStyle::UI_BOLD, res.theme.text);
}

fn draw(c: &mut Canvas, res: &Res) {
    let t = res.theme.clone();
    let b = c.bounds();
    c.fill(b, t.face);
    let mut p = TestPlatform::new();

    // --- Boutons ---
    caption(c, res, 8, 6, "Boutons");
    let mut bt = Button::new("&OK").default_button();
    bt.paint(c, res, Rect::new(8, 26, 75, 23));
    let bt = Button::new("&Annuler");
    bt.paint(c, res, Rect::new(90, 26, 75, 23));
    let mut bt = Button::new("Appli&quer");
    bt.enabled = false;
    bt.paint(c, res, Rect::new(172, 26, 75, 23));
    let mut bt = Button::new("Focus");
    bt.focused = true;
    bt.paint(c, res, Rect::new(254, 26, 75, 23));
    let mut bt = Button::new("Enfoncé");
    let r = Rect::new(336, 26, 75, 23);
    bt.event(&Input::click(340, 30), r, res, &mut p);
    bt.paint(c, res, r);
    let mut bt = Button::new("").flat().with_icon(res.icons.get("connect"));
    let r = Rect::new(418, 26, 24, 23);
    bt.event(&Input::moved(420, 30), r, res, &mut p);
    bt.paint(c, res, r);
    let mut bt = Button::new("").flat().with_icon(res.icons.get("options"));
    bt.toggled = true;
    bt.paint(c, res, Rect::new(444, 26, 24, 23));
    let bt = Button::new("").flat().with_icon(res.icons.get("channel"));
    bt.paint(c, res, Rect::new(470, 26, 24, 23));
    let mut bt = Button::new("Icône").with_icon(res.icons.get("status"));
    bt.enabled = true;
    bt.paint(c, res, Rect::new(500, 26, 90, 23));

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
    let mut e = TextEdit::new("Guiwald");
    e.paint(c, res, Rect::new(490, 78, 160, 22));
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
    e.focused = true;
    e.event(&Input::FocusIn, Rect::new(660, 130, 120, 22), res, &mut p);
    e.event(&Input::ImePreedit { text: "にほん".into(), cursor: None }, Rect::new(660, 130, 120, 22), res, &mut p);
    e.paint(c, res, Rect::new(660, 130, 120, 22));

    // --- Listes ---
    caption(c, res, 8, 168, "Listes");
    let mut lb = ListBox::new(SelectMode::Extended);
    lb.set_items(&["#lyon", "#grenoble", "#chambery", "#annecy", "#paris", "#marseille", "#toulouse", "#bordeaux", "#nantes"]);
    lb.items[2].color = Some(0xFF0000);
    lb.sel.click(1, Mods::NONE);
    lb.sel.click(3, Mods::SHIFT);
    lb.focused = true;
    lb.paint(c, res, Rect::new(8, 188, 130, 120));
    let mut lv = alpirc_ui::widgets::ListView::new(vec![
        Column::new("Canal", 90),
        Column { title: "Util.".into(), width: 50, align: Align::Right },
        Column::new("Sujet", 160),
    ]);
    lv.set_rows(vec![
        alpirc_ui::widgets::listview::ListRow::new(&["#lyon", "42", "Bienvenue sur #lyon !"]),
        alpirc_ui::widgets::listview::ListRow::new(&["#grenoble", "17", "Montagnes et IRC"]),
        alpirc_ui::widgets::listview::ListRow::new(&["#chambery", "8", "Savoie"]),
        alpirc_ui::widgets::listview::ListRow::new(&["#mirc", "230", "Aide mIRC"]),
    ]);
    lv.sort = Some((1, false));
    lv.sel.click(1, Mods::NONE);
    lv.focused = true;
    lv.paint(c, res, Rect::new(146, 188, 260, 120));
    let mut tv = TreeView::new();
    tv.roots = vec![
        TreeNode::new(1, "Undernet").with_icon(res.icons.get("status")).with_children(vec![
            TreeNode::new(2, "#lyon").with_icon(res.icons.get("channel")),
            TreeNode::new(3, "#grenoble").with_icon(res.icons.get("channel")),
            TreeNode::new(4, "Elisa").with_icon(res.icons.get("query")),
        ]),
        TreeNode { expanded: false, ..TreeNode::new(5, "EFnet").with_icon(res.icons.get("status")).with_children(vec![TreeNode::new(6, "#x")]) },
    ];
    tv.selected = Some(3);
    tv.focused = true;
    tv.paint(c, res, Rect::new(414, 188, 160, 120));

    // --- Liste déroulante ---
    let mut cbx = ComboBox::new(false);
    cbx.set_items(&["Undernet", "EFnet", "IRCnet", "Libera.Chat"]);
    cbx.select(Some(0));
    let cr = Rect::new(582, 188, 150, 22);
    cbx.paint(c, res, cr);
    let mut cbx2 = ComboBox::new(true);
    cbx2.set_items(&["irc.undernet.org", "eu.undernet.org", "us.undernet.org"]);
    cbx2.select(Some(1));
    let cr2 = Rect::new(582, 216, 150, 22);
    cbx2.event(&Input::click(cr2.right() - 6, cr2.y + 6), cr2, b, res, &mut p);
    cbx2.event(&Input::moved(cr2.x + 20, cr2.bottom() + 20), cr2, b, res, &mut p);
    cbx2.paint(c, res, cr2);

    // --- Barres de défilement, progression ---
    caption(c, res, 8, 316, "Défilement, progression, onglets");
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
    sb.paint(c, res, Rect::new(52, 336, 200, 16));
    ProgressBar::new(45, 100).paint(c, res, Rect::new(52, 360, 200, 18));
    ProgressBar { value: 80, max: 100, smooth: true }.paint(c, res, Rect::new(52, 384, 200, 18));
    let mut tabs = Tabs::new(&["&Général", "Connexion", "IRC", "DCC"]);
    tabs.selected = 1;
    tabs.focused = true;
    let tr = Rect::new(262, 336, 260, 120);
    tabs.paint(c, res, tr);
    let pr = tabs.page_rect(res, tr);
    CheckBox::new("Se reconnecter", true).paint(c, res, Rect::new(pr.x + 4, pr.y + 4, 150, 18));

    // --- Zone de texte ---
    let mut ta = TextArea::new("; Script AlpIRC\nalias salut {\n\techo -a Salut $1 !\n  msg $chan Bonjour\n}\non *:JOIN:#lyon:{ notice $nick Bienvenue }");
    ta.focused = true;
    ta.core.select(16..30);
    ta.paint(c, res, Rect::new(530, 250, 210, 120));

    // --- Info-bulle ---
    let tip = Tooltip::new("Se connecter au serveur");
    let tr = tip.rect_at(res, Point::new(530, 380), b);
    tip.paint(c, res, tr);

    // La liste ouverte, par-dessus tout.
    cbx2.paint_popup(c, res);
}

fn render(scale: f32, theme: Theme, path: &str) {
    let mut res = Res::new(fonts());
    res.theme = theme;
    let (w, h) = (760, 470);
    let mut c = Canvas::new((w as f32 * scale) as u32, (h as f32 * scale) as u32, scale);
    draw(&mut c, &res);
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
