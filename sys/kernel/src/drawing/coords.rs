// Coordinate Types

use core::{convert::TryFrom, ops::*};

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct Point {
    pub x: isize,
    pub y: isize,
}

impl Point {
    #[inline]
    pub const fn new(x: isize, y: isize) -> Self {
        Self { x, y }
    }

    #[inline]
    pub const fn x(&self) -> isize {
        self.x
    }

    #[inline]
    pub const fn y(&self) -> isize {
        self.y
    }

    pub fn line_to<F>(&self, other: Point, mut f: F)
    where
        F: FnMut(Self),
    {
        let c0 = *self;
        let c1 = other;

        let d = Point::new(
            if c1.x > c0.x {
                c1.x - c0.x
            } else {
                c0.x - c1.x
            },
            if c1.y > c0.y {
                c1.y - c0.y
            } else {
                c0.y - c1.y
            },
        );

        let s = Self::new(
            if c1.x > c0.x { 1 } else { -1 },
            if c1.y > c0.y { 1 } else { -1 },
        );

        let mut c0 = c0;
        let mut e = d.x - d.y;
        loop {
            f(c0);
            if c0.x == c1.x && c0.y == c1.y {
                break;
            }
            let e2 = e + e;
            if e2 > -d.y {
                e -= d.y;
                c0.x += s.x;
            }
            if e2 < d.x {
                e += d.x;
                c0.y += s.y;
            }
        }
    }

    #[inline]
    pub fn is_within(self, rect: Rect) -> bool {
        if let Ok(coords) = Coordinates::from_rect(rect) {
            coords.left <= self.x
                && coords.right > self.x
                && coords.top <= self.y
                && coords.bottom > self.y
        } else {
            false
        }
    }
}

