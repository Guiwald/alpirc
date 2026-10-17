//! Icônes d'AlpIRC, tirées des images en pixel-art de `assets/icons/app/alpirc-<taille>.png` (carrées, transparence
//! permise ; dessinées par `assets/icons/src/draw_icons.py`).
//!
//! - **La plus grande** sert de source (de préférence `alpirc-1024.png`) : réduite pour les tailles inférieures,
//!   agrandie au pixel près pour les supérieures.
//! - **Les autres tailles** (ex. `alpirc-16.png`, `alpirc-32.png`), dessinées à la main, sont prises telles quelles
//!   à leur taille au lieu d'être calculées.
//! - **Icône des notifications Android** : `assets/icons/app/alpirc-notification.png` (silhouette : seule la
//!   transparence compte, la couleur devient blanche). Sans elle, une bulle de discussion dessinée ici.
//! - **Sans image source** (pas encore dessinée), une icône provisoire est dessinée ici, avec un avertissement :
//!   les commandes marchent quand même.
//!
//! Réduction : moyenne par zones (filtre « boîte », alpha prémultiplié), propre pour du pixel-art réduit d'un
//! facteur quelconque ; `--nearest` (ou `ALPIRC_ICON_FILTER=nearest`) prend le plus proche voisin quand la taille
//! divise exactement la source (pixels nets, aucune couleur mélangée).
//!
//! `cargo xtask icons [dossier]` écrit, par défaut dans `assets/icons/generated/` (versionné : à refaire après un
//! changement de la source) :
//!
//! ```text
//! macos/AlpIRC.icns
//! linux/alpirc-16.png … linux/alpirc-512.png
//! android/mipmap-{mdpi,hdpi,xhdpi,xxhdpi,xxxhdpi}/ic_launcher.png, ic_launcher_round.png (48 dp),
//!         ic_launcher_foreground.png (108 dp, calque de l'icône adaptative)
//! android/mipmap-anydpi-v26/ic_launcher.xml, ic_launcher_round.xml (icône adaptative)
//! android/values/ic_launcher_colors.xml (fond de l'icône adaptative)
//! android/drawable-{…}/ic_stat_alpirc.png (24 dp, petite icône blanche des notifications)
//! ```
//!
//! Les commandes de paquet (`bundle-mac`, `apk`, `dist-linux`, `deb`) refont ces images en mémoire depuis la
//! source : elles ne lisent pas ce dossier et ne sont donc jamais en retard sur la source.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::{Res, Result, workspace_root};

/// Tailles des PNG Linux (thème d'icônes « hicolor »).
pub const LINUX_SIZES: [u32; 8] = [16, 24, 32, 48, 64, 128, 256, 512];

/// Densités Android : (nom, facteur × 4) — mdpi = 1, hdpi = 1,5, xhdpi = 2, xxhdpi = 3, xxxhdpi = 4.
const DENSITIES: [(&str, u32); 5] = [("mdpi", 4), ("hdpi", 6), ("xhdpi", 8), ("xxhdpi", 12), ("xxxhdpi", 16)];

/// Part du calque de 108 dp occupée par l'icône dans l'icône adaptative : le lanceur n'en montre que les 72 dp du
/// milieu, découpés en rond sur le Pixel ; 62 dp environ laissent l'icône entière visible, coins compris.
const FOREGROUND_RATIO: f32 = 0.58;

/// Fond de l'icône adaptative : le bleu-vert du bureau de Windows 98.
pub const ADAPTIVE_BACKGROUND: &str = "#008080";

/// Entrées PNG d'un fichier .icns : (type, taille en pixels).
const ICNS_ENTRIES: [(&[u8; 4], u32); 11] = [
    (b"icp4", 16),
    (b"icp5", 32),
    (b"icp6", 64),
    (b"ic07", 128),
    (b"ic08", 256),
    (b"ic09", 512),
    (b"ic10", 1024), // 512 @2x
    (b"ic11", 32),   // 16 @2x
    (b"ic12", 64),   // 32 @2x
    (b"ic13", 256),  // 128 @2x
    (b"ic14", 512),  // 256 @2x
];

