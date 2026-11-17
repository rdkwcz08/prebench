// app/src-tauri/src/core/components/display.rs — Image d'un écran de composant
// (phase 3, sections 8.1 et 11.3 ; component-api.d.ts, Display).
//
// L'image vit dans le cœur, hors de QuickJS : le modèle décode les commandes
// de l'écran et copie les pixels par blit. Les octets sont gardés dans le
// format de l'écran, exactement comme le modèle les fournit :
//   - Rgb565        : 2 octets par pixel, octet de poids fort d'abord (ordre
//                     du fil d'un ILI9341), lignes de gauche à droite, de haut
//                     en bas ;
//   - Mono1         : 1 bit par pixel, lignes horizontales ; chaque ligne
//                     commence sur un octet ; bit 7 (poids fort) = pixel de
//                     gauche ; 1 = allumé ;
//   - Mono1Vertical : 1 bit par pixel, pages de 8 lignes de haut en bas ; dans
//                     une page, un octet par colonne de gauche à droite ;
//                     bit 0 (poids faible) = ligne du haut de la page ;
//                     1 = allumé (mémoire d'un SSD1306).
//
// Ordre de balayage (Display.setScan, rgb565 seulement) : échange des axes,
// miroirs et ordre rouge/bleu, comme le registre MADCTL d'un ILI9341 ou d'un
// ST7735. Les blits suivants sont donnés dans l'image logique (largeur et
// hauteur échangées si swap_xy) ; le cœur replace chaque pixel dans l'image
// physique, en Rust : en JavaScript, une rotation d'écran entier 240 × 320
// coûtait 3,9 ms et l'échange rouge/bleu 4,6 ms, près du budget de 5 ms d'un
// appel de modèle (mesure du 17 novembre 2026).
//
// Le cœur cumule la zone modifiée (rectangle englobant) et l'envoie à la vue
// au plus une fois par image, en binaire (message ci-dessous, décodé par
// app/ui/lib/displays.svelte.ts). La zone est élargie à l'alignement du
// format (octet en mono1, page en mono1-vertical) : ses octets suivent les
// mêmes règles qu'un blit.
//
// Message « displays » (little-endian), un par image :
//   u16 nombre d'écrans, puis pour chacun :
//   u16 longueur de l'identifiant, identifiant UTF-8,
//   u8 format (0 mono1-vertical, 1 mono1, 2 rgb565), u8 drapeaux (bit 0 :
//   écran neuf ou redimensionné, image noire avant la zone),
//   u16 largeur, u16 hauteur de l'écran,
//   u16 x, u16 y, u16 w, u16 h de la zone, u32 longueur, octets de la zone.

use thiserror::Error;

/// Côté maximal d'un écran, en pixels (tampon borné, règle 16.2.11).
pub const MAX_SIDE: u32 = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayFormat {
    Mono1Vertical,
    Mono1,
    Rgb565,
}

impl DisplayFormat {
    /// Nom utilisé dans component-api.d.ts.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "mono1-vertical" => Some(Self::Mono1Vertical),
            "mono1" => Some(Self::Mono1),
            "rgb565" => Some(Self::Rgb565),
            _ => None,
        }
    }

    /// Code du format dans le message « displays ».
    pub fn code(self) -> u8 {
        match self {
            Self::Mono1Vertical => 0,
            Self::Mono1 => 1,
            Self::Rgb565 => 2,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DisplayError {
    #[error("taille d'écran invalide : {0}×{1} (1 à 1024 pixels par côté)")]
    InvalidSize(u32, u32),
    #[error("format d'écran inconnu : « {0} » (mono1-vertical, mono1 ou rgb565)")]
    UnknownFormat(String),
    #[error("en mono1-vertical, la hauteur ({0}) doit être un multiple de 8")]
    HeightNotPageAligned(u32),
    #[error("en mono1-vertical, y ({0}) et h ({1}) doivent être des multiples de 8")]
    RectNotPageAligned(i64, u32),
    #[error("rectangle de blit trop grand : {0}×{1} (1024 pixels par côté au plus)")]
    RectTooLarge(u32, u32),
    #[error("données trop courtes pour le rectangle : {got} octets, {need} attendus")]
    DataTooShort { got: usize, need: usize },
    #[error("ordre de balayage (setScan) : écrans rgb565 seulement")]
    ScanNeedsRgb565,
}

/// Rectangle en pixels de l'écran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    fn union(self, other: Rect) -> Rect {
        let x0 = self.x.min(other.x);
        let y0 = self.y.min(other.y);
        let x1 = (self.x + self.w).max(other.x + other.w);
        let y1 = (self.y + self.h).max(other.y + other.h);
        Rect {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        }
    }
}

