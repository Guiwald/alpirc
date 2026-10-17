//! Registre d'icônes : images PNG enregistrées par nom, et icônes de secours dessinées par programme (pour les
//! tests, et tant que les vraies icônes en pixel art ne sont pas fournies).
//!
//! Noms conventionnels utilisés par les composants : `status`, `channel`, `query`, `dcc`, `notify`, `list`,
//! `connect`, `disconnect`, `options`, `address`, `timer`, `colors`, `cascade`, `tile`, `about`, `app`,
//! `folder`, `script`, `url`.

use std::collections::HashMap;

use crate::canvas::Image;

/// Identifiant d'une icône dans un [`Icons`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct IconId(pub u32);

/// Registre d'icônes.
#[derive(Clone, Debug, Default)]
pub struct Icons {
    images: Vec<Image>,
    names: HashMap<String, IconId>,
}

impl Icons {
    /// Registre vide.
    pub fn new() -> Icons {
        Icons::default()
    }

    /// Registre contenant les icônes de secours (voir le module).
    pub fn with_fallbacks() -> Icons {
        let mut icons = Icons::new();
        for (name, art) in FALLBACKS {
            icons.register(name, from_art(art));
        }
        icons
    }

    /// Enregistre (ou remplace) une image sous un nom.
    pub fn register(&mut self, name: &str, img: Image) -> IconId {
        if let Some(&id) = self.names.get(name) {
            self.images[id.0 as usize] = img;
            return id;
        }
        let id = IconId(self.images.len() as u32);
        self.images.push(img);
        self.names.insert(name.to_string(), id);
        id
    }

    /// Décode et enregistre un PNG ; `None` si le PNG est illisible.
    pub fn register_png(&mut self, name: &str, bytes: &[u8]) -> Option<IconId> {
        let img = Image::from_png(bytes)?;
        Some(self.register(name, img))
    }

    /// Identifiant d'une icône par son nom.
    pub fn get(&self, name: &str) -> Option<IconId> {
        self.names.get(name).copied()
    }

    /// Image d'une icône.
    pub fn image(&self, id: IconId) -> Option<&Image> {
        self.images.get(id.0 as usize)
    }

    /// Image d'une icône par son nom.
    pub fn by_name(&self, name: &str) -> Option<&Image> {
        self.get(name).and_then(|id| self.image(id))
    }

    pub fn len(&self) -> usize {
        self.images.len()
    }

    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }
}

fn art_color(c: u8) -> Option<u32> {
    Some(match c {
        b'k' => 0x000000,
        b'w' => 0xFFFFFF,
        b'g' => 0x808080,
        b'l' => 0xC0C0C0,
        b'b' => 0x000080,
        b'B' => 0x0000FF,
        b'c' => 0x008080,
        b'C' => 0x00FFFF,
        b'r' => 0x800000,
        b'R' => 0xFF0000,
        b'y' => 0x808000,
        b'Y' => 0xFFFF00,
        b'v' => 0x008000,
        b'G' => 0x00FF00,
        b'm' => 0x800080,
        b'M' => 0xFF00FF,
        b'o' => 0xFF8000,
        b's' => 0xFFE0A0,
        _ => return None,
    })
}

/// Fabrique une image depuis un dessin en caractères (`.` = transparent ; voir `art_color`).
pub fn from_art(rows: &[&str]) -> Image {
    let h = rows.len() as u32;
    let w = rows.iter().map(|r| r.len()).max().unwrap_or(0) as u32;
    let mut img = Image::new(w, h);
    for (y, row) in rows.iter().enumerate() {
        for (x, c) in row.bytes().enumerate() {
            if let Some(rgb) = art_color(c) {
                img.set_rgb(x as u32, y as u32, rgb);
            }
        }
    }
    img
}

