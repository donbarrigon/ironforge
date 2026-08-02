// range_time.rs

pub const MSG_AFTER: &str = "the :field field must be after :after";
pub const MSG_NOT_AFTER: &str = "the :field field must not be after :after";
pub const MSG_BEFORE: &str = "the :field field must be before :before";
pub const MSG_NOT_BEFORE: &str = "the :field field must not be before :before";
pub const MSG_AFTER_EQ: &str = "the :field field must be on or after :after_eq";
pub const MSG_NOT_AFTER_EQ: &str = "the :field field must not be on or after :after_eq";
pub const MSG_BEFORE_EQ: &str = "the :field field must be on or before :before_eq";
pub const MSG_NOT_BEFORE_EQ: &str = "the :field field must not be on or before :before_eq";
pub const MSG_WITHIN: &str = "the :field field must be within :min and :max";
pub const MSG_NOT_WITHIN: &str = "the :field field must not be within :min and :max";
pub const MSG_WITHIN_EXCLUSIVE: &str = "the :field field must be strictly within :min and :max";
pub const MSG_NOT_WITHIN_EXCLUSIVE: &str = "the :field field must not be strictly within :min and :max";
pub const MSG_BEYOND: &str = "the :field field must be beyond the range :min - :max";
pub const MSG_NOT_BEYOND: &str = "the :field field must not be beyond the range :min - :max";

/// Valida rangos de tiempo con nombres semánticos.
///
/// - `after` / `before` — comparación estricta.
/// - `after_eq` / `before_eq` — comparación inclusiva.
/// - `within` — dentro del rango [start, end] (inclusivo).
/// - `within_exclusive` — dentro del rango (start, end) (exclusivo).
/// - `beyond` — fuera del rango [start, end] (inclusivo); `start` y
///   `end` cuentan como dentro del rango, no como fuera.
///
/// Trait independiente de `Range` -- las fechas/duraciones NO
/// implementan `Range`, así que no hay choque de nombres (`between`
/// pasa a llamarse `within`, `outside` pasa a llamarse `beyond`) ni
/// ambigüedad de qué `MSG_*` usar cuando la macro `rules!` resuelve el
/// mensaje por defecto a partir del nombre del método.
pub trait RangeTime {
    fn after(&self, other: Self) -> bool;
    fn before(&self, other: Self) -> bool;
    fn after_eq(&self, other: Self) -> bool;
    fn before_eq(&self, other: Self) -> bool;
    fn within(&self, start: Self, end: Self) -> bool;
    fn within_exclusive(&self, start: Self, end: Self) -> bool;
    fn beyond(&self, start: Self, end: Self) -> bool;
}

/// Genera el impl completo de `RangeTime` para un tipo que ya implementa
/// `Ord` (o al menos `cmp`), usando siempre el mismo cuerpo. Cada línea
/// de invocación dice explícitamente qué tipo queda cubierto -- no hay
/// nada oculto, solo se evita repetir los 7 métodos por cada tipo.
macro_rules! impl_range_time {
    ($($t:ty),* $(,)?) => {
        $(
            impl RangeTime for $t {
                fn after(&self, other: Self) -> bool {
                    self.cmp(&other).is_gt()
                }

                fn before(&self, other: Self) -> bool {
                    self.cmp(&other).is_lt()
                }

                fn after_eq(&self, other: Self) -> bool {
                    self.cmp(&other).is_ge()
                }

                fn before_eq(&self, other: Self) -> bool {
                    self.cmp(&other).is_le()
                }

                fn within(&self, start: Self, end: Self) -> bool {
                    self.cmp(&start).is_ge() && self.cmp(&end).is_le()
                }

                fn within_exclusive(&self, start: Self, end: Self) -> bool {
                    self.cmp(&start).is_gt() && self.cmp(&end).is_lt()
                }

                fn beyond(&self, start: Self, end: Self) -> bool {
                    self.cmp(&start).is_lt() || self.cmp(&end).is_gt()
                }
            }
        )*
    };
}

// ===============================================================================
// chrono
// ===============================================================================
impl_range_time!(chrono::NaiveDate, chrono::NaiveDateTime, chrono::Duration,);

// ===============================================================================
// time
// ===============================================================================
impl_range_time!(
    time::Date,
    time::OffsetDateTime,
    time::PrimitiveDateTime,
    time::Duration,
);

// ===============================================================================
// std
// ===============================================================================
impl_range_time!(std::time::Duration);

// ===============================================================================
// chrono::DateTime<Tz> queda aparte: es genérico sobre Tz, la macro de arriba
// solo acepta tipos concretos ($t:ty sin parámetros libres que resolver).
// ===============================================================================
impl<Tz: chrono::TimeZone> RangeTime for chrono::DateTime<Tz> {
    fn after(&self, other: Self) -> bool {
        self.cmp(&other).is_gt()
    }

    fn before(&self, other: Self) -> bool {
        self.cmp(&other).is_lt()
    }

    fn after_eq(&self, other: Self) -> bool {
        self.cmp(&other).is_ge()
    }

    fn before_eq(&self, other: Self) -> bool {
        self.cmp(&other).is_le()
    }

    fn within(&self, start: Self, end: Self) -> bool {
        self.cmp(&start).is_ge() && self.cmp(&end).is_le()
    }

    fn within_exclusive(&self, start: Self, end: Self) -> bool {
        self.cmp(&start).is_gt() && self.cmp(&end).is_lt()
    }

    fn beyond(&self, start: Self, end: Self) -> bool {
        self.cmp(&start).is_lt() || self.cmp(&end).is_gt()
    }
}
