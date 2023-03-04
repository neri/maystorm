use crate::{Movement, Point};

pub type FloatType = f64;
use core::{
    f64::consts::{FRAC_PI_2, PI, TAU},
    ops::{Add, Div, Mul, Sub},
};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex2d {
    pub x: FloatType,
    pub y: FloatType,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex3d {
    pub x: FloatType,
    pub y: FloatType,
    pub z: FloatType,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex4d {
    pub x: FloatType,
    pub y: FloatType,
    pub z: FloatType,
    pub w: FloatType,
}

impl Vertex2d {
    #[inline]
    pub const fn new(x: FloatType, y: FloatType) -> Self {
        Self { x, y }
    }

    #[inline]
    pub const fn from_point(value: Point) -> Self {
        Self {
            x: value.x as FloatType,
            y: value.y as FloatType,
        }
    }

    #[inline]
    pub fn into_point(self) -> Point {
        Point {
            x: libm::round(self.x) as isize,
            y: libm::round(self.y) as isize,
        }
    }

    #[inline]
    pub fn transformed(&self, affine_matrix: &AffineMatrix2d) -> Self {
        affine_matrix.transformed(self)
    }
}

impl Transform<AffineMatrix2d> for Vertex2d {
    #[inline]
    fn transform(&mut self, affine_matrix: &AffineMatrix2d) {
        *self = self.transformed(affine_matrix)
    }
}

impl Transform<AffineMatrix2d> for [Vertex2d] {
    #[inline]
    fn transform(&mut self, affine_matrix: &AffineMatrix2d) {
        for vertex in self.iter_mut() {
            vertex.transform(affine_matrix);
        }
    }
}

impl const From<Point> for Vertex2d {
    #[inline]
    fn from(value: Point) -> Self {
        Vertex2d::from_point(value)
    }
}

impl From<Vertex2d> for Point {
    #[inline]
    fn from(value: Vertex2d) -> Self {
        value.into_point()
    }
}

impl Vertex3d {
    #[inline]
    pub const fn new(x: FloatType, y: FloatType, z: FloatType) -> Self {
        Self { x, y, z }
    }
}

impl Vertex4d {
    #[inline]
    pub const fn new(x: FloatType, y: FloatType, z: FloatType, w: FloatType) -> Self {
        Self { x, y, z, w }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Radian(FloatType);

impl Radian {
    /// 0.0 = 0 degrees
    pub const ZERO: Self = Self(0.0);
    /// π/2 = 90 degrees
    pub const FRAC_PI_2: Self = Self(FRAC_PI_2);
    /// π = 180 degrees
    pub const PI: Self = Self(PI);
    /// τ (2π) = 360 degrees
    pub const TAU: Self = Self(TAU);

    #[inline]
    pub const fn new(radian: FloatType) -> Self {
        Self(radian)
    }

    #[inline]
    pub const fn radian(&self) -> FloatType {
        self.0
    }
}

impl Add<Radian> for Radian {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Radian) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Add<FloatType> for Radian {
    type Output = Self;

    #[inline]
    fn add(self, rhs: FloatType) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl Sub<FloatType> for Radian {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: FloatType) -> Self::Output {
        Self(self.0 - rhs)
    }
}

impl Sub<Radian> for Radian {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Radian) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Mul<FloatType> for Radian {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: FloatType) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl Div<FloatType> for Radian {
    type Output = Self;

    #[inline]
    fn div(self, rhs: FloatType) -> Self::Output {
        Self(self.0 / rhs)
    }
}

pub trait AffineMatrix {}

pub trait Transform<T: AffineMatrix> {
    fn transform(&mut self, affine_matrix: &T);
}

/// Affine Transformation
///
/// ```plain
/// (x')   (a b c) (x)
/// (y') = (d e f) (y)
/// (1)    (0 0 1) (1) <- redundant
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AffineMatrix2d {
    pub a: FloatType,
    pub b: FloatType,
    pub c: FloatType,
    pub d: FloatType,
    pub e: FloatType,
    pub f: FloatType,
}

impl AffineMatrix2d {
    #[inline]
    pub fn new(translation: Movement, rotation: Radian, scale: FloatType) -> Self {
        Self {
            a: libm::cos(rotation.radian()) * scale,
            b: 0.0 - libm::sin(rotation.radian()) * scale,
            c: translation.x as FloatType,
            d: libm::sin(rotation.radian()) * scale,
            e: libm::cos(rotation.radian()) * scale,
            f: translation.y as FloatType,
        }
    }

    #[inline]
    pub fn transformed(&self, vertex: &Vertex2d) -> Vertex2d {
        let x1 = vertex.x;
        let y1 = vertex.y;
        Vertex2d::new(
            self.a * x1 + self.b * y1 + self.c,
            self.d * x1 + self.e * y1 + self.f,
        )
    }

    #[inline]
    pub fn translation(translation: Movement) -> Self {
        Self {
            a: 0.0,
            b: 0.0,
            c: translation.x as FloatType,
            d: 0.0,
            e: 0.0,
            f: translation.y as FloatType,
        }
    }

    #[inline]
    pub fn rotation(rotation: Radian) -> Self {
        Self {
            a: libm::cos(rotation.radian()),
            b: 0.0 - libm::sin(rotation.radian()),
            c: 0.0,
            d: libm::sin(rotation.radian()),
            e: libm::cos(rotation.radian()),
            f: 0.0,
        }
    }

    #[inline]
    pub fn scaling(scale: FloatType) -> Self {
        Self {
            a: scale,
            b: 0.0,
            c: 0.0,
            d: 0.0,
            e: scale,
            f: 0.0,
        }
    }
}

impl AffineMatrix for AffineMatrix2d {}