/// Image RGBA 8 bits, alpha non prémultiplié.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<[u8; 4]>,
}

impl Image {
    pub fn new(width: u32, height: u32) -> Image {
        Image { width, height, pixels: vec![[0; 4]; (width * height) as usize] }
    }

    fn get(&self, x: u32, y: u32) -> [u8; 4] {
        self.pixels[(y * self.width + x) as usize]
    }

    fn set(&mut self, x: u32, y: u32, c: [u8; 4]) {
        let i = (y * self.width + x) as usize;
        self.pixels[i] = c;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Filter {
    /// Moyenne par zones (toutes tailles).
    Box,
    /// Plus proche voisin quand la taille divise exactement la source, moyenne par zones sinon.
    Nearest,
}

pub fn filter_from_env() -> Filter {
    match std::env::var("ALPIRC_ICON_FILTER").as_deref() {
        Ok("nearest") => Filter::Nearest,
        _ => Filter::Box,
    }
}

/// Redimensionne une image (carrée ou non).
pub fn resize(src: &Image, w: u32, h: u32, filter: Filter) -> Image {
    if (w, h) == (src.width, src.height) {
        return src.clone();
    }
    // Agrandissement (source plus petite) : chaque pixel devient un carré net de k×k ; pour un facteur non entier
    // (32 → 48), agrandissement entier au-dessus (×2) puis moyenne par zones, plutôt que des pixels inégaux.
    if w >= src.width && h >= src.height {
        let k = w.div_ceil(src.width).max(h.div_ceil(src.height));
        let mut big = Image::new(src.width * k, src.height * k);
        for y in 0..big.height {
            for x in 0..big.width {
                big.set(x, y, src.get(x / k, y / k));
            }
        }
        return if (big.width, big.height) == (w, h) { big } else { box_resize(&big, w, h) };
    }
    if filter == Filter::Nearest && src.width.is_multiple_of(w) && src.height.is_multiple_of(h) {
        let (kx, ky) = (src.width / w, src.height / h);
        let mut out = Image::new(w, h);
        for y in 0..h {
            for x in 0..w {
                out.set(x, y, src.get(x * kx + kx / 2, y * ky + ky / 2));
            }
        }
        return out;
    }
    box_resize(src, w, h)
}

/// Poids de chaque pixel source pour chaque pixel cible (recouvrement des intervalles), sur un axe.
fn weights(src: u32, dst: u32) -> Vec<Vec<(usize, f32)>> {
    let scale = src as f64 / dst as f64;
    (0..dst)
        .map(|i| {
            let (a, b) = (i as f64 * scale, (i + 1) as f64 * scale);
            let mut v = Vec::new();
            let mut j = a.floor() as usize;
            while (j as f64) < b && j < src as usize {
                let w = (b.min(j as f64 + 1.0) - a.max(j as f64)) / scale;
                if w > 1e-9 {
                    v.push((j, w as f32));
                }
                j += 1;
            }
            v
        })
        .collect()
}

fn box_resize(src: &Image, w: u32, h: u32) -> Image {
    // Alpha prémultiplié : les pixels transparents ne noircissent pas les bords.
    let pre: Vec<[f32; 4]> = src
        .pixels
        .iter()
        .map(|p| {
            let a = p[3] as f32 / 255.0;
            [p[0] as f32 * a, p[1] as f32 * a, p[2] as f32 * a, p[3] as f32]
        })
        .collect();
    let wx = weights(src.width, w);
    let wy = weights(src.height, h);
    // Passe horizontale : w × hauteur source.
    let mut tmp = vec![[0f32; 4]; (w * src.height) as usize];
    for y in 0..src.height as usize {
        for (x, ws) in wx.iter().enumerate() {
            let mut acc = [0f32; 4];
            for &(j, k) in ws {
                let p = pre[y * src.width as usize + j];
                for c in 0..4 {
                    acc[c] += p[c] * k;
                }
            }
            tmp[y * w as usize + x] = acc;
        }
    }
    // Passe verticale.
    let mut out = Image::new(w, h);
    for (y, ws) in wy.iter().enumerate() {
        for x in 0..w as usize {
            let mut acc = [0f32; 4];
            for &(j, k) in ws {
                let p = tmp[j * w as usize + x];
                for c in 0..4 {
                    acc[c] += p[c] * k;
                }
            }
            let a = acc[3];
            let px = if a <= 0.0 {
                [0, 0, 0, 0]
            } else {
                let f = 255.0 / a;
                let ch = |v: f32| (v * f).round().clamp(0.0, 255.0) as u8;
                [ch(acc[0]), ch(acc[1]), ch(acc[2]), a.round().clamp(0.0, 255.0) as u8]
            };
            out.pixels[y * w as usize + x] = px;
        }
    }
    out
}

/// Place une image au centre d'un carré transparent de côté `size`.
fn centered(img: &Image, size: u32) -> Image {
    let mut out = Image::new(size, size);
    let (ox, oy) = ((size - img.width) / 2, (size - img.height) / 2);
    for y in 0..img.height {
        for x in 0..img.width {
            out.set(ox + x, oy + y, img.get(x, y));
        }
    }
    out
}

pub fn decode_png(bytes: &[u8]) -> Result<Image> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = dec.read_info()?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("PNG trop grand")?];
    let info = reader.next_frame(&mut buf)?;
    let (w, h) = (info.width, info.height);
    let data = &buf[..info.buffer_size()];
    let mut img = Image::new(w, h);
    let n = (w * h) as usize;
    for i in 0..n {
        img.pixels[i] = match info.color_type {
            png::ColorType::Rgba => [data[i * 4], data[i * 4 + 1], data[i * 4 + 2], data[i * 4 + 3]],
            png::ColorType::Rgb => [data[i * 3], data[i * 3 + 1], data[i * 3 + 2], 255],
            png::ColorType::GrayscaleAlpha => [data[i * 2], data[i * 2], data[i * 2], data[i * 2 + 1]],
            png::ColorType::Grayscale => [data[i], data[i], data[i], 255],
            png::ColorType::Indexed => return Err("PNG à palette non développé".into()),
        };
    }
    Ok(img)
}

