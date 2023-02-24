use crate::{Movement, Point};

pub type FloatType = f64;

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
    pub const fn into_point(self) -> Point {
        Point {
            x: self.x as isize,
            y: self.y as isize,
        }
    }

    #[inline]
    pub fn transform(&mut self, affine_matrix: &AffineMatrix2d) {
        *self = affine_matrix.transformed(*self);
    }
}

impl const From<Point> for Vertex2d {
    #[inline]
    fn from(value: Point) -> Self {
        Vertex2d::from_point(value)
    }
}

impl const From<Vertex2d> for Point {
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

/// Affine Transformation
///
/// [x']   [a b c] [x]
/// [y'] = [d e f] [y]
/// [1]    [0 0 1] [1] <- redundant
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AffineMatrix2d {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl AffineMatrix2d {
    #[inline]
    pub fn new(translation: Movement, rotation: f64, scale: f64) -> Self {
        Self {
            a: libm::cos(rotation) * scale,
            b: 0.0 - libm::sin(rotation) * scale,
            c: translation.x as f64,
            d: libm::sin(rotation) * scale,
            e: libm::cos(rotation) * scale,
            f: translation.y as f64,
        }
    }

    #[inline]
    pub fn transformed(&self, vertex: Vertex2d) -> Vertex2d {
        let x1 = vertex.x;
        let y1 = vertex.y;
        Vertex2d::new(
            self.a * x1 + self.b * y1 + self.c,
            self.d * x1 + self.e * y1 + self.f,
        )
    }

    #[inline]
    pub fn transform_polygon(&self, polygon: &mut [Vertex2d]) {
        for vertex in polygon {
            vertex.transform(self);
        }
    }

    #[inline]
    pub fn translation(translation: Movement) -> Self {
        Self {
            a: 0.0,
            b: 0.0,
            c: translation.x as f64,
            d: 0.0,
            e: 0.0,
            f: translation.y as f64,
        }
    }

    #[inline]
    pub fn rotation(rotation: f64) -> Self {
        Self {
            a: libm::cos(rotation),
            b: 0.0 - libm::sin(rotation),
            c: 0.0,
            d: libm::sin(rotation),
            e: libm::cos(rotation),
            f: 0.0,
        }
    }

    #[inline]
    pub fn scaling(scale: f64) -> Self {
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