impl Add<Self> for Point {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Point {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Add<Size> for Point {
    type Output = Self;
    fn add(self, rhs: Size) -> Self {
        Point {
            x: self.x + rhs.width,
            y: self.y + rhs.height,
        }
    }
}

impl Add<isize> for Point {
    type Output = Self;
    fn add(self, rhs: isize) -> Self {
        Point {
            x: self.x + rhs,
            y: self.y + rhs,
        }
    }
}

impl AddAssign for Point {
    fn add_assign(&mut self, rhs: Self) {
        *self = Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub<Self> for Point {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Point {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Sub<Size> for Point {
    type Output = Self;
    fn sub(self, rhs: Size) -> Self {
        Point {
            x: self.x - rhs.width,
            y: self.y - rhs.height,
        }
    }
}

impl Sub<isize> for Point {
    type Output = Self;
    fn sub(self, rhs: isize) -> Self {
        Point {
            x: self.x - rhs,
            y: self.y - rhs,
        }
    }
}

impl SubAssign for Point {
    fn sub_assign(&mut self, rhs: Self) {
        *self = Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct Size {
    pub width: isize,
    pub height: isize,
}

impl Size {
    #[inline]
    pub const fn new(width: isize, height: isize) -> Self {
        Self { width, height }
    }

    #[inline]
    pub const fn width(&self) -> isize {
        self.width
    }

    #[inline]
    pub const fn height(&self) -> isize {
        self.height
    }
}

impl Add<Self> for Size {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Size {
            width: self.width + rhs.width,
            height: self.height + rhs.height,
        }
    }
}

impl Add<EdgeInsets> for Size {
    type Output = Self;
    fn add(self, rhs: EdgeInsets) -> Self {
        Size {
            width: self.width + rhs.left + rhs.right,
            height: self.height + rhs.top + rhs.bottom,
        }
    }
}

impl AddAssign<Self> for Size {
    fn add_assign(&mut self, rhs: Self) {
        *self = Self {
            width: self.width + rhs.width,
            height: self.height + rhs.height,
        }
    }
}

impl AddAssign<EdgeInsets> for Size {
    fn add_assign(&mut self, rhs: EdgeInsets) {
        *self = Self {
            width: self.width + rhs.left + rhs.right,
            height: self.height + rhs.top + rhs.bottom,
        }
    }
}

impl Sub<Self> for Size {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Size {
            width: self.width - rhs.width,
            height: self.height - rhs.height,
        }
    }
}

impl Sub<EdgeInsets> for Size {
    type Output = Self;
    fn sub(self, rhs: EdgeInsets) -> Self {
        Size {
            width: self.width - (rhs.left + rhs.left),
            height: self.height - (rhs.top + rhs.bottom),
        }
    }
}

impl SubAssign for Size {
    fn sub_assign(&mut self, rhs: Self) {
        *self = Self {
            width: self.width - rhs.width,
            height: self.height - rhs.height,
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    #[inline]
    pub const fn new(x: isize, y: isize, width: isize, height: isize) -> Self {
        Self {
            origin: Point { x, y },
            size: Size { width, height },
        }
    }

    #[inline]
    pub const fn origin(&self) -> Point {
        self.origin
    }

    #[inline]
    pub const fn x(&self) -> isize {
        self.origin.x
    }

    #[inline]
    pub const fn y(&self) -> isize {
        self.origin.y
    }

    #[inline]
    pub const fn size(&self) -> Size {
        self.size
    }

    #[inline]
    pub const fn width(&self) -> isize {
        self.size.width
    }

    #[inline]
    pub const fn height(&self) -> isize {
        self.size.height
    }

    #[inline]
    pub fn insets_by(self, insets: EdgeInsets) -> Self {
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

    pub fn is_within_rect(self, rhs: Self) -> bool {
        let cl = match Coordinates::from_rect(self) {
            Ok(coords) => coords,
            Err(_) => return false,
        };
        let cr = match Coordinates::from_rect(rhs) {
            Ok(coords) => coords,
            Err(_) => return false,
        };

        cl.left < cr.right && cr.left < cl.right && cl.top < cr.bottom && cr.top < cl.bottom
    }

    pub fn center(&self) -> Point {
        Point::new(
            self.origin.x + self.size.width / 2,
            self.origin.y + self.size.height / 2,
        )
    }
}

impl From<Size> for Rect {
    fn from(size: Size) -> Self {
        Rect {
            origin: Point::new(0, 0),
            size,
        }
    }
}

impl Add<Point> for Rect {
    type Output = Self;
    fn add(self, rhs: Point) -> Self::Output {
        Self {
            origin: self.origin + rhs,
            size: self.size,
        }
    }
}

impl Sub<Point> for Rect {
    type Output = Self;
    fn sub(self, rhs: Point) -> Self::Output {
        Self {
            origin: self.origin - rhs,
            size: self.size,
        }
    }
}

impl Add<Size> for Rect {
    type Output = Self;
    fn add(self, rhs: Size) -> Self::Output {
        Self {
            origin: self.origin,
            size: self.size + rhs,
        }
    }
}

impl Sub<Size> for Rect {
    type Output = Self;
    fn sub(self, rhs: Size) -> Self::Output {
        Self {
            origin: self.origin,
            size: self.size - rhs,
        }
    }
}

impl Add<EdgeInsets> for Rect {
    type Output = Self;
    fn add(self, rhs: EdgeInsets) -> Self::Output {
        Rect {
            origin: Point {
                x: self.origin.x - rhs.left,
                y: self.origin.y - rhs.top,
            },
            size: Size {
                width: self.size.width + (rhs.left + rhs.right),
                height: self.size.height + (rhs.top + rhs.bottom),
            },
        }
    }
}

impl Sub<EdgeInsets> for Rect {
    type Output = Self;
    fn sub(self, rhs: EdgeInsets) -> Self::Output {
        self.insets_by(rhs)
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct Coordinates {
    pub left: isize,
    pub top: isize,
    pub right: isize,
    pub bottom: isize,
}

impl Coordinates {
    pub const fn new(left: isize, top: isize, right: isize, bottom: isize) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    #[inline]
    pub fn left_top(self) -> Point {
        Point::new(self.left, self.top)
    }

    #[inline]
    pub fn right_bottom(self) -> Point {
        Point::new(self.right, self.bottom)
    }

    #[inline]
    pub fn left_bottom(self) -> Point {
        Point::new(self.left, self.bottom)
    }

    #[inline]
    pub fn right_top(self) -> Point {
        Point::new(self.right, self.top)
    }

    #[inline]
    pub fn size(self) -> Size {
        Size::new(self.right - self.left, self.bottom - self.top)
    }

    #[inline]
    pub fn from_rect(rect: Rect) -> Result<Coordinates, ()> {
        if rect.size.width == 0 || rect.size.height == 0 {
            Err(())
        } else {
            Ok(unsafe { Self::from_rect_unchecked(rect) })
        }
    }

    #[inline]
    pub unsafe fn from_rect_unchecked(rect: Rect) -> Coordinates {
        let left: isize;
        let right: isize;
        if rect.size.width > 0 {
            left = rect.origin.x;
            right = left + rect.size.width;
        } else {
            right = rect.origin.x;
            left = right + rect.size.width;
        }

        let top: isize;
        let bottom: isize;
        if rect.size.height > 0 {
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

impl TryFrom<Rect> for Coordinates {
    type Error = ();
    fn try_from(value: Rect) -> Result<Self, Self::Error> {
        Self::from_rect(value)
    }
}

impl From<Coordinates> for Rect {
    fn from(coods: Coordinates) -> Rect {
        Rect {
            origin: coods.left_top(),
            size: coods.size(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Default, PartialEq)]
pub struct EdgeInsets {
    pub top: isize,
    pub left: isize,
    pub bottom: isize,
    pub right: isize,
}

impl EdgeInsets {
    #[inline]
    pub const fn new(top: isize, left: isize, bottom: isize, right: isize) -> Self {
        Self {
            top,
            left,
            bottom,
            right,
        }
    }

    #[inline]
    pub const fn padding_each(value: isize) -> Self {
        Self {
            top: value,
            left: value,
            bottom: value,
            right: value,
        }
    }
}

impl Add for EdgeInsets {
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

impl AddAssign for EdgeInsets {
    fn add_assign(&mut self, rhs: Self) {
        *self = Self {
            top: self.top + rhs.top,
            left: self.left + rhs.left,
            bottom: self.bottom + rhs.bottom,
            right: self.right + rhs.right,
        }
    }
}

impl Sub for EdgeInsets {
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

impl SubAssign for EdgeInsets {
    fn sub_assign(&mut self, rhs: Self) {
        *self = Self {
            top: self.top - rhs.top,
            left: self.left - rhs.left,
            bottom: self.bottom - rhs.bottom,
            right: self.right - rhs.right,
        }
    }
}