/// Ordre de balayage des blits (component-api.d.ts, DisplayScan).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Scan {
    /// Axes échangés : x logique le long des lignes physiques (paysage).
    pub swap_xy: bool,
    /// x physique = largeur − 1 − x (après échange).
    pub mirror_x: bool,
    /// y physique = hauteur − 1 − y (après échange).
    pub mirror_y: bool,
    /// Rouge et bleu échangés dans chaque pixel RGB565.
    pub bgr: bool,
}

/// Zone modifiée prête à partir vers la vue.
#[derive(Debug, PartialEq, Eq)]
pub struct Update {
    /// Écran neuf ou redimensionné : la vue repart d'une image noire.
    pub reset: bool,
    pub rect: Rect,
    /// Octets de la zone, mêmes règles qu'un blit.
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub struct Framebuffer {
    width: u32,
    height: u32,
    format: DisplayFormat,
    pixels: Vec<u8>,
    dirty: Option<Rect>,
    /// Créé depuis le dernier envoi : la vue doit repartir d'une image noire.
    fresh: bool,
    scan: Scan,
}

impl Framebuffer {
    /// Image noire (tout éteint), entièrement à envoyer.
    pub fn new(width: u32, height: u32, format: DisplayFormat) -> Result<Self, DisplayError> {
        if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
            return Err(DisplayError::InvalidSize(width, height));
        }
        if format == DisplayFormat::Mono1Vertical && !height.is_multiple_of(8) {
            return Err(DisplayError::HeightNotPageAligned(height));
        }
        let len = Self::data_len(format, width, height);
        Ok(Self {
            width,
            height,
            format,
            pixels: vec![0; len],
            dirty: Some(Rect {
                x: 0,
                y: 0,
                w: width,
                h: height,
            }),
            fresh: true,
            scan: Scan::default(),
        })
    }

    /// Ordre de balayage des blits suivants (rgb565 seulement, sauf l'identité).
    pub fn set_scan(&mut self, scan: Scan) -> Result<(), DisplayError> {
        if scan != Scan::default() && self.format != DisplayFormat::Rgb565 {
            return Err(DisplayError::ScanNeedsRgb565);
        }
        self.scan = scan;
        Ok(())
    }

    /// Octets d'un rectangle w × h au format donné (règles de blit).
    pub fn data_len(format: DisplayFormat, w: u32, h: u32) -> usize {
        let (w, h) = (w as usize, h as usize);
        match format {
            DisplayFormat::Rgb565 => w * h * 2,
            DisplayFormat::Mono1 => w.div_ceil(8) * h,
            DisplayFormat::Mono1Vertical => w * (h / 8),
        }
    }

    /// Vérifie un rectangle de blit et renvoie le nombre d'octets attendus.
    pub fn blit_len(&self, y: i64, w: u32, h: u32) -> Result<usize, DisplayError> {
        if w > MAX_SIDE || h > MAX_SIDE {
            return Err(DisplayError::RectTooLarge(w, h));
        }
        if self.format == DisplayFormat::Mono1Vertical
            && (y.rem_euclid(8) != 0 || !h.is_multiple_of(8))
        {
            return Err(DisplayError::RectNotPageAligned(y, h));
        }
        Ok(Self::data_len(self.format, w, h))
    }

