use alloc::vec::Vec;
use byteorder::*;
use megstd::drawing::*;
use png_decoder;
use zune_jpeg::JpegDecoder;

pub struct ImageLoader;

impl ImageLoader {
    pub fn load(blob: &[u8]) -> Result<OwnedBitmap32, DecodeError> {
        let drivers = [
            Self::_from_png,
            Self::_from_jpeg,
            Self::_from_qoi,
            Self::_from_mpic,
            Self::_from_msdib,
        ];
        for driver in drivers {
            match driver(blob) {
                Err(DecodeError::NotSupported) => continue,
                Ok(v) => return Ok(v),
                Err(err) => return Err(err),
            }
        }
        Err(DecodeError::NotSupported)
    }

    #[inline]
    fn _from_png(blob: &[u8]) -> Result<OwnedBitmap32, DecodeError> {
        png_decoder::decode(blob)
            .map_err(|err| match err {
                png_decoder::DecodeError::InvalidMagicBytes => DecodeError::NotSupported,
                _ => DecodeError::InvalidData,
            })
            .map(|(header, pixels)| {
                let size = Size::new(header.width as isize, header.height as isize);
                OwnedBitmap32::from_vec_rgba(pixels, size)
            })
    }

    #[inline]
    fn _from_jpeg(blob: &[u8]) -> Result<OwnedBitmap32, DecodeError> {
        let mut decoder = JpegDecoder::new(blob);
        decoder
            .decode_headers()
            .map_err(|_| DecodeError::NotSupported)?;
        let info = decoder.info().ok_or(DecodeError::InvalidData)?;
        let pixels = decoder.decode().map_err(|_| DecodeError::InvalidData)?;
        OwnedBitmap32::from_bytes_rgb(
            &pixels,
            Size::new(info.width as isize, info.height as isize),
        )
        .ok_or(DecodeError::InvalidData)
    }

    #[inline]
    fn _from_qoi(blob: &[u8]) -> Result<OwnedBitmap32, DecodeError> {
        rapid_qoi::Qoi::decode_alloc(blob)
            .map_err(|err| match err {
                rapid_qoi::DecodeError::NotEnoughData => DecodeError::OutOfMemory,
                rapid_qoi::DecodeError::InvalidMagic => DecodeError::NotSupported,
                _ => DecodeError::InvalidData,
            })
            .and_then(|(qoi, pixels)| {
                let size = Size::new(qoi.width as isize, qoi.height as isize);
                if qoi.colors.has_alpha() {
                    Some(OwnedBitmap32::from_vec_rgba(pixels, size))
                } else {
                    OwnedBitmap32::from_bytes_rgb(&pixels, size)
                }
                .ok_or(DecodeError::InvalidData)
            })
    }

    #[inline]
    fn _from_mpic(blob: &[u8]) -> Result<OwnedBitmap32, DecodeError> {
        let decoder = mpic::Decoder::<()>::new(blob).ok_or(DecodeError::NotSupported)?;
        let info = decoder.info();
        let pixels = decoder
            .decode_rgba()
            .map_err(|_| DecodeError::InvalidData)?;
        Ok(OwnedBitmap32::from_vec_rgba(
            pixels,
            Size::new(info.width() as isize, info.height() as isize),
        ))
    }

    #[inline]
    fn _from_msdib(blob: &[u8]) -> Result<OwnedBitmap32, DecodeError> {
        (LE::read_u16(blob) == 0x4D42)
            .then(|| ())
            .ok_or(DecodeError::NotSupported)?;

        let bpp = LE::read_u16(&blob[0x1C..0x1E]) as usize;
        matches!(bpp, 4 | 8 | 24 | 32)
            .then(|| ())
            .ok_or(DecodeError::InvalidData)?;

        let offset = LE::read_u32(&blob[0x0A..0x0E]) as usize;
        let pal_offset = LE::read_u32(&blob[0x0E..0x12]) as usize + 0x0E;
        let width = LE::read_u32(&blob[0x12..0x16]) as usize;
        let height = LE::read_u32(&blob[0x16..0x1A]) as usize;
        let pal_len = LE::read_u32(&blob[0x2E..0x32]) as usize;
        let bpp8 = (bpp + 7) / 8;
        let stride = (width * bpp8 + 3) & !3;
        let mut vec = Vec::new();
        vec.try_reserve(width * height)
            .map_err(|_| DecodeError::OutOfMemory)?;

        match bpp {
            4 => {
                let palette = &blob[pal_offset..pal_offset + pal_len * 4];
                let width2_f = width / 2;
                let width2_c = (width + 1) / 2;
                let stride = (width2_c + 3) & !3;
                for y in 0..height {
                    let mut src = offset + (height - y - 1) * stride;
                    for _ in 0..width2_f {
                        let c4 = blob[src] as usize;
                        let cl = c4 >> 4;
                        let cr = c4 & 0x0F;
                        vec.push(TrueColor::from_rgb(LE::read_u32(&palette[cl * 4..cl * 4 + 4])));
                        vec.push(TrueColor::from_rgb(LE::read_u32(&palette[cr * 4..cr * 4 + 4])));
                        src += bpp8;
                    }
                    if width2_f < width2_c {
                        let c4 = blob[src] as usize;
                        let cl = c4 >> 4;
                        vec.push(TrueColor::from_rgb(LE::read_u32(&palette[cl * 4..cl * 4 + 4])));
                    }
                }
            }
            8 => {
                let palette = &blob[pal_offset..pal_offset + pal_len * 4];
                for y in 0..height {
                    let mut src = offset + (height - y - 1) * stride;
                    for _ in 0..width {
                        let ic = blob[src] as usize;
                        vec.push(TrueColor::from_rgb(LE::read_u32(&palette[ic * 4..ic * 4 + 4])));
                        src += bpp8;
                    }
                }
            }
            24 => {
                for y in 0..height {
                    let mut src = offset + (height - y - 1) * stride;
                    for _ in 0..width {
                        let b = blob[src];
                        let g = blob[src + 1];
                        let r = blob[src + 2];
                        vec.push(ColorComponents::from_rgb(r, g, b).into_true_color());
                        src += bpp8;
                    }
                }
            }
            32 => {
                for y in 0..height {
                    let mut src = offset + (height - y - 1) * stride;
                    for _ in 0..width {
                        vec.push(TrueColor::from_rgb(LE::read_u32(&blob[src..src + bpp8])));
                        src += bpp8;
                    }
                }
            }
            _ => unreachable!(),
        }
        Ok(OwnedBitmap32::from_vec(vec, Size::new(width as isize, height as isize)))
    }
}

#[derive(Debug)]
pub enum DecodeError {
    General,
    OutOfMemory,
    NotSupported,
    InvalidParameter,
    InvalidData,
}
