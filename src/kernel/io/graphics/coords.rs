// Coordinate Types

use crate::kernel::num::*;
use core::ops::*;

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct Point<T: Number> {
    pub x: T,
    pub y: T,
}

impl<T: Number> Point<T> {
    #[inline]
    pub const fn new(x: T, y: T) -> Self {
        Point { x, y }
    }
}

impl<T: Number> From<(T, T)> for Point<T> {
    fn from(p: (T, T)) -> Self {
        Self::new(p.0, p.1)
    }
}

impl<T: Number> Zero for Point<T> {
    fn zero() -> Self {
        Point {
            x: T::zero(),
            y: T::zero(),
        }
    }
}

impl<T: Number> Add for Point<T> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Point {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl<T: Number> AddAssign for Point<T> {
    fn add_assign(&mut self, rhs: Self) {
        *self = Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl<T: Number> Sub for Point<T> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Point {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl<T: Number> SubAssign for Point<T> {
    fn sub_assign(&mut self, rhs: Self) {
        *self = Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct Size<T: Number> {
    pub width: T,
    pub height: T,
}

impl<T: Number> Size<T> {
    #[inline]
    pub const fn new(width: T, height: T) -> Self {
        Size { width, height }
    }
}

impl<T: Number> From<(T, T)> for Size<T> {
    fn from(p: (T, T)) -> Self {
        Self::new(p.0, p.1)
    }
}

impl<T: Number> Zero for Size<T> {
    fn zero() -> Self {
        Size {
            width: T::zero(),
            height: T::zero(),
        }
    }
}

impl<T: Number> Add for Size<T> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Size {
            width: self.width + rhs.width,
            height: self.height + rhs.height,
        }
    }
}

impl<T: Number> AddAssign for Size<T> {
    fn add_assign(&mut self, rhs: Self) {
        *self = Self {
            width: self.width + rhs.width,
            height: self.height + rhs.height,
        }
    }
}

impl<T: Number> Sub for Size<T> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Size {
            width: self.width - rhs.width,
            height: self.height - rhs.height,
        }
    }
}

impl<T: Number> SubAssign for Size<T> {
    fn sub_assign(&mut self, rhs: Self) {
        *self = Self {
            width: self.width - rhs.width,
            height: self.height - rhs.height,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct Rect<T: Number> {
    pub origin: Point<T>,
    pub size: Size<T>,
}

impl<T: Number> Rect<T> {
    #[inline]
    pub const fn new(x: T, y: T, width: T, height: T) -> Self {
        Rect {
            origin: Point { x, y },
            size: Size { width, height },
        }
    }

    #[inline]
    pub const fn x(self) -> T {
        self.origin.x
    }

    #[inline]
    pub const fn y(self) -> T {
        self.origin.y
    }

    #[inline]
    pub const fn width(self) -> T {
        self.size.width
    }

    #[inline]
    pub const fn height(self) -> T {
        self.size.height
    }

    #[inline]
    pub fn insets_by(self, insets: EdgeInsets<T>) -> Self {
        Rect {
            origin: Point {
                x: self.origin.x + insets.left,
                y: self.origin.y + insets.top,
            },
            size: Size {
                width: self.size.width - (insets.left + insets.right),
                height: self.size.height - (insets.top + insets.bottom),
            },
        }
    }

    pub fn hit_test_rect(self, rhs: Self) -> bool {
        let cl = match Coordinates::from_rect(self) {
            Some(coords) => coords,
            None => return false,
        };
        let cr = match Coordinates::from_rect(rhs) {
            Some(coords) => coords,
            None => return false,
        };

        cl.left < cr.right && cr.left < cl.right && cl.top < cr.bottom && cr.top < cl.bottom
    }

    pub fn hit_test_point(self, point: Point<T>) -> bool {
        if let Some(coords) = Coordinates::from_rect(self) {
            coords.left <= point.x
                && coords.right > point.x
                && coords.top <= point.y
                && coords.bottom > point.y
        } else {
            false
        }
    }
}

impl<T: Number> From<(T, T, T, T)> for Rect<T> {
    fn from(p: (T, T, T, T)) -> Self {
        Self::new(p.0, p.1, p.2, p.3)
    }
}

impl<T: Number> Zero for Rect<T> {
    fn zero() -> Self {
        Rect {
            origin: Point::zero(),
            size: Size::zero(),
        }
    }
}

impl<T: Number> From<Size<T>> for Rect<T> {
    fn from(size: Size<T>) -> Self {
        Rect {
            origin: Point::zero(),
            size,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct Coordinates<T: Number> {
    pub left: T,
    pub top: T,
    pub right: T,
    pub bottom: T,
}

impl<T: Number> Coordinates<T> {
    pub const fn new(left: T, top: T, right: T, bottom: T) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    #[inline]
    pub fn left_top(self) -> Point<T> {
        Point::new(self.left, self.top)
    }

    #[inline]
    pub fn right_bottom(self) -> Point<T> {
        Point::new(self.right, self.bottom)
    }

    #[inline]
    pub fn left_bottom(self) -> Point<T> {
        Point::new(self.left, self.bottom)
    }

    #[inline]
    pub fn right_top(self) -> Point<T> {
        Point::new(self.right, self.top)
    }

    #[inline]
    pub fn size(self) -> Size<T> {
        Size::new(self.right - self.left, self.bottom - self.top)
    }

    #[inline]
    pub fn from_rect(rect: Rect<T>) -> Option<Coordinates<T>> {
        if rect.size.width == T::zero() || rect.size.height == T::zero() {
            None
        } else {
            Some(unsafe { Self::from_rect_unchecked(rect) })
        }
    }

    #[inline]
    pub unsafe fn from_rect_unchecked(rect: Rect<T>) -> Coordinates<T> {
        let left: T;
        let right: T;
        if rect.size.width > T::zero() {
            left = rect.origin.x;
            right = left + rect.size.width;
        } else {
            right = rect.origin.x;
            left = right + rect.size.width;
        }

        let top: T;
        let bottom: T;
        if rect.size.height > T::zero() {
            top = rect.origin.y;
            bottom = top + rect.size.height;
        } else {
            bottom = rect.origin.y;
            top = bottom + rect.size.height;
        }

        Self {
            left,
            top,
            right,
            bottom,
        }
    }
}

impl<T: Number> From<Coordinates<T>> for Rect<T> {
    fn from(coods: Coordinates<T>) -> Rect<T> {
        Rect {
            origin: coods.left_top(),
            size: coods.size(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct EdgeInsets<T: Number> {
    pub top: T,
    pub left: T,
    pub bottom: T,
    pub right: T,
}

impl<T: Number> EdgeInsets<T> {
    #[inline]
    pub const fn new(top: T, left: T, bottom: T, right: T) -> Self {
        Self {
            top,
            left,
            bottom,
            right,
        }
    }

    #[inline]
    pub fn padding_all(value: T) -> Self {
        Self {
            top: value,
            left: value,
            bottom: value,
            right: value,
        }
    }
}

impl<T: Number> From<(T, T, T, T)> for EdgeInsets<T> {
    fn from(p: (T, T, T, T)) -> Self {
        Self::new(p.0, p.1, p.2, p.3)
    }
}

impl<T: Number> Zero for EdgeInsets<T> {
    fn zero() -> Self {
        EdgeInsets {
            top: T::zero(),
            left: T::zero(),
            bottom: T::zero(),
            right: T::zero(),
        }
    }
}

impl<T: Number> Add for EdgeInsets<T> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            top: self.top + rhs.top,
            left: self.left + rhs.left,
            bottom: self.bottom + rhs.bottom,
            right: self.right + rhs.right,
        }
    }
}

impl<T: Number> AddAssign for EdgeInsets<T> {
    fn add_assign(&mut self, rhs: Self) {
        *self = Self {
            top: self.top + rhs.top,
            left: self.left + rhs.left,
            bottom: self.bottom + rhs.bottom,
            right: self.right + rhs.right,
        }
    }
}

impl<T: Number> Sub for EdgeInsets<T> {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self {
            top: self.top - rhs.top,
            left: self.left - rhs.left,
            bottom: self.bottom - rhs.bottom,
            right: self.right - rhs.right,
        }
    }
}

impl<T: Number> SubAssign for EdgeInsets<T> {
    fn sub_assign(&mut self, rhs: Self) {
        *self = Self {
            top: self.top - rhs.top,
            left: self.left - rhs.left,
            bottom: self.bottom - rhs.bottom,
            right: self.right - rhs.right,
        }
    }
}
