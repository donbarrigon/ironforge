use std::collections::{BTreeMap, HashMap};

use ahash::AHashMap;

pub const MSG_IS_IN: &str = "the :field field is not an allowed value";
pub const MSG_NOT_IS_IN: &str = "the :field field must not be an allowed value";
pub const MSG_NOT_IN: &str = "the :field field contains a value that is not allowed";
pub const MSG_NOT_NOT_IN: &str = "the :field field must not contain a disallowed value";
pub const MSG_IS_DISTINCT: &str = "the :field field must not have repeated values";
pub const MSG_NOT_IS_DISTINCT: &str = "the :field field must have repeated values";

/// Agrupa las tres reglas que preguntan cosas sobre una **colección**:
///
/// - `is_in(&value)` / `not_in(&value)` -- ¿está (o no) `value` entre
///   los elementos de `self`? (regla `in`/`not_in` de Laravel: acá el
///   "listado permitido" es la colección misma, y `value` es el dato
///   del campo que se está validando).
/// - `is_distinct()` -- ¿la colección no tiene elementos repetidos?
///   Para mapas se compara por *valores*, no por keys (las keys ya son
///   únicas por definición del tipo).
///
/// Un solo trait porque las tres preguntas son sobre la misma clase de
/// dato (una colección) y typicamente se usan juntas al validar arrays
/// que llegan en un body (ej: "que la lista de roles sea de valores
/// permitidos y sin duplicados").
pub trait Membership<T> {
    fn is_in(&self, value: &T) -> bool;

    fn not_in(&self, value: &T) -> bool {
        !self.is_in(value)
    }

    fn is_distinct(&self) -> bool;
}

/// Cuerpo compartido de `is_distinct`: compara por pares (i, j) con
/// i < j vía `PartialEq`. O(n²), suficiente para los tamaños típicos de
/// un payload de validación (decenas/cientos de elementos). Se invoca
/// dentro de cada `impl` -- misma técnica que `range_len_methods!` en
/// `range.rs`, para no pelear con la ambigüedad de parsing de intentar
/// unificar distintas aridades de genéricos en una sola macro.
macro_rules! distinct_by_pairs {
    ($items:expr) => {{
        let items = $items;
        for i in 0..items.len() {
            for j in (i + 1)..items.len() {
                if items[i] == items[j] {
                    return false;
                }
            }
        }
        true
    }};
}

impl<T: PartialEq> Membership<T> for Vec<T> {
    fn is_in(&self, value: &T) -> bool {
        self.contains(value)
    }

    fn is_distinct(&self) -> bool {
        distinct_by_pairs!(self.as_slice())
    }
}

impl<T: PartialEq> Membership<T> for &[T] {
    fn is_in(&self, value: &T) -> bool {
        self.contains(value)
    }

    fn is_distinct(&self) -> bool {
        distinct_by_pairs!(*self)
    }
}

impl<T: PartialEq, const N: usize> Membership<T> for [T; N] {
    fn is_in(&self, value: &T) -> bool {
        self.contains(value)
    }

    fn is_distinct(&self) -> bool {
        distinct_by_pairs!(self.as_slice())
    }
}

impl<K, V: PartialEq> Membership<V> for HashMap<K, V> {
    fn is_in(&self, value: &V) -> bool {
        self.values().any(|v| v == value)
    }

    fn is_distinct(&self) -> bool {
        let values: Vec<&V> = self.values().collect();
        distinct_by_pairs!(values.as_slice())
    }
}

impl<K, V: PartialEq> Membership<V> for AHashMap<K, V> {
    fn is_in(&self, value: &V) -> bool {
        self.values().any(|v| v == value)
    }

    fn is_distinct(&self) -> bool {
        let values: Vec<&V> = self.values().collect();
        distinct_by_pairs!(values.as_slice())
    }
}

impl<K, V: PartialEq> Membership<V> for BTreeMap<K, V> {
    fn is_in(&self, value: &V) -> bool {
        self.values().any(|v| v == value)
    }

    fn is_distinct(&self) -> bool {
        let values: Vec<&V> = self.values().collect();
        distinct_by_pairs!(values.as_slice())
    }
}
