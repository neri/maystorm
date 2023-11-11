use super::*;
use core::mem::transmute;

#[const_trait]
pub trait Drawable
where
    Self::ColorType: PixelColor,
{
    type ColorType;

    fn size(&self) -> Size;

    #[inline]
    fn width(&self) -> usize {
        self.size().width() as usize
    }

    #[inline]
    fn height(&self) -> usize {
        self.size().height() as usize
    }

    #[inline]
    fn bounds(&self) -> Rect {
        Rect::from(self.size())
    }
}

pub trait GetPixel: Drawable {
    /// # Safety
    ///
    /// The point must be within the size range.
    unsafe fn get_pixel_unchecked(&self, point: Point) -> Self::ColorType;

    #[inline]
    fn get_pixel(&self, point: Point) -> Option<Self::ColorType> {
        if self.bounds().contains(point) {
            Some(unsafe { self.get_pixel_unchecked(point) })
        } else {
            None
        }
    }

    #[inline]
    fn to_operational<F>(&self, kernel: F) -> OperationalBitmap
    where
        F: FnMut(<Self as Drawable>::ColorType) -> u8,
    {
        OperationalBitmap::from_pixels(self, kernel)
    }

    /// Returns an iterator that enumerates all pixels of the bitmap in order from left to right and top to bottom.
    #[inline]
    fn all_pixels<'a>(&'a self) -> AllPixels<'a, Self> {
        AllPixels::new(self)
    }
}

pub struct AllPixels<'a, T>
where
    T: GetPixel + ?Sized,
{
    inner: &'a T,
    x: usize,
    y: usize,
}

impl<'a, T> AllPixels<'a, T>
where
    T: GetPixel + ?Sized,
{
    #[inline]
    pub const fn new(inner: &'a T) -> Self {
        Self { inner, x: 0, y: 0 }
    }
}

impl<T> Iterator for AllPixels<'_, T>
where
    T: GetPixel + ?Sized,
{
    type Item = T::ColorType;

    fn next(&mut self) -> Option<Self::Item> {
        if self.y < self.inner.height() {
            if self.x >= self.inner.width() {
                self.x = 0;
                self.y += 1;
            }
            let result = unsafe {
                self.inner
                    .get_pixel_unchecked(Point::new(self.x as isize, self.y as isize))
            };
            self.x += 1;
            Some(result)
        } else {
            None
        }
    }
}

pub trait SetPixel: Drawable {
    /// # Safety
    ///
    /// The point must be within the size range.
    unsafe fn set_pixel_unchecked(&mut self, point: Point, pixel: Self::ColorType);

    #[inline]
    fn set_pixel(&mut self, point: Point, pixel: Self::ColorType) {
        if self.bounds().contains(point) {
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
    unsafe fn process_pixel_unchecked<F>(&mut self, point: Point, kernel: F)
    where
        F: FnOnce(Self::ColorType) -> Self::ColorType,
    {
        let stride = self.stride();
        let pixel = self
            .slice_mut()
            .get_unchecked_mut(point.x as usize + point.y as usize * stride);
        *pixel = kernel(*pixel);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rotation {
    /// 0 degree
    Default = 0,
    /// 90 degree
    ClockWise = 1,
    /// 180 degree
    UpsideDown = 2,
    /// 270 degree
    CounterClockWise = 3,
}

impl Rotation {
    #[inline]
    pub const fn succ(self) -> Self {
        match self {
            Self::Default => Self::ClockWise,
            Self::ClockWise => Self::UpsideDown,
            Self::UpsideDown => Self::CounterClockWise,
            Self::CounterClockWise => Self::Default,
        }
    }
}

impl const Default for Rotation {
    #[inline]
    fn default() -> Self {
        Self::Default
    }
}

impl const From<usize> for Rotation {
    #[inline]
    fn from(value: usize) -> Self {
        unsafe { transmute(value as u8) }
    }
}

impl const From<Rotation> for usize {
    #[inline]
    fn from(value: Rotation) -> Self {
        value as usize
    }
}
