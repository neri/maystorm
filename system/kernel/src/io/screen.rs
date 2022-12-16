use crate::sync::atomic::AtomicEnum;
use core::{cell::UnsafeCell, mem::transmute};
use megstd::drawing::*;

pub trait Screen<T>: Drawable
where
    T: Drawable<ColorType = Self::ColorType>,
{
    fn native_size(&self) -> Size;

    fn blt(&self, src: &T, origin: Point, rect: Rect);

    fn fill_rect(&self, rect: Rect, color: Self::ColorType);

    fn rotation(&self) -> ScreenRotation {
        Default::default()
    }

    fn set_rotation(&self, val: ScreenRotation) -> Result<ScreenRotation, ScreenRotation> {
        let _ = val;
        Err(self.rotation())
    }

    fn rotate(&self) -> Result<ScreenRotation, ScreenRotation> {
        self.set_rotation(self.rotation().succ())
    }

    fn orientation(&self) -> ScreenOrientation {
        let dims = self.size();
        if dims.width() < dims.height() {
            ScreenOrientation::Portrait
        } else {
            ScreenOrientation::Landscape
        }
    }

    fn set_orientation(
        &self,
        val: ScreenOrientation,
    ) -> Result<ScreenOrientation, ScreenOrientation> {
        let _ = val;
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
            ScreenRotation::_180 => {
                // TODO:
            }
            ScreenRotation::_270 => {
                // TODO:
            }
        }
    }

    fn fill_rect(&self, rect: Rect, color: Self::ColorType) {
        if self.is_natural_orientation() {
            self.bitmap().fill_rect(rect, color.into());
        } else {
            let Ok(coords) = Coordinates::from_rect(rect) else { return };
            let c1 = coords.left_top().swapped();
            let c2 = coords.right_bottom().swapped();
            self.bitmap()
                .fill_rect(Coordinates::from_diagonal(c1, c2).into(), color.into());
        }
    }

    fn rotation(&self) -> ScreenRotation {
        self.rotation.value()
    }

    fn set_rotation(&self, val: ScreenRotation) -> Result<ScreenRotation, ScreenRotation> {
        if match val {
            ScreenRotation::_0 | ScreenRotation::_90 => {
                self.rotation.set(val);
                true
            }
            ScreenRotation::_180 => {
                self.rotation.set(ScreenRotation::_0);
                false
            }
            ScreenRotation::_270 => {
                self.rotation.set(ScreenRotation::_90);
                false
            }
        } {
            Ok(self.rotation())
        } else {
            Err(self.rotation())
        }
    }

    fn set_orientation(
        &self,
        val: ScreenOrientation,
    ) -> Result<ScreenOrientation, ScreenOrientation> {
        if self.is_portrait_native() {
            self.rotation.set(match val {
                ScreenOrientation::Portrait => ScreenRotation::_0,
                ScreenOrientation::Landscape => ScreenRotation::_90,
            });
        } else {
            self.rotation.set(match val {
                ScreenOrientation::Portrait => ScreenRotation::_90,
                ScreenOrientation::Landscape => ScreenRotation::_0,
            });
        }
        Ok(self.orientation())
    }
}
