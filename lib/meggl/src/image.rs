use super::*;
use alloc::vec::Vec;
use byteorder::*;

pub struct ImageLoader;

impl ImageLoader {
    pub fn from_msdib(dib: &[u8]) -> Option<OwnedBitmap> {
        if LE::read_u16(dib) != 0x4D42 {
            return None;
        }
        let bpp = LE::read_u16(&dib[0x1C..0x1E]) as usize;
        match bpp {
            4 | 8 | 24 | 32 => (),
            _ => return None,
        }
        let offset = LE::read_u32(&dib[0x0A..0x0E]) as usize;
        let pal_offset = LE::read_u32(&dib[0x0E..0x12]) as usize + 0x0E;
        let width = LE::read_u32(&dib[0x12..0x16]) as usize;
        let height = LE::read_u32(&dib[0x16..0x1A]) as usize;
        let pal_len = LE::read_u32(&dib[0x2E..0x32]) as usize;
        let bpp8 = (bpp + 7) / 8;
        let stride = (width * bpp8 + 3) & !3;
        let mut vec = Vec::with_capacity(width * height);
        match bpp {
            4 => {
                let palette = &dib[pal_offset..pal_offset + pal_len * 4];
                let width2_f = width / 2;
                let width2_c = (width + 1) / 2;
                let stride = (width2_c + 3) & !3;
                for y in 0..height {
                    let mut src = offset + (height - y - 1) * stride;
                    for _ in 0..width2_f {
                        let c4 = dib[src] as usize;
                        let cl = c4 >> 4;
                        let cr = c4 & 0x0F;
                        vec.push(TrueColor::from_rgb(LE::read_u32(
                            &palette[cl * 4..cl * 4 + 4],
                        )));
                        vec.push(TrueColor::from_rgb(LE::read_u32(
                            &palette[cr * 4..cr * 4 + 4],
                        )));
                        src += bpp8;
                    }
                    if width2_f < width2_c {
                        let c4 = dib[src] as usize;
                        let cl = c4 >> 4;
                        vec.push(TrueColor::from_rgb(LE::read_u32(
                            &palette[cl * 4..cl * 4 + 4],
                        )));
                    }
                }
            }
            8 => {
                let palette = &dib[pal_offset..pal_offset + pal_len * 4];
                for y in 0..height {
                    let mut src = offset + (height - y - 1) * stride;
                    for _ in 0..width {
                        let ic = dib[src] as usize;
                        vec.push(TrueColor::from_rgb(LE::read_u32(
                            &palette[ic * 4..ic * 4 + 4],
                        )));
                        src += bpp8;
                    }
                }
            }
            24 => {
                for y in 0..height {
                    let mut src = offset + (height - y - 1) * stride;
                    for _ in 0..width {
                        let b = dib[src] as u32;
                        let g = dib[src + 1] as u32;
                        let r = dib[src + 2] as u32;
                        vec.push(TrueColor::from_rgb(b + g * 0x100 + r * 0x10000));
                        src += bpp8;
                    }
                }
            }
            32 => {
                for y in 0..height {
                    let mut src = offset + (height - y - 1) * stride;
                    for _ in 0..width {
                        vec.push(TrueColor::from_rgb(LE::read_u32(&dib[src..src + bpp8])));
                        src += bpp8;
                    }
                }
            }
            _ => unreachable!(),
        }
        Some(OwnedBitmap32::from_vec(vec, Size::new(width as isize, height as isize)).into())
    }

    pub fn from_qoi(bytes: &[u8]) -> Option<OwnedBitmap> {
        match rapid_qoi::Qoi::decode_alloc(bytes) {
            Ok((qoi, pixels)) => {
                let count = qoi.width as usize * qoi.height as usize;
                let mut vec = Vec::with_capacity(count);
                if qoi.colors.has_alpha() {
                    let channels = qoi.colors.channels();
                    for i in 0..count * channels {
                        let c = unsafe {
                            let r = *pixels.get_unchecked(i * channels);
                            let g = *pixels.get_unchecked(i * channels + 1);
                            let b = *pixels.get_unchecked(i * channels + 2);
                            let a = *pixels.get_unchecked(i * channels + 3);
                            ColorComponents::from_rgba(r, g, b, a).into_true_color()
                        };
                        vec.push(c);
                    }
                } else {
                    let channels = qoi.colors.channels();
                    for i in 0..count * channels {
                        let c = unsafe {
                            let r = *pixels.get_unchecked(i * channels);
                            let g = *pixels.get_unchecked(i * channels + 1);
                            let b = *pixels.get_unchecked(i * channels + 2);
                            ColorComponents::from_rgb(r, g, b).into_true_color()
                        };
                        vec.push(c);
                    }
                }
                let size = Size::new(qoi.width as isize, qoi.height as isize);
                Some(OwnedBitmap32::from_vec(vec, size).into())
            }
            Err(_) => None,
        }
    }

    pub fn from_qoi_mask(bytes: &[u8]) -> Option<OperationalBitmap> {
        match rapid_qoi::Qoi::decode_alloc(bytes) {
            Ok((qoi, pixels)) => {
                let count = qoi.width as usize * qoi.height as usize;
                let mut vec = Vec::with_capacity(count);
                if !qoi.colors.has_alpha() {
                    return None;
                }
                let channels = qoi.colors.channels();
                for i in 0..count * channels {
                    let a = unsafe { *pixels.get_unchecked(i * channels + 3) };
                    vec.push(a);
                }
                let size = Size::new(qoi.width as isize, qoi.height as isize);
                Some(OperationalBitmap::from_vec(vec, size))
            }
            Err(_) => None,
        }
    }
}
