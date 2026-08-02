use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};

use ahash::AHashMap;
use bytes::Bytes;

/// Valida que un valor se encuentre dentro o fuera de un rango.
///
/// - **Números:** comparación directa del valor.
/// - **Strings:** longitud en **caracteres** (no bytes).
/// - **Colecciones:** cantidad de elementos.
pub trait Range {
    type Bound;

    /// Mayor o igual que el límite (inclusivo).
    fn min(&self, v: Self::Bound) -> bool;

    /// Menor o igual que el límite (inclusivo).
    fn max(&self, v: Self::Bound) -> bool;

    /// Estrictamente mayor que.
    fn gt(&self, v: Self::Bound) -> bool;

    /// Estrictamente menor que.
    fn lt(&self, v: Self::Bound) -> bool;

    /// Dentro del rango [min, max] (inclusivo).
    fn between(&self, min: Self::Bound, max: Self::Bound) -> bool;

    /// Dentro del rango (min, max) (exclusivo).
    fn between_exclusive(&self, min: Self::Bound, max: Self::Bound) -> bool;

    /// Fuera del rango [min, max] (inclusivo); `min` y `max` cuentan
    /// como dentro del rango, no como fuera. Ej: `outside(3, 10)` es
    /// `true` para 2 y 11, `false` para 3 y 10.
    fn outside(&self, min: Self::Bound, max: Self::Bound) -> bool;
}

/// Genera el impl de `Range` para tipos numéricos donde `Bound = Self` y
/// la comparación es directa (`*self >= v`, etc).
macro_rules! impl_range_numeric {
    ($($t:ty),* $(,)?) => {
        $(
            impl Range for $t {
                type Bound = $t;

                fn min(&self, v: Self::Bound) -> bool {
                    *self >= v
                }

                fn max(&self, v: Self::Bound) -> bool {
                    *self <= v
                }

                fn gt(&self, v: Self::Bound) -> bool {
                    *self > v
                }

                fn lt(&self, v: Self::Bound) -> bool {
                    *self < v
                }

                fn between(&self, min: Self::Bound, max: Self::Bound) -> bool {
                    *self >= min && *self <= max
                }

                fn between_exclusive(&self, min: Self::Bound, max: Self::Bound) -> bool {
                    *self > min && *self < max
                }

                fn outside(&self, min: Self::Bound, max: Self::Bound) -> bool {
                    *self < min || *self > max
                }
            }
        )*
    };
}

/// Genera el impl de `Range` para tipos tipo-string donde `Bound = usize`
/// y se mide en **caracteres** (`self.chars().count()`), no bytes.
macro_rules! impl_range_by_chars {
    ($($t:ty),* $(,)?) => {
        $(
            impl Range for $t {
                type Bound = usize;

                fn min(&self, v: Self::Bound) -> bool {
                    self.chars().count() >= v
                }

                fn max(&self, v: Self::Bound) -> bool {
                    self.chars().count() <= v
                }

                fn gt(&self, v: Self::Bound) -> bool {
                    self.chars().count() > v
                }

                fn lt(&self, v: Self::Bound) -> bool {
                    self.chars().count() < v
                }

                fn between(&self, min: Self::Bound, max: Self::Bound) -> bool {
                    let len = self.chars().count();
                    len >= min && len <= max
                }

                fn between_exclusive(&self, min: Self::Bound, max: Self::Bound) -> bool {
                    let len = self.chars().count();
                    len > min && len < max
                }

                fn outside(&self, min: Self::Bound, max: Self::Bound) -> bool {
                    let len = self.chars().count();
                    len < min || len > max
                }
            }
        )*
    };
}

/// Cuerpo compartido de `Range` para colecciones medidas por
/// `self.len()`. No es una macro que genera el `impl` completo -- genera
/// solo los métodos asociados, para poder invocarla dentro de un `impl`
/// con el header de genéricos que corresponda en cada caso (`impl<T>`,
/// `impl<K, V>`, o sin genéricos). Esto evita la ambigüedad de parsing
/// que sale al intentar hacer opcional la lista de genéricos dentro del
/// matcher de una sola macro (`local ambiguity ... built-in NTs ty`).
macro_rules! range_len_methods {
    () => {
        type Bound = usize;

        fn min(&self, v: Self::Bound) -> bool {
            self.len() >= v
        }

        fn max(&self, v: Self::Bound) -> bool {
            self.len() <= v
        }

        fn gt(&self, v: Self::Bound) -> bool {
            self.len() > v
        }

        fn lt(&self, v: Self::Bound) -> bool {
            self.len() < v
        }

        fn between(&self, min: Self::Bound, max: Self::Bound) -> bool {
            let len = self.len();
            len >= min && len <= max
        }

        fn between_exclusive(&self, min: Self::Bound, max: Self::Bound) -> bool {
            let len = self.len();
            len > min && len < max
        }

        fn outside(&self, min: Self::Bound, max: Self::Bound) -> bool {
            let len = self.len();
            len < min || len > max
        }
    };
}

// ===============================================================================
// Enteros y flotantes
// ===============================================================================
impl_range_numeric!(i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64,);

// ===============================================================================
// Strings (por cantidad de caracteres, no bytes)
// ===============================================================================
impl_range_by_chars!(String, &str, Cow<'_, str>);

// ===============================================================================
// Colecciones (por cantidad de elementos)
// ===============================================================================
impl Range for Bytes {
    range_len_methods!();
}

impl<T> Range for Vec<T> {
    range_len_methods!();
}

impl<T> Range for &[T] {
    range_len_methods!();
}

impl<K, V> Range for HashMap<K, V> {
    range_len_methods!();
}

impl<K, V> Range for AHashMap<K, V> {
    range_len_methods!();
}

impl<K, V> Range for BTreeMap<K, V> {
    range_len_methods!();
}

// ===============================================================================
// [T; N] queda aparte: usa el parámetro const N directamente en vez de
// self.len(), y necesita `const N: usize` en la lista de genéricos, algo
// que `range_len_methods!` no cubre (esa macro asume que `self.len()` ya
// existe -- un array no tiene `.len()` const-friendly de la misma forma
// dentro del cuerpo compartido). Un solo caso, se escribe a mano.
// ===============================================================================
impl<T, const N: usize> Range for [T; N] {
    type Bound = usize;

    fn min(&self, v: Self::Bound) -> bool {
        N >= v
    }

    fn max(&self, v: Self::Bound) -> bool {
        N <= v
    }

    fn gt(&self, v: Self::Bound) -> bool {
        N > v
    }

    fn lt(&self, v: Self::Bound) -> bool {
        N < v
    }

    fn between(&self, min: Self::Bound, max: Self::Bound) -> bool {
        N >= min && N <= max
    }

    fn between_exclusive(&self, min: Self::Bound, max: Self::Bound) -> bool {
        N > min && N < max
    }

    fn outside(&self, min: Self::Bound, max: Self::Bound) -> bool {
        N < min || N > max
    }
}
