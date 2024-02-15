//! Vector and Matrix

use core::mem::size_of;
use core::mem::transmute;
use core::ops::{Add, AddAssign, Index, IndexMut, Mul, MulAssign, Sub, SubAssign};
use num_traits::Zero;

macro_rules! vec_mat_impl {
    { $vis:vis struct $class:ident ( $n_elements:literal, $($param:ident,)* ); } => {
        #[repr(C)]
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
        $vis struct $class<T> {
            $(
                pub $param: T,
            )*
        }

        impl<T> $class<T> {
            #[inline]
            pub const fn new(
                $(
                    $param: T,
                )*
            ) -> Self {
                Self {
                    $(
                        $param,
                    )*
                }
            }

            $(
                #[inline]
                pub const fn $param(&self) -> T
                where
                    T: Copy
                {
                    self.$param
                }
            )*

            #[inline]
            pub fn into_slice(self) -> [T; $n_elements] {
                [
                    $( self.$param, )*
                ]
            }
        }

        impl<T: Zero> Zero for $class<T> {
            #[inline]
            fn zero() -> Self {
                Self {
                    $(
                        $param: T::zero(),
                    )*
                }
            }

            #[inline]
            fn is_zero(&self) -> bool {
                vec_mat_impl!(fn is_zero, self, $($param,)*)
            }
        }

        impl<T: Add<Output = T>> Add<Self> for $class<T> {
            type Output = Self;

            #[inline]
            fn add(self, rhs: Self) -> Self::Output {
                Self {
                    $(
                        $param: self.$param.add(rhs.$param),
                    )*
                }
            }
        }

        impl<T: AddAssign<T>> AddAssign<Self> for $class<T> {
            #[inline]
            fn add_assign(&mut self, rhs: Self) {
                $(
                    self.$param.add_assign(rhs.$param);
                )*
            }
        }

        impl<T: Sub<Output = T>> Sub<Self> for $class<T> {
            type Output = Self;

            #[inline]
            fn sub(self, rhs: Self) -> Self::Output {
                Self {
                    $(
                        $param: self.$param.sub(rhs.$param),
                    )*
                }
            }
        }

        impl<T: SubAssign<T>> SubAssign<Self> for $class<T> {
            #[inline]
            fn sub_assign(&mut self, rhs: Self) {
                $(
                    self.$param.sub_assign(rhs.$param);
                )*
            }
        }
    };
    (fn is_zero, $self:ident, $param1:ident, $($param:ident,)*) => {
        $self.$param1.is_zero()
        $(
            && $self.$param.is_zero()
        )*
    };
}

macro_rules! vec_impl {
    { $vis:vis struct $class:ident ($n_elements:literal, $($param:ident,)* ); } => {
        vec_mat_impl! {
            $vis struct $class (
                $n_elements,
                $($param, )*
            );
        }

        impl<T> $class<T> {
            #[inline]
            pub fn dot(self, rhs: &Self) -> T
            where
                T: Add<Output = T> + Mul<Output = T> + Copy,
            {
                vec_impl!(fn dot, self, rhs, $($param,)*)
            }
        }

        impl<T: Sized> $class<T> {
            #[inline]
            pub fn as_slice(&self) -> &[T; $n_elements] {
                assert_eq!(size_of::<Self>(), size_of::<[T; $n_elements]>());
                unsafe {
                    transmute(self)
                }
            }

            #[inline]
            pub fn as_slice_mut(&mut self) -> &mut [T; $n_elements] {
                assert_eq!(size_of::<Self>(), size_of::<[T; $n_elements]>());
                unsafe {
                    transmute(self)
                }
            }

            #[inline]
            pub fn iter(&self) -> impl Iterator<Item = &T> {
                self.as_slice().iter()
            }

            #[inline]
            pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
                self.as_slice_mut().iter_mut()
            }
        }

        impl<T: Mul<Output = T>> Mul<Self> for $class<T> {
            type Output = Self;

            #[inline]
            fn mul(self, rhs: Self) -> Self::Output {
                Self {
                    $(
                        $param: self.$param.mul(rhs.$param),
                    )*
                }
            }
        }

        impl<T: MulAssign<T>> MulAssign<Self> for $class<T> {
            #[inline]
            fn mul_assign(&mut self, rhs: Self) {
                $(
                    self.$param.mul_assign(rhs.$param);
                )*
            }
        }

        impl<T: Mul<Output = T> + Copy> Mul<T> for $class<T> {
            type Output = Self;

            #[inline]
            fn mul(self, rhs: T) -> Self::Output {
                Self {
                    $(
                        $param: self.$param.mul(rhs),
                    )*
                }
            }
        }

        impl<T: MulAssign<T> + Copy> MulAssign<T> for $class<T> {
            #[inline]
            fn mul_assign(&mut self, rhs: T) {
                $(
                    self.$param.mul_assign(rhs);
                )*
            }
        }

        impl<T: Sized> Index<usize> for $class<T> {
            type Output = T;
            #[inline]
            fn index<'a>(&'a self, index: usize) -> &'a T {
                let slice: &[T; $n_elements] = unsafe {
                    transmute(self)
                };
                slice.index(index)
            }
        }

        impl<T: Sized> IndexMut<usize> for $class<T> {
            #[inline]
            fn index_mut<'a>(&'a mut self, index: usize) -> &'a mut Self::Output {
                let slice: &mut [T; $n_elements] = unsafe {
                    transmute(self)
                };
                slice.index_mut(index)
            }
        }
    };
    (fn dot, $self:ident, $rhs:ident, $param1:ident, $($param:ident,)*) => {
        $self.$param1.mul($rhs.$param1)
        $(
            .add($self.$param.mul($rhs.$param))
        )*
    };
}

macro_rules! mat_impl {
    { $vis:vis struct $class:ident ($n_elements:literal, $($param:ident,)* ); } => {
        vec_mat_impl! {
            $vis struct $class (
                $n_elements,
                $($param,)*
            );
        }
    };
}

vec_impl! {
    pub struct Vec2 ( 2, x, y, );
}

vec_impl! { pub struct Vec3 ( 3,x, y, z, ); }

vec_impl! { pub struct Vec4 ( 4, x, y, z, w, ); }

mat_impl! {
    pub struct Mat2 ( 4,
        m00, m01,
        m10, m11,
    );
}
mat_impl! {
    pub struct Mat3 ( 9,
        m00, m01, m02,
        m10, m11, m12,
        m20, m21, m22,
    );
}
mat_impl! {
    pub struct Mat4 ( 16,
        m00, m01, m02, m03,
        m10, m11, m12, m13,
        m20, m21, m22, m23,
        m30, m31, m32, m33,
    );
}
