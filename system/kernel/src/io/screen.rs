use crate::sync::atomic::AtomicEnum;
use core::{cell::UnsafeCell, mem::transmute};
use megstd::drawing::*;

pub trait Screen<T>: Drawable
where
    T: Drawable<ColorType = Self::ColorType>,
{
    fn blt(&self, src: &T, origin: Point, rect: Rect);

    fn fill_rect(&self, rect: Rect, color: Self::ColorType);

    /// Returns the native screen size
    fn native_size(&self) -> Size;

    /// Returns the physical size of the screen, if available.
    fn physical_size(&self) -> Option<Size> {
        None
    }

    /// Returns the number of pixels per inch.
    fn pixels_per_inch(&self) -> usize {
        96
    }

    /// Returns the screen rotation status.
    fn rotation(&self) -> ScreenRotation {
        Default::default()
    }

    /// Changes the rotation state of the screen.
    fn set_rotation(&self, value: ScreenRotation) -> Result<ScreenRotation, ScreenRotation> {
        let _ = value;
        Err(self.rotation())
    }

    /// Rotate the screen one level, if possible.
    fn rotate(&self) -> Result<ScreenRotation, ScreenRotation> {
        self.set_rotation(self.rotation().succ())
    }

    /// Returns the screen orientation status.
    fn orientation(&self) -> ScreenOrientation {
        let dims = self.size();
        if dims.width() < dims.height() {
            ScreenOrientation::Portrait
        } else {
            ScreenOrientation::Landscape
        }
    }

    /// Changes the screen orientation state.
    fn set_orientation(
        &self,
        value: ScreenOrientation,
    ) -> Result<ScreenOrientation, ScreenOrientation> {
        let _ = value;
        Err(self.orientation())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScreenRotation {
    _0 = 0,
    _90 = 1,
    _180 = 2,
    _270 = 3,
}

impl ScreenRotation {
    #[inline]
    pub const fn succ(self) -> Self {
        match self {
            Self::_0 => Self::_90,
            Self::_90 => Self::_180,
            Self::_180 => Self::_270,
            Self::_270 => Self::_0,
        }
    }
}

impl const Default for ScreenRotation {
    #[inline]
    fn default() -> Self {
        Self::_0
    }
}

impl const From<usize> for ScreenRotation {
    #[inline]
    fn from(value: usize) -> Self {
        unsafe { transmute(value as u8) }
    }
}

impl const From<ScreenRotation> for usize {
    #[inline]
    fn from(value: ScreenRotation) -> Self {
        value as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScreenOrientation {
    Portrait,
    Landscape,
    // LandscapeLeft,
    // LandscapeRight,
    // PortraitUpsideDown,
}

impl ScreenOrientation {
    #[inline]
    pub const fn is_portrait(&self) -> bool {
        matches!(self, Self::Portrait)
    }

    #[inline]
    pub const fn is_landscape(&self) -> bool {
        matches!(self, Self::Landscape)
    }
}

pub struct BitmapScreen<'a> {
    fb: UnsafeCell<Bitmap32<'a>>,
    dims: Size,
    rotation: AtomicEnum<ScreenRotation>,
}

impl<'a> BitmapScreen<'a> {
    #[inline]
    pub const fn new(bitmap: Bitmap32<'a>) -> Self {
        Self {
            dims: bitmap.size(),
            fb: UnsafeCell::new(bitmap),
            rotation: AtomicEnum::default(),
        }
    }

    #[inline]
    fn bitmap(&self) -> &'a mut Bitmap32<'a> {
        unsafe { &mut *self.fb.get() }
    }

    #[inline]
    fn is_natural_orientation(&self) -> bool {
        matches!(self.rotation(), ScreenRotation::_0 | ScreenRotation::_180)
    }

    #[inline]
    fn is_portrait_native(&self) -> bool {
        self.dims.width < self.dims.height
    }
}

impl Drawable for BitmapScreen<'_> {
    type ColorType = TrueColor;

    fn size(&self) -> Size {
        if self.is_natural_orientation() {
            self.dims
        } else {
            Size::new(self.dims.height, self.dims.width)
        }
    }
}

impl Screen<ConstBitmap32<'_>> for BitmapScreen<'_> {
    fn native_size(&self) -> Size {
        self.dims
    }

    fn blt(&self, src: &ConstBitmap32, origin: Point, rect: Rect) {
        match self.rotation() {
            ScreenRotation::_0 => self.bitmap().blt(src, origin, rect),
            ScreenRotation::_90 => self.bitmap().blt_rotate(src, origin, rect),
            ScreenRotation::_180 | ScreenRotation::_270 => unreachable!(),
        }
    }

    fn fill_rect(&self, rect: Rect, color: Self::ColorType) {
        if self.is_natural_orientation() {
            self.bitmap().fill_rect(rect, color.into());
        } else {
            let rect = Rect::new(
                self.dims.height() - rect.min_y() - rect.height(),
                rect.min_x(),
                rect.height(),
                rect.width(),
            );
            self.bitmap().fill_rect(rect, color.into());
        }
    }

    fn rotation(&self) -> ScreenRotation {
        self.rotation.value()
    }

    fn set_rotation(&self, value: ScreenRotation) -> Result<ScreenRotation, ScreenRotation> {
        if match value {
            ScreenRotation::_0 | ScreenRotation::_90 => {
                self.rotation.set(value);
                true
            }
            ScreenRotation::_180 | ScreenRotation::_270 => false,
        } {
            Ok(self.rotation())
        } else {
            Err(self.rotation())
        }
    }

    fn rotate(&self) -> Result<ScreenRotation, ScreenRotation> {
        let new_val = match self.rotation.value() {
            ScreenRotation::_0 => ScreenRotation::_90,
            ScreenRotation::_90 => ScreenRotation::_0,
            ScreenRotation::_180 => ScreenRotation::_90,
            ScreenRotation::_270 => ScreenRotation::_0,
        };
        self.rotation.set(new_val);
        Ok(self.rotation())
    }

    fn set_orientation(
        &self,
        value: ScreenOrientation,
    ) -> Result<ScreenOrientation, ScreenOrientation> {
        if self.is_portrait_native() {
            self.rotation.set(match value {
                ScreenOrientation::Portrait => ScreenRotation::_0,
                ScreenOrientation::Landscape => ScreenRotation::_90,
            });
        } else {
            self.rotation.set(match value {
                ScreenOrientation::Portrait => ScreenRotation::_90,
                ScreenOrientation::Landscape => ScreenRotation::_0,
            });
        }
        Ok(self.orientation())
    }
}
