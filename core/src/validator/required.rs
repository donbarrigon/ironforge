// core/src/validator/required.rs

use std::collections::{BTreeMap, HashMap};

pub const MSG_REQUIRED: &str = "the :field field is required";
pub const MSG_NOT_REQUIRED: &str = "the :field field must not be required";
pub const MSG_EMPTY: &str = "the :field field must be empty";
pub const MSG_NOT_EMPTY: &str = "the :field field must not be empty";

/// Verifica que un valor no sea considerado "vacío" o en su "zero value" según su tipo.
///
/// - **Números:** distinto de cero (`0`).
/// - **Bool:** `true` (`false` se considera ausente).
/// - **Strings:** no vacíos (`""`).
/// - **Option:** `Some(_)`.
/// - **Colecciones:** al menos un elemento.
/// - **Fechas (`chrono`, `time`):** distintas del Unix Epoch (`1970-01-01 00:00:00`).
/// - **`ObjectId`:** distinto de 12 bytes en cero.
/// - **Box:** delega la validación al valor interno.
pub trait Required {
    fn required(&self) -> bool;

    /// Inverso de `required()` -- `true` si el valor está vacío/en su
    /// zero value. Default derivado, ningún impl necesita definirlo.
    fn empty(&self) -> bool {
        !self.required()
    }
}

/// Genera el impl de `Required` para tipos numéricos comparando contra
/// `Default::default()` -- cubre enteros (`0`) y flotantes (`0.0`) con
/// una sola rama, sin repetir la lógica por tipo.
macro_rules! impl_required_numeric {
    ($($t:ty),* $(,)?) => {
        $(
            impl Required for $t {
                fn required(&self) -> bool {
                    *self != <$t>::default()
                }
            }
        )*
    };
}

impl_required_numeric!(i8, i16, i32, i64, i128, isize, u8, u16, u32, u64, u128, usize, f32, f64,);

// =======================================
// Booleano
// =======================================
impl Required for bool {
    fn required(&self) -> bool {
        *self
    }
}

// =======================================
// Strings -- solo `str`. `String` y `Cow<'_, str>` lo heredan gratis vía
// deref coercion (`Deref<Target = str>`), no hace falta repetir el impl.
// =======================================
impl Required for str {
    fn required(&self) -> bool {
        !self.is_empty()
    }
}

// =======================================
// Option
// =======================================
impl<T> Required for Option<T> {
    fn required(&self) -> bool {
        self.is_some()
    }
}

/// Cuerpo compartido para colecciones donde "requerido" es simplemente
/// "no vacío" (`!self.is_empty()`). Se invoca dentro de cada `impl` con
/// el header de genéricos que corresponda -- misma técnica que
/// `range_len_methods!` en `range.rs`, para evitar la ambigüedad de
/// parsing de unificar distintas aridades de genéricos en una macro.
macro_rules! required_is_empty {
    () => {
        fn required(&self) -> bool {
            !self.is_empty()
        }
    };
}

// =======================================
// Vec / Slice / Array
// =======================================
impl<T> Required for Vec<T> {
    required_is_empty!();
}

impl<T> Required for &[T] {
    required_is_empty!();
}

impl<T, const N: usize> Required for [T; N] {
    fn required(&self) -> bool {
        N != 0
    }
}

// =======================================
// Maps
// =======================================
impl<K, V> Required for HashMap<K, V> {
    required_is_empty!();
}

impl<K, V> Required for BTreeMap<K, V> {
    required_is_empty!();
}

impl<K, V> Required for ahash::AHashMap<K, V> {
    required_is_empty!();
}

// =======================================
// Box (delega al interior)
// =======================================
impl<T: Required> Required for Box<T> {
    fn required(&self) -> bool {
        self.as_ref().required()
    }
}

// =======================================
// serde_json::Value -- cada variante tiene su propia noción de "vacío",
// no encaja en ningún patrón compartido con los demás tipos.
// =======================================
impl Required for serde_json::Value {
    fn required(&self) -> bool {
        match self {
            serde_json::Value::Null => false,
            serde_json::Value::Bool(_) => true,
            serde_json::Value::Number(n) => n.as_f64().map_or(true, |v| v != 0.0),
            serde_json::Value::String(s) => !s.is_empty(),
            serde_json::Value::Array(a) => !a.is_empty(),
            serde_json::Value::Object(o) => !o.is_empty(),
        }
    }
}

// =======================================
// chrono / time -- cada tipo construye su propio "epoch" de forma
// distinta (algunos tienen una constante `UNIX_EPOCH`, otros necesitan
// `from_ymd_opt(...).unwrap()`), así que no comparten un cuerpo de
// macro; quedan explícitos.
// =======================================
impl Required for chrono::NaiveDate {
    fn required(&self) -> bool {
        *self != chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()
    }
}

impl Required for chrono::NaiveDateTime {
    fn required(&self) -> bool {
        *self
            != chrono::NaiveDateTime::new(
                chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap(),
                chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap(),
            )
    }
}

impl<Tz: chrono::TimeZone> Required for chrono::DateTime<Tz> {
    fn required(&self) -> bool {
        *self != chrono::DateTime::UNIX_EPOCH
    }
}

impl Required for time::Date {
    fn required(&self) -> bool {
        *self != time::Date::from_calendar_date(1970, time::Month::January, 1).unwrap()
    }
}

impl Required for time::OffsetDateTime {
    fn required(&self) -> bool {
        *self != time::OffsetDateTime::UNIX_EPOCH
    }
}

impl Required for time::PrimitiveDateTime {
    fn required(&self) -> bool {
        *self
            != time::PrimitiveDateTime::new(
                time::Date::from_calendar_date(1970, time::Month::January, 1).unwrap(),
                time::Time::MIDNIGHT,
            )
    }
}

// =======================================
// bson
// =======================================
impl Required for mongodb::bson::oid::ObjectId {
    fn required(&self) -> bool {
        self.bytes() != [0; 12]
    }
}