const FALLBACKS: &[(&str, &[&str])] = &[
    (
        "status",
        &[
            "................",
            ".kkkkkkkkkkkkk..",
            ".kwwwwwwwwwwwgk.",
            ".kwbbbbbbbbbwgk.",
            ".kwbCbbbbbbbwgk.",
            ".kwbbbbbbbbbwgk.",
            ".kwbbCCbbbbbwgk.",
            ".kwbbbbbbbbbwgk.",
            ".kwbbbbbbbbbwgk.",
            ".kwwwwwwwwwwwgk.",
            ".kggggggggggggk.",
            "..kkkkkkkkkkkk..",
            ".....kllllk.....",
            "...kkkkkkkkkk...",
            "...kllllllllgk..",
            "...kkkkkkkkkkk..",
        ],
    ),
    (
        "channel",
        &[
            "................",
            "..kkkkkkkkkkkk..",
            ".kwwwwwwwwwwwwk.",
            ".kwwwkwwwkwwwwk.",
            ".kwwwkwwwkwwwwk.",
            ".kwkkkkkkkkkwwk.",
            ".kwwwkwwwkwwwwk.",
            ".kwwkwwwkwwwwwk.",
            ".kkkkkkkkkkwwwk.",
            ".kwwkwwwkwwwwwk.",
            ".kwwwwwwwwwwwwk.",
            "..kkkkkkwwkkkk..",
            ".......kwwk.....",
            "......kwwk......",
            ".....kkkk.......",
            "................",
        ],
    ),
    (
        "query",
        &[
            "................",
            ".....kkkkkk.....",
            "....ksssssk.....",
            "...kssssssskk...",
            "...kskssksssk...",
            "...kssssssssk...",
            "...ksskkksssk...",
            "....kssssssk....",
            ".....kkkkkk.....",
            "...kkBBBBBBkk...",
            "..kBBBBBBBBBBk..",
            ".kBBBBBBBBBBBBk.",
            ".kBBBBBBBBBBBBk.",
            ".kBBBBBBBBBBBBk.",
            ".kkkkkkkkkkkkkk.",
            "................",
        ],
    ),
    (
        "dcc",
        &[
            "................",
            "......k.........",
            ".....kvk........",
            "....kvvvk.......",
            "...kvvvvvk......",
            "..kkkvvvkkk.....",
            "....kvvvk.......",
            "....kvvvk..kkk..",
            "....kvvvk..kRk..",
            "....kkkkk..kRk..",
            "...........kRk..",
            ".........kkkRkkk",
            "..........kRRRk.",
            "...........kRk..",
            "............k...",
            "................",
        ],
    ),
    (
        "connect",
        &[
            "................",
            ".........kkkk...",
            "........kYYYk...",
            ".......kYYYk....",
            "......kYYYk.....",
            ".....kYYYk......",
            "....kYYYkkkkk...",
            "...kYYYYYYYYk...",
            "...kkkkkYYYk....",
            ".......kYYk.....",
            "......kYYk......",
            ".....kYYk.......",
            "....kYk.........",
            "....kk..........",
            "...k............",
            "................",
        ],
    ),
    (
        "disconnect",
        &[
            "................",
            "..kk........kk..",
            "..kRk......kRk..",
            "...kRk....kRk...",
            "....kRk..kRk....",
            ".....kRkkRk.....",
            "......kRRk......",
            "......kRRk......",
            ".....kRkkRk.....",
            "....kRk..kRk....",
            "...kRk....kRk...",
            "..kRk......kRk..",
            "..kk........kk..",
            "................",
            "................",
            "................",
        ],
    ),
    (
        "options",
        &[
            "................",
            "...kkkk.........",
            "..kllllk........",
            "..klggglk.......",
            "...klgggk.......",
            "....kkgggk......",
            "......kgggk.....",
            ".......kgggk....",
            "........kyyyk...",
            ".........kyyyk..",
            "..........kyyyk.",
            "...........kyyk.",
            "............kk..",
            "................",
            "................",
            "................",
        ],
    ),
    (
        "list",
        &[
            "................",
            ".kkkkkkkkkkkkkk.",
            ".kwwwwwwwwwwwwk.",
            ".kwBBwkkkkkkwwk.",
            ".kwwwwwwwwwwwwk.",
            ".kwBBwkkkkkwwwk.",
            ".kwwwwwwwwwwwwk.",
            ".kwBBwkkkkkkkwk.",
            ".kwwwwwwwwwwwwk.",
            ".kwBBwkkkkwwwwk.",
            ".kwwwwwwwwwwwwk.",
            ".kwBBwkkkkkkwwk.",
            ".kwwwwwwwwwwwwk.",
            ".kkkkkkkkkkkkkk.",
            "................",
            "................",
        ],
    ),
    (
        "notify",
        &[
            "................",
            ".......kk.......",
            "......kYYk......",
            ".....kYYYYk.....",
            ".....kYYYYk.....",
            "....kYYYYYYk....",
            "....kYYYYYYk....",
            "....kYYYYYYk....",
            "...kYYYYYYYYk...",
            "...kYYYYYYYYk...",
            "..kYYYYYYYYYYk..",
            "..kkkkkkkkkkkk..",
            "......kyyk......",
            ".......kk.......",
            "................",
            "................",
        ],
    ),
    (
        "address",
        &[
            "................",
            "..kkkkkkkkkkk...",
            "..krrrrrrrrrrk..",
            "..krwwwwwwwwrk..",
            "..krwkkkwwwwrk..",
            "..krwwwwwwwwrk..",
            "..krwkkkkkkwrk..",
            "..krwwwwwwwwrk..",
            "..krwkkkkkwwrk..",
            "..krwwwwwwwwrk..",
            "..krwkkkkkkwrk..",
            "..krwwwwwwwwrk..",
            "..krrrrrrrrrrk..",
            "..kkkkkkkkkkkk..",
            "................",
            "................",
        ],
    ),
    (
        "timer",
        &[
            "................",
            ".....kkkkkk.....",
            "...kkwwwwwwkk...",
            "..kwwwwkwwwwwk..",
            "..kwwwwkwwwwwk..",
            ".kwwwwwkwwwwwwk.",
            ".kwwwwwkwwwwwwk.",
            ".kwwwwwkkkkkwwk.",
            ".kwwwwwwwwwwwwk.",
            ".kwwwwwwwwwwwwk.",
            "..kwwwwwwwwwwk..",
            "..kwwwwwwwwwwk..",
            "...kkwwwwwwkk...",
            ".....kkkkkk.....",
            "................",
            "................",
        ],
    ),
    (
        "colors",
        &[
            "................",
            ".....kkkkkk.....",
            "...kkllllllkk...",
            "..klRRllllBBlk..",
            "..klRRllllBBlk..",
            ".kllllllllllllk.",
            ".klGGllllllYYlk.",
            ".klGGllkkllYYlk.",
            ".kllllkllkllllk.",
            ".kllllkllkllllk.",
            "..kllllkkllMMk..",
            "..kllllllllMMk..",
            "...kkllllllkk...",
            ".....kkkkkk.....",
            "................",
            "................",
        ],
    ),
    (
        "cascade",
        &[
            "kkkkkkkkkk......",
            "kbbbbbbbbk......",
            "kwkkkkkkkkkk....",
            "kwkbbbbbbbbk....",
            "kwkwkkkkkkkkkk..",
            "kwkwkbbbbbbbbk..",
            "kkkwkwwwwwwwwk..",
            "..kwkwwwwwwwwk..",
            "..kkkwwwwwwwwk..",
            "....kwwwwwwwwk..",
            "....kkkkkkkkkk..",
            "................",
            "................",
            "................",
            "................",
            "................",
        ],
    ),
    (
        "tile",
        &[
            "kkkkkkkkkkkkkkk.",
            "kbbbbbbkbbbbbbk.",
            "kwwwwwwkwwwwwwk.",
            "kwwwwwwkwwwwwwk.",
            "kwwwwwwkwwwwwwk.",
            "kwwwwwwkwwwwwwk.",
            "kwwwwwwkwwwwwwk.",
            "kkkkkkkkkkkkkkk.",
            "kbbbbbbkbbbbbbk.",
            "kwwwwwwkwwwwwwk.",
            "kwwwwwwkwwwwwwk.",
            "kwwwwwwkwwwwwwk.",
            "kwwwwwwkwwwwwwk.",
            "kkkkkkkkkkkkkkk.",
            "................",
            "................",
        ],
    ),
    (
        "about",
        &[
            "................",
            ".....kkkkkk.....",
            "...kkBBBBBBkk...",
            "..kBBBBwwBBBBk..",
            "..kBBBBwwBBBBk..",
            ".kBBBBBBBBBBBBk.",
            ".kBBBBwwwBBBBBk.",
            ".kBBBBBwwBBBBBk.",
            ".kBBBBBwwBBBBBk.",
            ".kBBBBBwwBBBBBk.",
            "..kBBBwwwwBBBk..",
            "..kBBBBBBBBBBk..",
            "...kkBBBBBBkk...",
            ".....kkkkkk.....",
            "................",
            "................",
        ],
    ),
    (
        "app",
        &[
            "................",
            ".......kk.......",
            "......kwwk......",
            ".....kwwwwk.....",
            ".....kwwwwk.....",
            "....kwgwwgwk....",
            "...kwggwwggwk...",
            "...kggggggggk...",
            "..kgggkggkgggk..",
            "..kggk.kk.kggk..",
            ".kggk......kggk.",
            ".kgk........kgk.",
            "kgk..........kgk",
            "kk............kk",
            "................",
            "................",
        ],
    ),
    (
        "folder",
        &[
            "................",
            "................",
            "..kkkkk.........",
            ".kYsYsYk........",
            "kYsYsYsYkkkkkk..",
            "kwwwwwwwwwwwwyk.",
            "kwYsYsYsYsYsYyk.",
            "kwsYsYsYsYsYsyk.",
            "kwYsYsYsYsYsYyk.",
            "kwsYsYsYsYsYsyk.",
            "kwYsYsYsYsYsYyk.",
            "kyyyyyyyyyyyyyk.",
            "kkkkkkkkkkkkkkk.",
            "................",
            "................",
            "................",
        ],
    ),
    (
        "script",
        &[
            "................",
            "..kkkkkkkkkk....",
            "..kwwwwwwwwkk...",
            "..kwkkkkkwwkwk..",
            "..kwwwwwwwwkkkk.",
            "..kwkkkkkkkwwwk.",
            "..kwwwwwwwwwwwk.",
            "..kwkkkkkwwwwwk.",
            "..kwwwwwwwwwwwk.",
            "..kwkkkkkkkkwwk.",
            "..kwwwwwwwwwwwk.",
            "..kwkkkkwwwwwwk.",
            "..kwwwwwwwwwwwk.",
            "..kkkkkkkkkkkkk.",
            "................",
            "................",
        ],
    ),
    (
        "url",
        &[
            "................",
            ".....kkkkkk.....",
            "...kkcBBBvckk...",
            "..kcBBvvvBBcck..",
            "..kBBvvvvvBBBk..",
            ".kBBBvvvvBBBBBk.",
            ".kBBBBvvBBBvvBk.",
            ".kBBBBvvBBvvvBk.",
            ".kBBBBBvBBvvvBk.",
            ".kBBBBvvvBBvBBk.",
            "..kBBBvvvBBBBk..",
            "..kcBBBvvBBBck..",
            "...kkcBBBBckk...",
            ".....kkkkkk.....",
            "................",
            "................",
        ],
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry() {
        let mut icons = Icons::with_fallbacks();
        let st = icons.get("status").unwrap();
        assert_eq!(icons.image(st).unwrap().w, 16);
        for (name, art) in FALLBACKS {
            assert_eq!(art.len(), 16, "{name}");
            assert!(art.iter().all(|r| r.len() == 16), "{name}");
        }
        let mut img = Image::new(2, 2);
        img.set_rgb(0, 0, 0xFF0000);
        let id = icons.register("status", img);
        assert_eq!(id, st, "remplacer garde l'identifiant");
        assert_eq!(icons.image(st).unwrap().w, 2);
        let mut c = crate::Canvas::new(2, 2, 1.0);
        c.fill(crate::Rect::new(0, 0, 1, 1), 0x00FF00);
        let png = c.encode_png();
        let id = icons.register_png("x", &png).unwrap();
        assert_eq!(icons.image(id).unwrap().get(0, 0) & 0xFFFFFF, 0x00FF00);
        assert!(icons.register_png("bad", b"nope").is_none());
    }
}
