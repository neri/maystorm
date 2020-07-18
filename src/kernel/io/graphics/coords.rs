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
    pub const fn new(x: T, y: T) -> Self {
        Point { x: x, y: y }
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
    pub fn new(width: T, height: T) -> Self {
        Size {
            width: width,
            height: height,
        }
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
    pub fn new(x: T, y: T, width: T, height: T) -> Self {
        Rect {
            origin: Point { x: x, y: y },
            size: Size {
                width: width,
                height: height,
            },
        }
    }

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
            size: size,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Default, Clone, PartialEq)]
pub struct EdgeInsets<T: Number> {
    pub top: T,
    pub left: T,
    pub bottom: T,
    pub right: T,
}

impl<T: Number> EdgeInsets<T> {
    pub fn new(top: T, left: T, bottom: T, right: T) -> Self {
        EdgeInsets {
            top: top,
            left: left,
            bottom: bottom,
            right: right,
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