    /// Copie un rectangle de pixels (component-api.d.ts, Display.blit). La
    /// partie hors de l'écran est rognée ; des données trop courtes sont une
    /// erreur, sans rien modifier.
    pub fn blit(
        &mut self,
        x: i64,
        y: i64,
        w: u32,
        h: u32,
        data: &[u8],
    ) -> Result<(), DisplayError> {
        let need = self.blit_len(y, w, h)?;
        if data.len() < need {
            return Err(DisplayError::DataTooShort {
                got: data.len(),
                need,
            });
        }
        // Rognage : intersection du rectangle et de l'image logique (axes
        // échangés par setScan).
        let (lw, lh) = if self.scan.swap_xy {
            (self.height, self.width)
        } else {
            (self.width, self.height)
        };
        let x0 = x.max(0);
        let y0 = y.max(0);
        let x1 = (x + i64::from(w)).min(i64::from(lw));
        let y1 = (y + i64::from(h)).min(i64::from(lh));
        if x0 >= x1 || y0 >= y1 {
            return Ok(());
        }
        // Zone réellement modifiée : un firmware qui renvoie la même image
        // ne coûte aucun envoi ni aucun dessin dans l'interface.
        let mut changed: Option<(i64, i64, i64, i64)> = None;
        let mut mark = |px: i64, py: i64| {
            changed = Some(match changed {
                None => (px, py, px, py),
                Some((a, b, c, d)) => (a.min(px), b.min(py), c.max(px), d.max(py)),
            });
        };
        let src_w = w as usize;
        if self.scan != Scan::default() {
            // Image logique -> image physique, pixel par pixel (rgb565).
            let scan = self.scan;
            let (pw, ph) = (i64::from(self.width), i64::from(self.height));
            for ly in y0..y1 {
                for lx in x0..x1 {
                    let i = ((ly - y) as usize * src_w + (lx - x) as usize) * 2;
                    let mut v = u16::from_be_bytes([data[i], data[i + 1]]);
                    if scan.bgr {
                        v = (v & 0x07E0) | (v >> 11) | ((v & 0x1F) << 11);
                    }
                    let (a, b) = if scan.swap_xy { (ly, lx) } else { (lx, ly) };
                    let px = if scan.mirror_x { pw - 1 - a } else { a };
                    let py = if scan.mirror_y { ph - 1 - b } else { b };
                    let (ux, uy) = (px as usize, py as usize);
                    if read(self.format, &self.pixels, self.width as usize, ux, uy) != v {
                        put(
                            self.format,
                            &mut self.pixels,
                            self.width as usize,
                            ux,
                            uy,
                            v,
                        );
                        mark(px, py);
                    }
                }
            }
        } else if self.format == DisplayFormat::Rgb565 {
            // Chemin rapide : comparaison et copie par ligne (trames TFT de 150 Ko).
            let (sx0, row) = (((x0 - x) as usize) * 2, ((x1 - x0) as usize) * 2);
            for py in y0..y1 {
                let src = &data[((py - y) as usize * src_w) * 2 + sx0..][..row];
                let dst = (py as usize * self.width as usize + x0 as usize) * 2;
                let cur = &mut self.pixels[dst..dst + row];
                if cur != src {
                    let first = cur.iter().zip(src).position(|(a, b)| a != b).unwrap_or(0) / 2;
                    let last = (row
                        - 1
                        - cur
                            .iter()
                            .rev()
                            .zip(src.iter().rev())
                            .position(|(a, b)| a != b)
                            .unwrap_or(0))
                        / 2;
                    mark(x0 + first as i64, py);
                    mark(x0 + last as i64, py);
                    cur.copy_from_slice(src);
                }
            }
        } else {
            for py in y0..y1 {
                for px in x0..x1 {
                    let (sx, sy) = ((px - x) as usize, (py - y) as usize);
                    let value = read(self.format, data, src_w, sx, sy);
                    let (ux, uy) = (px as usize, py as usize);
                    if read(self.format, &self.pixels, self.width as usize, ux, uy) != value {
                        put(
                            self.format,
                            &mut self.pixels,
                            self.width as usize,
                            ux,
                            uy,
                            value,
                        );
                        mark(px, py);
                    }
                }
            }
        }
        if let Some((cx0, cy0, cx1, cy1)) = changed {
            let rect = Rect {
                x: cx0 as u32,
                y: cy0 as u32,
                w: (cx1 - cx0 + 1) as u32,
                h: (cy1 - cy0 + 1) as u32,
            };
            self.dirty = Some(self.dirty.map_or(rect, |d| d.union(rect)));
        }
        Ok(())
    }

    /// Valeur d'un pixel : 0 ou 1 en monochrome, valeur RGB565 sur 16 bits.
    pub fn pixel(&self, x: u32, y: u32) -> u16 {
        read(
            self.format,
            &self.pixels,
            self.width as usize,
            x as usize,
            y as usize,
        )
    }