pub fn encode_png(img: &Image) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, img.width, img.height);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let data: Vec<u8> = img.pixels.iter().flatten().copied().collect();
        enc.write_header()?.write_image_data(&data)?;
    }
    Ok(out)
}

/// Les images de départ : la grande icône, les petites dessinées à la main, la silhouette des notifications.
pub struct Sources {
    pub main: Image,
    hand: BTreeMap<u32, Image>,
    notification: Option<Image>,
    filter: Filter,
}

impl Sources {
    pub fn dir() -> PathBuf {
        workspace_root().join("assets/icons/app")
    }

    /// Lit `assets/icons/app/alpirc-<taille>.png` : la plus grande sert de source (de préférence 1024), les autres
    /// sont prises telles quelles à leur taille. Sans aucune, l'icône provisoire.
    pub fn load(filter: Filter) -> Result<Sources> {
        Self::load_from(&Self::dir(), filter)
    }

    fn load_from(dir: &Path, filter: Filter) -> Result<Sources> {
        let mut hand = BTreeMap::new();
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                let Some(size) = name.strip_prefix("alpirc-").and_then(|n| n.strip_suffix(".png")).and_then(|n| n.parse::<u32>().ok()) else { continue };
                match decode_png(&std::fs::read(e.path())?) {
                    Ok(img) if img.width == size && img.height == size => {
                        hand.insert(size, img);
                    }
                    Ok(img) => eprintln!("attention : {name} fait {}×{} au lieu de {size}×{size} : ignorée", img.width, img.height),
                    Err(err) => eprintln!("attention : {name} illisible ({err}) : ignorée"),
                }
            }
        }
        let main = match hand.keys().next_back().copied() {
            Some(size) => {
                if size < 1024 {
                    eprintln!("(icône : pas de alpirc-1024.png, les grandes tailles sont agrandies depuis alpirc-{size}.png)");
                }
                hand.remove(&size).unwrap()
            }
            None => {
                eprintln!("attention : aucune {}/alpirc-<taille>.png : icône provisoire", dir.display());
                placeholder()
            }
        };
        let notif_path = dir.join("alpirc-notification.png");
        let notification = if notif_path.is_file() { Some(decode_png(&std::fs::read(&notif_path)?)?) } else { None };
        Ok(Sources { main, hand, notification, filter })
    }

    /// L'icône à une taille donnée : dessinée à la main si elle existe, sinon réduite depuis la grande.
    pub fn at(&self, size: u32) -> Image {
        match self.hand.get(&size) {
            Some(img) => img.clone(),
            None => resize(&self.main, size, size, self.filter),
        }
    }

    pub fn png(&self, size: u32) -> Result<Vec<u8>> {
        encode_png(&self.at(size))
    }

    /// Calque avant de l'icône adaptative Android (108 dp) : l'icône réduite, centrée.
    fn foreground(&self, size: u32) -> Image {
        let inner = ((size as f32 * FOREGROUND_RATIO).round() as u32).max(1);
        let inner = inner - (size - inner) % 2; // centrage exact
        centered(&self.at(inner), size)
    }

    /// Petite icône des notifications : silhouette blanche (Android n'en garde que la transparence).
    fn notification(&self, size: u32) -> Image {
        let src = self.notification.clone().unwrap_or_else(notification_placeholder);
        let mut img = resize(&src, size, size, self.filter);
        for p in &mut img.pixels {
            *p = [255, 255, 255, p[3]];
        }
        img
    }
}

