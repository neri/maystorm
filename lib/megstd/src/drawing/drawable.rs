use super::*;
use core::mem::size_of;

pub trait Drawable
where
    Self::ColorType: ColorTrait,
{
    type ColorType;

    fn width(&self) -> usize;

    fn height(&self) -> usize;

    #[inline]
    fn bpp(&self) -> usize {
        8 * size_of::<Self::ColorType>()
    }

    #[inline]
    fn size(&self) -> Size {
        Size::new(self.width() as isize, self.height() as isize)
    }

    #[inline]
    fn bounds(&self) -> Rect {
        Rect::from(self.size())
    }
}

pub trait GetPixel: Drawable {
    /// SAFETY: The point must be within the size range.
    unsafe fn get_pixel_unchecked(&self, point: Point) -> Self::ColorType;

    fn get_pixel(&self, point: Point) -> Option<Self::ColorType> {
        if point.is_within(Rect::from(self.size())) {
            Some(unsafe { self.get_pixel_unchecked(point) })
        } else {
            None
        }
    }
}

pub trait SetPixel: Drawable {
    /// SAFETY: The point must be within the size range.
    unsafe fn set_pixel_unchecked(&mut self, point: Point, pixel: Self::ColorType);

    fn set_pixel(&mut self, point: Point, pixel: Self::ColorType) {
        if point.is_within(Rect::from(self.size())) {
            unsafe {
                self.set_pixel_unchecked(point, pixel);
            }
        }
    }
}

pub trait RasterImage: Drawable {
    fn slice(&self) -> &[Self::ColorType];

    fn stride(&self) -> usize {
        self.width()
    }
}

impl<T> GetPixel for T
where
    Self: RasterImage,
{
    unsafe fn get_pixel_unchecked(&self, point: Point) -> Self::ColorType {
        *self
            .slice()
            .get_unchecked(point.x as usize + point.y as usize * self.stride())
    }
}

pub trait MutableRasterImage: RasterImage {
    fn slice_mut(&mut self) -> &mut [Self::ColorType];

    #[inline]
    unsafe fn process_pixel_unchecked<F>(&mut self, point: Point, f: F)
    where
        F: FnOnce(Self::ColorType) -> Self::ColorType,
    {
        let stride = self.stride();
        let pixel = self
            .slice_mut()
            .get_unchecked_mut(point.x as usize + point.y as usize * stride);
        *pixel = f(*pixel);
    }
}

impl<T> SetPixel for T
where
    Self: MutableRasterImage,
{
    unsafe fn set_pixel_unchecked(&mut self, point: Point, pixel: Self::ColorType) {
        let stride = self.stride();
        *self
            .slice_mut()
            .get_unchecked_mut(point.x as usize + point.y as usize * stride) = pixel;
    }
}
