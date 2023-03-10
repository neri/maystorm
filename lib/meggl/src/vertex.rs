use crate::{Movement, Point};

pub type FloatType = f64;
use core::{
    f64::consts::{FRAC_PI_2, PI, TAU},
    ops::{Add, Div, Mul, Sub},
};

#[inline]
fn cos(radian: Radian) -> FloatType {
    libm::cos(radian.radian())
}

#[inline]
fn sin(radian: Radian) -> FloatType {
    libm::sin(radian.radian())
}

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
        affine_matrix.transformed(*self)
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
/// (x')   (m11 m12 m13) (x)
/// (y') = (m21 m22 m23) (y)
/// (1)    (  0   0   1) (1) <- redundant
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AffineMatrix2d {
    pub m11: FloatType,
    pub m12: FloatType,
    pub m13: FloatType,
    pub m21: FloatType,
    pub m22: FloatType,
    pub m23: FloatType,
}

impl AffineMatrix2d {
    #[inline]
    pub fn new(translation: Movement, rotation: Radian, scale: FloatType) -> Self {
        Self {
            m11: cos(rotation) * scale,
            m12: 0.0 - libm::sin(rotation.radian()) * scale,
            m13: translation.x as FloatType,
            m21: sin(rotation) * scale,
            m22: cos(rotation) * scale,
            m23: translation.y as FloatType,
        }
    }

    #[inline]
    pub fn transformed(&self, vertex: Vertex2d) -> Vertex2d {
        let x1 = vertex.x;
        let y1 = vertex.y;
        Vertex2d::new(
            self.m11 * x1 + self.m12 * y1 + self.m13,
            self.m21 * x1 + self.m22 * y1 + self.m23,
        )
    }

    #[inline]
    pub fn translation(translation: Movement) -> Self {
        Self {
            m11: 0.0,
            m12: 0.0,
            m13: translation.x as FloatType,
            m21: 0.0,
            m22: 0.0,
            m23: translation.y as FloatType,
        }
    }

    #[inline]
    pub fn rotation(rotation: Radian) -> Self {
        Self {
            m11: cos(rotation),
            m12: 0.0 - sin(rotation),
            m13: 0.0,
            m21: sin(rotation),
            m22: cos(rotation),
            m23: 0.0,
        }
    }

    #[inline]
    pub fn scaling(scale: FloatType) -> Self {
        Self {
            m11: scale,
            m12: 0.0,
            m13: 0.0,
            m21: 0.0,
            m22: scale,
            m23: 0.0,
        }
    }
}

impl AffineMatrix for AffineMatrix2d {}

impl Mul<AffineMatrix2d> for AffineMatrix2d {
    type Output = Self;

    fn mul(self, rhs: AffineMatrix2d) -> Self::Output {
        Self {
            m11: self.m11 * rhs.m11 + self.m12 * rhs.m21,
            m12: self.m11 * rhs.m12 + self.m12 * rhs.m22,
            m13: self.m11 * rhs.m13 + self.m12 * rhs.m23 + self.m13,
            m21: self.m21 * rhs.m11 + self.m22 * rhs.m21,
            m22: self.m21 * rhs.m12 + self.m22 * rhs.m22,
            m23: self.m21 * rhs.m13 + self.m22 * rhs.m23 + self.m23,
        }
    }
}

/// 3D Affine Transformation
///
/// ```plain
/// (x')   (m11 m12 m13 m14) (x)
/// (y') = (m21 m22 m23 m24) (y)
/// (z') = (m31 m32 m33 m34) (z)
/// (1)    (  0   0   0   1) (1) <- redundant
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AffineMatrix3d {
    pub m11: FloatType,
    pub m12: FloatType,
    pub m13: FloatType,
    pub m14: FloatType,
    pub m21: FloatType,
    pub m22: FloatType,
    pub m23: FloatType,
    pub m24: FloatType,
    pub m31: FloatType,
    pub m32: FloatType,
    pub m33: FloatType,
    pub m34: FloatType,
}

impl AffineMatrix3d {
    #[inline]
    pub fn transformed(&self, vertex: &Vertex3d) -> Vertex3d {
        let x1 = vertex.x;
        let y1 = vertex.y;
        let z1 = vertex.z;
        Vertex3d::new(
            self.m11 * x1 + self.m12 * y1 + self.m13 * z1 + self.m14,
            self.m21 * x1 + self.m22 * y1 + self.m23 * z1 + self.m24,
            self.m31 * x1 + self.m32 * y1 + self.m33 * z1 + self.m34,
        )
    }

    #[inline]
    pub fn x_axis_rotation(radian: Radian) -> Self {
        Self {
            m11: 1.0,
            m12: 0.0,
            m13: 0.0,
            m14: 0.0,

            m21: 0.0,
            m22: cos(radian),
            m23: -sin(radian),
            m24: 0.0,

            m31: 0.0,
            m32: sin(radian),
            m33: cos(radian),
            m34: 0.0,
        }
    }

    #[inline]
    pub fn y_axis_rotation(radian: Radian) -> Self {
        Self {
            m11: cos(radian),
            m12: 0.0,
            m13: sin(radian),
            m14: 0.0,

            m21: 0.0,
            m22: 1.0,
            m23: 0.0,
            m24: 0.0,

            m31: -sin(radian),
            m32: 0.0,
            m33: cos(radian),
            m34: 0.0,
        }
    }

    #[inline]
    pub fn z_axis_rotation(radian: Radian) -> Self {
        Self {
            m11: cos(radian),
            m12: -sin(radian),
            m13: 0.0,
            m14: 0.0,

            m21: sin(radian),
            m22: cos(radian),
            m23: 0.0,
            m24: 0.0,

            m31: 0.0,
            m32: 0.0,
            m33: 1.0,
            m34: 0.0,
        }
    }

    #[inline]
    pub fn translation(x: FloatType, y: FloatType, z: FloatType) -> Self {
        Self {
            m11: 0.0,
            m12: 0.0,
            m13: 0.0,
            m14: x,

            m21: 0.0,
            m22: 0.0,
            m23: 0.0,
            m24: y,

            m31: 0.0,
            m32: 0.0,
            m33: 0.0,
            m34: z,
        }
    }

    #[inline]
    pub fn scaling(scale: FloatType) -> Self {
        Self {
            m11: scale,
            m12: 0.0,
            m13: 0.0,
            m14: 0.0,

            m21: 0.0,
            m22: scale,
            m23: 0.0,
            m24: 0.0,

            m31: 0.0,
            m32: 0.0,
            m33: scale,
            m34: 0.0,
        }
    }
}

impl AffineMatrix for AffineMatrix3d {}