/// Fichier .icns (macOS) fait d'entrées PNG, de 16 à 1024 pixels.
pub fn icns(src: &Sources) -> Result<Vec<u8>> {
    let mut body = Vec::new();
    for (tag, size) in ICNS_ENTRIES {
        let p = src.png(size)?;
        body.extend_from_slice(tag);
        body.extend_from_slice(&((p.len() + 8) as u32).to_be_bytes());
        body.extend_from_slice(&p);
    }
    let mut out = b"icns".to_vec();
    out.extend_from_slice(&((body.len() + 8) as u32).to_be_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// Ressources Android des icônes, dans un dossier `res/` (utilisé par `cargo xtask apk` et `cargo xtask icons`).
pub fn write_android_res(src: &Sources, res: &Path) -> Res {
    for (dpi, f4) in DENSITIES {
        let px = |dp: u32| dp * f4 / 4;
        let d = res.join(format!("mipmap-{dpi}"));
        std::fs::create_dir_all(&d)?;
        let legacy = src.png(px(48))?;
        std::fs::write(d.join("ic_launcher.png"), &legacy)?;
        std::fs::write(d.join("ic_launcher_round.png"), &legacy)?;
        std::fs::write(d.join("ic_launcher_foreground.png"), encode_png(&src.foreground(px(108)))?)?;
        let d = res.join(format!("drawable-{dpi}"));
        std::fs::create_dir_all(&d)?;
        std::fs::write(d.join("ic_stat_alpirc.png"), encode_png(&src.notification(px(24)))?)?;
    }
    let d = res.join("mipmap-anydpi-v26");
    std::fs::create_dir_all(&d)?;
    let adaptive = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
<adaptive-icon xmlns:android=\"http://schemas.android.com/apk/res/android\">\n\
    <background android:drawable=\"@color/ic_launcher_background\" />\n\
    <foreground android:drawable=\"@mipmap/ic_launcher_foreground\" />\n\
</adaptive-icon>\n";
    std::fs::write(d.join("ic_launcher.xml"), adaptive)?;
    std::fs::write(d.join("ic_launcher_round.xml"), adaptive)?;
    let d = res.join("values");
    std::fs::create_dir_all(&d)?;
    std::fs::write(
        d.join("ic_launcher_colors.xml"),
        format!("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<resources>\n    <color name=\"ic_launcher_background\">{ADAPTIVE_BACKGROUND}</color>\n</resources>\n"),
    )?;
    Ok(())
}

/// `cargo xtask icons [dossier] [--nearest]`.
pub fn icons_cmd(out: Option<PathBuf>, nearest: bool) -> Res {
    let out = out.unwrap_or_else(|| workspace_root().join("assets/icons/generated"));
    let filter = if nearest { Filter::Nearest } else { filter_from_env() };
    let src = Sources::load(filter)?;
    std::fs::create_dir_all(out.join("macos"))?;
    std::fs::write(out.join("macos/AlpIRC.icns"), icns(&src)?)?;
    std::fs::create_dir_all(out.join("linux"))?;
    for s in LINUX_SIZES {
        std::fs::write(out.join(format!("linux/alpirc-{s}.png")), src.png(s)?)?;
    }
    write_android_res(&src, &out.join("android"))?;
    println!("icônes écrites dans {}", out.display());
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Icône provisoire (en attendant celle dessinée pour AlpIRC) : une bulle de discussion blanche avec un « # » de
// canal, sur un carré bleu-vert à bord marine, façon Windows 98. Dessinée sur une grille de 32×32, agrandie ×32.

const NAVY: [u8; 4] = [0x00, 0x00, 0x80, 0xFF];
const TEAL: [u8; 4] = [0x00, 0x80, 0x80, 0xFF];
const TEAL_LIGHT: [u8; 4] = [0x20, 0xB0, 0xB0, 0xFF];
const TEAL_DARK: [u8; 4] = [0x00, 0x58, 0x58, 0xFF];
const WHITE: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];
const BLACK: [u8; 4] = [0x00, 0x00, 0x00, 0xFF];

fn placeholder_grid() -> Image {
    let mut g = Image::new(32, 32);
    for y in 1..=30u32 {
        for x in 1..=30u32 {
            let (cx, cy) = ((x - 1).min(30 - x), (y - 1).min(30 - y));
            if cx + cy < 2 {
                continue; // coins arrondis
            }
            let c = if cx == 0 || cy == 0 || cx + cy == 2 {
                NAVY
            } else if cx == 1 || cy == 1 || cx + cy == 3 {
                // Relief façon Windows 98 : clair en haut à gauche, sombre en bas à droite.
                if x + y < 31 { TEAL_LIGHT } else { TEAL_DARK }
            } else {
                TEAL
            };
            g.set(x, y, c);
        }
    }
    // Bulle : rectangle blanc à bord noir, coins coupés, queue en bas à gauche.
    for y in 7..=20u32 {
        for x in 6..=25u32 {
            if (x == 6 || x == 25) && (y == 7 || y == 20) {
                continue;
            }
            let edge = x == 6 || x == 25 || y == 7 || y == 20;
            g.set(x, y, if edge { BLACK } else { WHITE });
        }
    }
    for (y, x0, x1) in [(20u32, 9u32, 10u32), (21, 9, 10), (22, 9, 9)] {
        for x in x0..=x1 {
            g.set(x, y, WHITE);
        }
    }
    for (x, y) in [(8u32, 21u32), (11, 21), (8, 22), (10, 22), (8, 23), (9, 23)] {
        g.set(x, y, BLACK);
    }
    // « # » marine.
    for y in 9..=18u32 {
        for x in [13u32, 14, 17, 18] {
            g.set(x, y, NAVY);
        }
    }
    for x in 10..=21u32 {
        for y in [11u32, 12, 15, 16] {
            g.set(x, y, NAVY);
        }
    }
    g
}

/// Icône provisoire en 1024×1024.
pub fn placeholder() -> Image {
    resize(&placeholder_grid(), 1024, 1024, Filter::Nearest)
}

/// Silhouette provisoire des notifications : la bulle pleine, le « # » ajouré (grille 24×24, agrandie ×4).
fn notification_placeholder() -> Image {
    let mut g = Image::new(24, 24);
    for y in 3..=16u32 {
        for x in 2..=21u32 {
            if (x == 2 || x == 21) && (y == 3 || y == 16) {
                continue;
            }
            g.set(x, y, WHITE);
        }
    }
    for (y, x0, x1) in [(17u32, 4u32, 7u32), (18, 4, 6), (19, 4, 5), (20, 4, 4)] {
        for x in x0..=x1 {
            g.set(x, y, WHITE);
        }
    }
    let clear = [0, 0, 0, 0];
    for y in 5..=14u32 {
        for x in [8u32, 9, 13, 14] {
            g.set(x, y, clear);
        }
    }
    for x in 5..=17u32 {
        for y in [7u32, 8, 11, 12] {
            g.set(x, y, clear);
        }
    }
    resize(&g, 96, 96, Filter::Nearest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, c: [u8; 4]) -> Image {
        Image { width: w, height: h, pixels: vec![c; (w * h) as usize] }
    }

    #[test]
    fn moyenne_par_zones() {
        // Damier noir et blanc 2×2 → gris moyen en 1×1.
        let img = Image { width: 2, height: 2, pixels: vec![[0, 0, 0, 255], [255, 255, 255, 255], [255, 255, 255, 255], [0, 0, 0, 255]] };
        let r = resize(&img, 1, 1, Filter::Box);
        assert_eq!(r.pixels[0], [128, 128, 128, 255]);
        // Une couleur unie reste exacte, même pour un rapport non entier (1024 → 48).
        let r = resize(&solid(1024, 1024, [0, 128, 128, 255]), 48, 48, Filter::Box);
        assert!(r.pixels.iter().all(|p| *p == [0, 128, 128, 255]));
    }

    #[test]
    fn transparence_sans_frange_noire() {
        // Moitié rouge opaque, moitié transparente (noir transparent) → rouge à demi transparent, pas bordeaux.
        let img = Image { width: 2, height: 1, pixels: vec![[255, 0, 0, 255], [0, 0, 0, 0]] };
        let r = resize(&img, 1, 1, Filter::Box);
        assert_eq!(r.pixels[0], [255, 0, 0, 128]);
    }

    #[test]
    fn plus_proche_voisin_pour_les_multiples() {
        let img = Image { width: 2, height: 2, pixels: vec![[0, 0, 0, 255], [255, 255, 255, 255], [255, 255, 255, 255], [0, 0, 0, 255]] };
        let big = resize(&img, 8, 8, Filter::Box);
        assert_eq!(big.get(0, 0), [0, 0, 0, 255]);
        assert_eq!(big.get(4, 0), [255, 255, 255, 255]);
        let back = resize(&big, 2, 2, Filter::Nearest);
        assert_eq!(back, img);
        // 8 → 3 ne divise pas : moyenne par zones.
        let r = resize(&big, 3, 3, Filter::Nearest);
        assert_eq!(r.width, 3);
        assert_ne!(r.get(1, 0), [0, 0, 0, 255]);
    }

    #[test]
    fn agrandissement_non_entier() {
        // 2 → 3 : agrandi ×2 au pixel près, puis ramené à 3 par zones (pas de pixels inégaux).
        let img = Image { width: 2, height: 1, pixels: vec![[0, 0, 0, 255], [255, 255, 255, 255]] };
        let r = resize(&img, 3, 3, Filter::Box);
        assert_eq!((r.width, r.height), (3, 3));
        assert_eq!(r.get(0, 0), [0, 0, 0, 255]);
        assert_eq!(r.get(2, 0), [255, 255, 255, 255]);
        assert_eq!(r.get(1, 0), [128, 128, 128, 255]);
    }

    #[test]
    fn tailles_dessinees_a_la_main() {
        let dir = std::env::temp_dir().join(format!("alpirc-app-icons-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let red = solid(32, 32, [255, 0, 0, 255]);
        let blue = solid(16, 16, [0, 0, 255, 255]);
        std::fs::write(dir.join("alpirc-32.png"), encode_png(&red).unwrap()).unwrap();
        std::fs::write(dir.join("alpirc-16.png"), encode_png(&blue).unwrap()).unwrap();
        std::fs::write(dir.join("alpirc-48.png"), encode_png(&blue).unwrap()).unwrap(); // mauvaise taille : ignorée
        let src = Sources::load_from(&dir, Filter::Box).unwrap();
        assert_eq!(src.main.width, 32, "la plus grande sert de source");
        assert_eq!(src.at(16), blue, "16 dessinée à la main");
        assert_eq!(src.at(1024).get(500, 500), [255, 0, 0, 255], "1024 agrandie depuis 32");
        assert_eq!(src.at(48).width, 48);
        let _ = std::fs::remove_dir_all(&dir);
        // Dossier vide : icône provisoire.
        let empty = std::env::temp_dir().join(format!("alpirc-app-icons-vide-{}", std::process::id()));
        assert_eq!(Sources::load_from(&empty, Filter::Box).unwrap().main.width, 1024);
    }

    #[test]
    fn png_aller_retour() {
        let img = placeholder();
        assert_eq!((img.width, img.height), (1024, 1024));
        let small = resize(&img, 48, 48, Filter::Box);
        let back = decode_png(&encode_png(&small).unwrap()).unwrap();
        assert_eq!(back, small);
    }

    #[test]
    fn icns_bien_forme() {
        let src = Sources { main: placeholder(), hand: BTreeMap::new(), notification: None, filter: Filter::Box };
        let data = icns(&src).unwrap();
        assert_eq!(&data[..4], b"icns");
        assert_eq!(u32::from_be_bytes(data[4..8].try_into().unwrap()) as usize, data.len());
        // Parcours des entrées : chacune est un PNG de la taille annoncée.
        let mut i = 8;
        let mut n = 0;
        while i < data.len() {
            let tag = &data[i..i + 4];
            let len = u32::from_be_bytes(data[i + 4..i + 8].try_into().unwrap()) as usize;
            let img = decode_png(&data[i + 8..i + len]).unwrap();
            let (_, size) = ICNS_ENTRIES.iter().find(|(t, _)| t.as_slice() == tag).unwrap();
            assert_eq!(img.width, *size);
            i += len;
            n += 1;
        }
        assert_eq!(n, ICNS_ENTRIES.len());
    }

    #[test]
    fn calque_adaptatif_centre() {
        let src = Sources { main: solid(64, 64, [255, 0, 0, 255]), hand: BTreeMap::new(), notification: None, filter: Filter::Box };
        let fg = src.foreground(108);
        assert_eq!(fg.width, 108);
        assert_eq!(fg.get(0, 0)[3], 0, "bord transparent");
        assert_eq!(fg.get(54, 54), [255, 0, 0, 255], "icône au centre");
        // Symétrie : autant de marge à gauche qu'à droite.
        let left = (0..108).find(|&x| fg.get(x, 54)[3] > 0).unwrap();
        let right = (0..108).rev().find(|&x| fg.get(x, 54)[3] > 0).unwrap();
        assert_eq!(left, 107 - right);
        let n = src.notification(48);
        assert!(n.pixels.iter().all(|p| p[0] == 255 && p[1] == 255 && p[2] == 255));
        assert!(n.pixels.iter().any(|p| p[3] == 255) && n.pixels.iter().any(|p| p[3] == 0));
    }

    #[test]
    fn ressources_android() {
        let dir = std::env::temp_dir().join(format!("alpirc-icons-{}", std::process::id()));
        let src = Sources { main: placeholder(), hand: BTreeMap::new(), notification: None, filter: Filter::Box };
        write_android_res(&src, &dir).unwrap();
        let fg = decode_png(&std::fs::read(dir.join("mipmap-xxxhdpi/ic_launcher_foreground.png")).unwrap()).unwrap();
        assert_eq!(fg.width, 432);
        let l = decode_png(&std::fs::read(dir.join("mipmap-hdpi/ic_launcher.png")).unwrap()).unwrap();
        assert_eq!(l.width, 72);
        let n = decode_png(&std::fs::read(dir.join("drawable-xxhdpi/ic_stat_alpirc.png")).unwrap()).unwrap();
        assert_eq!(n.width, 72);
        assert!(std::fs::read_to_string(dir.join("mipmap-anydpi-v26/ic_launcher_round.xml")).unwrap().contains("@mipmap/ic_launcher_foreground"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
