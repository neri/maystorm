//! Vector and Matrix

macro_rules! vec_impl {
    ($class:ident, $($param:ident,)*) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $class<T> {
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
        }
    };
}

vec_impl!(Vec2, x, y,);
vec_impl!(Vec3, x, y, z,);
vec_impl!(Vec4, x, y, z, w,);
