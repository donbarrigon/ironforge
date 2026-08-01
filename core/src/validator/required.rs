// core/src/validator/required.rs

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};

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
}

// =======================================
// Enteros con signo
// =======================================
impl Required for i8 {
    fn required(&self) -> bool {
        *self != 0
    }
}

impl Required for i16 {
    fn required(&self) -> bool {
        *self != 0
    }
}

impl Required for i32 {
    fn required(&self) -> bool {
        *self != 0
    }
}

impl Required for i64 {
    fn required(&self) -> bool {
        *self != 0
    }
}

impl Required for i128 {
    fn required(&self) -> bool {
        *self != 0
    }
}

impl Required for isize {
    fn required(&self) -> bool {
        *self != 0
    }
}

// =======================================
// Enteros sin signo
// =======================================
impl Required for u8 {
    fn required(&self) -> bool {
        *self != 0
    }
}

impl Required for u16 {
    fn required(&self) -> bool {
        *self != 0
    }
}

impl Required for u32 {
    fn required(&self) -> bool {
        *self != 0
    }
}

impl Required for u64 {
    fn required(&self) -> bool {
        *self != 0
    }
}

impl Required for u128 {
    fn required(&self) -> bool {
        *self != 0
    }
}

impl Required for usize {
    fn required(&self) -> bool {
        *self != 0
    }
}

// =======================================
// Flotantes
// =======================================
impl Required for f32 {
    fn required(&self) -> bool {
        *self != 0.0
    }
}

impl Required for f64 {
    fn required(&self) -> bool {
        *self != 0.0
    }
}

// =======================================
// Booleano
// =======================================
impl Required for bool {
    fn required(&self) -> bool {
        *self
    }
}

// =======================================
// Strings
// =======================================
impl Required for &str {
    fn required(&self) -> bool {
        !self.is_empty()
    }
}

impl Required for String {
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

// =======================================
// Vec
// =======================================
impl<T> Required for Vec<T> {
    fn required(&self) -> bool {
        !self.is_empty()
    }
}

// =======================================
// Slice
// =======================================
impl<T> Required for &[T] {
    fn required(&self) -> bool {
        !self.is_empty()
    }
}

// =======================================
// Array (const genérico)
// =======================================
impl<T, const N: usize> Required for [T; N] {
    fn required(&self) -> bool {
        N != 0
    }
}

// =======================================
// Maps
// =======================================
impl<K, V> Required for HashMap<K, V> {
    fn required(&self) -> bool {
        !self.is_empty()
    }
}

impl<K, V> Required for BTreeMap<K, V> {
    fn required(&self) -> bool {
        !self.is_empty()
    }
}

impl<K, V> Required for ahash::AHashMap<K, V> {
    fn required(&self) -> bool {
        !self.is_empty()
    }
}

// =======================================
// Cow
// =======================================
impl Required for Cow<'_, str> {
    fn required(&self) -> bool {
        !self.is_empty()
    }
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
// serde_json::Value
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
// chrono
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

// =======================================
// time
// =======================================
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