    /// Zone modifiée depuis le dernier appel, élargie à l'alignement du
    /// format, avec ses octets ; None si rien n'a changé.
    pub fn take_update(&mut self) -> Option<Update> {
        let r = self.dirty.take()?;
        let reset = std::mem::take(&mut self.fresh);
        let rect = match self.format {
            DisplayFormat::Rgb565 => r,
            DisplayFormat::Mono1 => {
                let x0 = r.x / 8 * 8;
                let x1 = (r.x + r.w).div_ceil(8) * 8;
                Rect {
                    x: x0,
                    w: x1.min(self.width) - x0,
                    ..r
                }
            }
            DisplayFormat::Mono1Vertical => {
                let y0 = r.y / 8 * 8;
                let y1 = (r.y + r.h).div_ceil(8) * 8;
                Rect {
                    y: y0,
                    h: y1 - y0,
                    ..r
                }
            }
        };
        let (w, h) = (rect.w as usize, rect.h as usize);
        let mut data = vec![0u8; Self::data_len(self.format, rect.w, rect.h)];
        for sy in 0..h {
            for sx in 0..w {
                let v = self.pixel(rect.x + sx as u32, rect.y + sy as u32);
                put(self.format, &mut data, w, sx, sy, v);
            }
        }
        Some(Update { reset, rect, data })
    }
}

fn read(format: DisplayFormat, data: &[u8], w: usize, x: usize, y: usize) -> u16 {
    match format {
        DisplayFormat::Rgb565 => {
            let i = (y * w + x) * 2;
            u16::from_be_bytes([data[i], data[i + 1]])
        }
        DisplayFormat::Mono1 => {
            let byte = data[y * w.div_ceil(8) + x / 8];
            u16::from((byte >> (7 - x % 8)) & 1)
        }
        DisplayFormat::Mono1Vertical => {
            let byte = data[(y / 8) * w + x];
            u16::from((byte >> (y % 8)) & 1)
        }
    }
}

fn put(format: DisplayFormat, data: &mut [u8], w: usize, x: usize, y: usize, value: u16) {
    match format {
        DisplayFormat::Rgb565 => {
            let i = (y * w + x) * 2;
            data[i..i + 2].copy_from_slice(&value.to_be_bytes());
        }
        DisplayFormat::Mono1 => {
            let i = y * w.div_ceil(8) + x / 8;
            let mask = 0x80u8 >> (x % 8);
            if value != 0 {
                data[i] |= mask
            } else {
                data[i] &= !mask
            }
        }
        DisplayFormat::Mono1Vertical => {
            let i = (y / 8) * w + x;
            let mask = 1u8 << (y % 8);
            if value != 0 {
                data[i] |= mask
            } else {
                data[i] &= !mask
            }
        }
    }
}

/// Ajoute la zone modifiée d'un écran au message « displays » (en-tête en
/// tête du message : voir `finish_message`).
pub fn encode_update(out: &mut Vec<u8>, id: &str, fb: &Framebuffer, update: &Update) {
    let id = id.as_bytes();
    let id_len = id.len().min(u16::MAX as usize);
    out.extend_from_slice(&(id_len as u16).to_le_bytes());
    out.extend_from_slice(&id[..id_len]);
    out.push(fb.format.code());
    out.push(u8::from(update.reset));
    for v in [
        fb.width,
        fb.height,
        update.rect.x,
        update.rect.y,
        update.rect.w,
        update.rect.h,
    ] {
        out.extend_from_slice(&(v as u16).to_le_bytes());
    }
    out.extend_from_slice(&(update.data.len() as u32).to_le_bytes());
    out.extend_from_slice(&update.data);
}

