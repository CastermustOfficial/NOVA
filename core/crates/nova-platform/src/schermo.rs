//! Catturare lo schermo, o una finestra, in un'immagine.
//!
//! Dall'altra parte lo fa `mss` con Pillow: due librerie Python per una
//! chiamata di sistema. Qui e' GDI, che su Windows c'e' sempre: si copia il
//! rettangolo dello schermo in una bitmap e se ne leggono i pixel. Il PNG lo
//! scrive la libreria `png`, che e' Rust e basta.
//!
//! **I pixel sono quelli veri.** Su uno schermo al 150% un processo che non
//! dichiara di sapere dei DPI vede coordinate finte, piu' piccole, e una
//! cattura fatta con quelle taglia via un terzo della finestra. Qui il filo
//! che cattura si dichiara consapevole **per monitor** per il tempo della
//! cattura, e rimette com'era dopo: e' lo stesso che fa `mss`.

use anyhow::Result;

/// Un'immagine catturata: righe dall'alto, tre byte per pixel.
#[derive(Debug, Clone)]
pub struct Immagine {
    pub larghezza: u32,
    pub altezza: u32,
    pub rgb: Vec<u8>,
}

/// Scrive l'immagine in PNG.
pub fn salva_png(percorso: &std::path::Path, im: &Immagine) -> Result<()> {
    let file = std::fs::File::create(percorso)?;
    let mut e = png::Encoder::new(std::io::BufWriter::new(file), im.larghezza, im.altezza);
    e.set_color(png::ColorType::Rgb);
    e.set_depth(png::BitDepth::Eight);
    let mut w = e.write_header()?;
    w.write_image_data(&im.rgb)?;
    w.finish()?;
    Ok(())
}

/// Lo schermo principale, per intero.
pub fn cattura_schermo() -> Result<Immagine> {
    imp::cattura_schermo()
}

/// Il rettangolo di una finestra, com'e' sullo schermo adesso: se un'altra
/// finestra ci sta sopra, si vede quella — come si vedrebbe guardando.
pub fn cattura_finestra(handle: i64) -> Result<Immagine> {
    imp::cattura_finestra(handle)
}

#[cfg(windows)]
mod imp {
    use super::Immagine;
    use anyhow::{anyhow, bail, Result};
    use windows::Win32::Foundation::{HWND, RECT};
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC,
        GetDIBits, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT,
        DIB_RGB_COLORS, ROP_CODE, SRCCOPY,
    };
    use windows::Win32::UI::HiDpi::{
        SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    };
    use windows::Win32::UI::WindowsAndMessaging::{IsIconic, IsWindow};

    /// Fa `f` con il filo consapevole dei DPI, e rimette com'era.
    fn con_i_pixel_veri<T>(f: impl FnOnce() -> Result<T>) -> Result<T> {
        let prima =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        let fatto = f();
        if !prima.0.is_null() {
            unsafe { SetThreadDpiAwarenessContext(prima) };
        }
        fatto
    }

    pub fn cattura_schermo() -> Result<Immagine> {
        con_i_pixel_veri(|| {
            let s = crate::finestre::schermi()?
                .into_iter()
                .next()
                .ok_or_else(|| anyhow!("il sistema non dice di avere nessuno schermo"))?;
            rettangolo(s.x, s.y, s.larghezza, s.altezza)
        })
    }

    pub fn cattura_finestra(handle: i64) -> Result<Immagine> {
        con_i_pixel_veri(|| {
            let h = HWND(handle as *mut core::ffi::c_void);
            if !unsafe { IsWindow(Some(h)) }.as_bool() {
                bail!("la finestra non c'e' piu'");
            }
            // Ridotta a icona sta a -32000: la «cattura» sarebbe un
            // rettangolo di niente, presentato come la finestra.
            if unsafe { IsIconic(h) }.as_bool() {
                bail!("la finestra e' ridotta a icona: portala davanti con app.avanti e riprova");
            }
            // I bordi che DWM disegna, non quelli che la finestra dichiara:
            // questi ultimi comprendono una cornice invisibile di qualche
            // pixel, e la cattura si porterebbe dietro un pezzo di sfondo.
            let mut r = RECT::default();
            unsafe {
                DwmGetWindowAttribute(
                    h,
                    DWMWA_EXTENDED_FRAME_BOUNDS,
                    &mut r as *mut RECT as *mut core::ffi::c_void,
                    std::mem::size_of::<RECT>() as u32,
                )
            }
            .map_err(|e| anyhow!("non riesco a sapere dove sta la finestra: {e}"))?;
            rettangolo(r.left, r.top, r.right - r.left, r.bottom - r.top)
        })
    }

    fn rettangolo(x: i32, y: i32, w: i32, h: i32) -> Result<Immagine> {
        if w <= 0 || h <= 0 {
            bail!("il rettangolo da catturare e' vuoto ({w}x{h})");
        }
        unsafe {
            let schermo = GetDC(None);
            if schermo.is_invalid() {
                bail!("il sistema non mi da' lo schermo da copiare");
            }
            let memoria = CreateCompatibleDC(Some(schermo));
            let bitmap = CreateCompatibleBitmap(schermo, w, h);
            let vecchia = SelectObject(memoria, bitmap.into());
            let copiato = BitBlt(
                memoria,
                0,
                0,
                w,
                h,
                Some(schermo),
                x,
                y,
                ROP_CODE(SRCCOPY.0 | CAPTUREBLT.0),
            );
            SelectObject(memoria, vecchia);
            let mut info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    // Negativo: righe dall'alto, come le vuole il PNG.
                    biHeight: -h,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bgra = vec![0u8; w as usize * h as usize * 4];
            let righe = if copiato.is_ok() {
                GetDIBits(
                    memoria,
                    bitmap,
                    0,
                    h as u32,
                    Some(bgra.as_mut_ptr() as *mut core::ffi::c_void),
                    &mut info,
                    DIB_RGB_COLORS,
                )
            } else {
                0
            };
            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(memoria);
            ReleaseDC(None, schermo);
            copiato.map_err(|e| anyhow!("la copia dello schermo non e' riuscita: {e}"))?;
            if righe != h {
                bail!("dallo schermo sono arrivate {righe} righe su {h}");
            }
            let rgb = bgra
                .chunks_exact(4)
                .flat_map(|p| [p[2], p[1], p[0]])
                .collect();
            Ok(Immagine {
                larghezza: w as u32,
                altezza: h as u32,
                rgb,
            })
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Immagine;
    use anyhow::{bail, Result};

    pub fn cattura_schermo() -> Result<Immagine> {
        bail!(
            "catturare lo schermo su {} non lo so ancora fare",
            std::env::consts::OS
        )
    }

    pub fn cattura_finestra(_handle: i64) -> Result<Immagine> {
        cattura_schermo()
    }
}

#[cfg(test)]
mod prove {
    use super::*;

    #[test]
    fn il_png_si_rilegge_uguale() {
        let im = Immagine {
            larghezza: 2,
            altezza: 1,
            rgb: vec![255, 0, 0, 0, 0, 255],
        };
        let p = std::env::temp_dir().join(format!("nova-png-{}.png", std::process::id()));
        salva_png(&p, &im).unwrap();
        let d = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&p).unwrap()));
        let mut r = d.read_info().unwrap();
        let mut buf = vec![0; r.output_buffer_size()];
        let info = r.next_frame(&mut buf).unwrap();
        assert_eq!((info.width, info.height), (2, 1));
        assert_eq!(&buf[..6], &im.rgb[..]);
        let _ = std::fs::remove_file(p);
    }
}
