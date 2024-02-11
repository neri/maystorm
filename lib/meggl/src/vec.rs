//! Vector and Matrix

use core::ops::{Add, AddAssign, Mul, MulAssign, Sub, SubAssign};
use num_traits::Zero;

macro_rules! vec_impl {
    { $vis:vis struct $class:ident ( $($param:ident,)* ); } => {
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
            pub fn dot(self, rhs: &Self) -> T
            where
                T: Add<Output = T> + Mul<Output = T> + Copy,
            {
                vec_impl!(fn dot, self, rhs, $($param,)*)
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
                vec_impl!(fn is_zero, self, $($param,)*)
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
    };
    (fn dot, $self:ident, $rhs:ident, $param1:ident, $($param:ident,)*) => {
        $self.$param1.mul($rhs.$param1)
        $(
            .add($self.$param.mul($rhs.$param))
        )*
    };
    (fn is_zero, $self:ident, $param1:ident, $($param:ident,)*) => {
        $self.$param1.is_zero()
        $(
            && $self.$param.is_zero()
        )*
    };

}

vec_impl! {
    pub struct Vec2 (x, y,);
}
vec_impl! { pub struct Vec3 (x, y, z,); }
vec_impl! { pub struct Vec4 (x, y, z, w,); }