/// Message « displays » complet : nombre d'écrans puis leurs zones.
pub fn finish_message(count: u16, records: Vec<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + records.len());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&records);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Écran neuf : tout l'écran part une fois (image noire), puis plus rien.
    fn settled(w: u32, h: u32, format: DisplayFormat) -> Framebuffer {
        let mut fb = Framebuffer::new(w, h, format).expect("écran");
        let first = fb.take_update().expect("écran neuf");
        assert!(first.reset);
        assert_eq!(first.rect, Rect { x: 0, y: 0, w, h });
        assert!(fb.take_update().is_none());
        fb
    }

    #[test]
    fn rgb565_high_byte_first() {
        let mut fb = settled(4, 2, DisplayFormat::Rgb565);
        // Deux pixels : rouge pur (0xF800) puis bleu pur (0x001F), octet fort d'abord.
        fb.blit(1, 1, 2, 1, &[0xF8, 0x00, 0x00, 0x1F])
            .expect("blit");
        assert_eq!(fb.pixel(1, 1), 0xF800);
        assert_eq!(fb.pixel(2, 1), 0x001F);
        assert_eq!(fb.pixel(0, 1), 0);
        let u = fb.take_update().expect("zone");
        assert!(!u.reset);
        assert_eq!(
            u.rect,
            Rect {
                x: 1,
                y: 1,
                w: 2,
                h: 1
            }
        );
        assert_eq!(u.data, vec![0xF8, 0x00, 0x00, 0x1F]);
        assert!(fb.take_update().is_none());
    }

    /// Ordre de balayage : un pixel logique (1, 0) d'une image 3 × 2 tournée
    /// dans un écran physique 2 × 3, pour chaque combinaison utile.
    #[test]
    fn scan_maps_logical_to_physical() {
        let red = [0xF8, 0x00];
        let cases = [
            // (échange, miroir x, miroir y, image logique, pixel physique attendu)
            (false, false, false, (2, 3), (1, 0)),
            (false, true, false, (2, 3), (0, 0)),
            (false, false, true, (2, 3), (1, 2)),
            (true, false, false, (3, 2), (0, 1)),
            (true, true, false, (3, 2), (1, 1)),
            (true, false, true, (3, 2), (0, 1)),
            (true, true, true, (3, 2), (1, 1)),
        ];
        for (swap, mx, my, (lw, lh), (px, py)) in cases {
            let mut fb = settled(2, 3, DisplayFormat::Rgb565);
            let scan = Scan {
                swap_xy: swap,
                mirror_x: mx,
                mirror_y: my,
                bgr: false,
            };
            fb.set_scan(scan).expect("scan");
            // Image logique entière, un seul pixel rouge en (1, 0).
            let mut data = vec![0u8; (lw * lh * 2) as usize];
            data[2..4].copy_from_slice(&red);
            fb.blit(0, 0, lw, lh, &data).expect("blit");
            assert_eq!(fb.pixel(px, py), 0xF800, "{scan:?}");
            let u = fb.take_update().expect("zone");
            assert_eq!(
                u.rect,
                Rect {
                    x: px,
                    y: py,
                    w: 1,
                    h: 1
                },
                "{scan:?}"
            );
        }
        // Avec échange, x logique déborde au-delà de la largeur physique : rogné à l'image logique.
        let mut fb = settled(2, 3, DisplayFormat::Rgb565);
        fb.set_scan(Scan {
            swap_xy: true,
            ..Scan::default()
        })
        .expect("scan");
        fb.blit(2, 1, 4, 1, &[0xFF; 8]).expect("blit");
        assert_eq!(fb.pixel(1, 2), 0xFFFF);
    }

    #[test]
    fn scan_bgr_swaps_red_and_blue() {
        let mut fb = settled(2, 1, DisplayFormat::Rgb565);
        fb.set_scan(Scan {
            bgr: true,
            ..Scan::default()
        })
        .expect("scan");
        fb.blit(0, 0, 2, 1, &[0xF8, 0x00, 0x07, 0xE0])
            .expect("blit");
        assert_eq!(fb.pixel(0, 0), 0x001F, "rouge écrit, bleu affiché");
        assert_eq!(fb.pixel(1, 0), 0x07E0, "vert inchangé");
        let mut mono = settled(8, 8, DisplayFormat::Mono1);
        assert_eq!(
            mono.set_scan(Scan {
                bgr: true,
                ..Scan::default()
            }),
            Err(DisplayError::ScanNeedsRgb565)
        );
        assert_eq!(mono.set_scan(Scan::default()), Ok(()));
    }

    #[test]
    fn mono1_msb_is_left_pixel() {
        let mut fb = settled(10, 2, DisplayFormat::Mono1);
        // Largeur 10 : 2 octets par ligne. 0b1000_0001 = pixels 0 et 7 ;
        // 0b0100_0000 = pixel 9 (deuxième bit du second octet).
        fb.blit(0, 0, 10, 1, &[0b1000_0001, 0b0100_0000])
            .expect("blit");
        let lit: Vec<u32> = (0..10).filter(|&x| fb.pixel(x, 0) == 1).collect();
        assert_eq!(lit, vec![0, 7, 9]);
        assert_eq!(fb.pixel(0, 1), 0);
    }

    #[test]
    fn mono1_update_is_byte_aligned() {
        let mut fb = settled(16, 2, DisplayFormat::Mono1);
        // Pixel 10 seul : zone élargie aux pixels 8 à 15, un octet.
        fb.blit(10, 1, 1, 1, &[0x80]).expect("blit");
        let u = fb.take_update().expect("zone");
        assert_eq!(
            u.rect,
            Rect {
                x: 8,
                y: 1,
                w: 8,
                h: 1
            }
        );
        assert_eq!(u.data, vec![0b0010_0000]);
    }

    #[test]
    fn mono1_vertical_bit0_is_top_row() {
        let mut fb = settled(4, 16, DisplayFormat::Mono1Vertical);
        // Page 1 (lignes 8 à 15), colonne 2 : 0b1000_0001 = lignes 8 et 15.
        fb.blit(2, 8, 1, 8, &[0b1000_0001]).expect("blit");
        let lit: Vec<u32> = (0..16).filter(|&y| fb.pixel(2, y) == 1).collect();
        assert_eq!(lit, vec![8, 15]);
        let u = fb.take_update().expect("zone");
        assert_eq!(
            u.rect,
            Rect {
                x: 2,
                y: 8,
                w: 1,
                h: 8
            }
        );
        assert_eq!(u.data, vec![0b1000_0001]);
        assert_eq!(
            fb.blit(0, 4, 1, 8, &[0]),
            Err(DisplayError::RectNotPageAligned(4, 8))
        );
    }

    #[test]
    fn blit_clips_and_rejects_bad_rectangles() {
        let mut fb = settled(4, 4, DisplayFormat::Rgb565);
        // 2×2 à (3, -1) : seul le pixel source (0, 1) tombe dans l'écran, en (3, 0).
        fb.blit(3, -1, 2, 2, &[0, 1, 0, 2, 0, 3, 0, 4])
            .expect("blit");
        assert_eq!(fb.pixel(3, 0), 3);
        assert_eq!(
            fb.take_update().map(|u| u.rect),
            Some(Rect {
                x: 3,
                y: 0,
                w: 1,
                h: 1
            })
        );
        assert_eq!(
            fb.blit(0, 0, 2, 2, &[0; 7]),
            Err(DisplayError::DataTooShort { got: 7, need: 8 })
        );
        assert_eq!(
            fb.blit(0, 0, 4096, 1, &[]),
            Err(DisplayError::RectTooLarge(4096, 1))
        );
        assert_eq!(
            Framebuffer::new(0, 4, DisplayFormat::Mono1).err(),
            Some(DisplayError::InvalidSize(0, 4))
        );
        assert_eq!(
            Framebuffer::new(8, 12, DisplayFormat::Mono1Vertical).err(),
            Some(DisplayError::HeightNotPageAligned(12))
        );
    }

    #[test]
    fn identical_blit_is_not_dirty() {
        let mut fb = settled(8, 8, DisplayFormat::Mono1Vertical);
        fb.blit(0, 0, 8, 8, &[0, 0, 0xFF, 0, 0, 0, 0, 0])
            .expect("blit");
        // Seule la colonne 2 a changé, même si tout l'écran a été réécrit.
        assert_eq!(
            fb.take_update().map(|u| u.rect),
            Some(Rect {
                x: 2,
                y: 0,
                w: 1,
                h: 8
            })
        );
        fb.blit(0, 0, 8, 8, &[0, 0, 0xFF, 0, 0, 0, 0, 0])
            .expect("blit");
        assert!(fb.take_update().is_none(), "même image : rien à envoyer");
        let mut tft = settled(4, 2, DisplayFormat::Rgb565);
        let mut frame = vec![0u8; 16];
        frame[10..12].copy_from_slice(&[0x12, 0x34]); // pixel (1, 1)
        tft.blit(0, 0, 4, 2, &frame).expect("blit");
        assert_eq!(
            tft.take_update().map(|u| u.rect),
            Some(Rect {
                x: 1,
                y: 1,
                w: 1,
                h: 1
            })
        );
        tft.blit(0, 0, 4, 2, &frame).expect("blit");
        assert!(tft.take_update().is_none());
    }

    #[test]
    fn message_layout() {
        let mut fb = Framebuffer::new(16, 8, DisplayFormat::Mono1Vertical).expect("écran");
        fb.blit(0, 0, 1, 8, &[0xFF]).expect("blit");
        let u = fb.take_update().expect("zone");
        let mut records = Vec::new();
        encode_update(&mut records, "oled1", &fb, &u);
        let msg = finish_message(1, records);
        let mut expected = vec![1, 0, 5, 0];
        expected.extend_from_slice(b"oled1");
        expected.extend_from_slice(&[0, 1]); // mono1-vertical, écran neuf
        expected.extend_from_slice(&[16, 0, 8, 0, 0, 0, 0, 0, 16, 0, 8, 0]);
        expected.extend_from_slice(&[16, 0, 0, 0]);
        let mut data = vec![0u8; 16];
        data[0] = 0xFF;
        expected.extend_from_slice(&data);
        assert_eq!(msg, expected);
    }
}
