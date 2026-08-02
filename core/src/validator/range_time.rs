/// Valida rangos de tiempo con nombres semánticos.
///
/// - `after` / `before` — comparación estricta.
/// - `after_eq` / `before_eq` — comparación inclusiva.
/// - `between` — dentro del rango [start, end] (inclusivo).
/// - `between_exclusive` — dentro del rango (start, end) (exclusivo).
/// - `outside` — fuera del rango [start, end] (inclusivo); `start` y
///   `end` cuentan como dentro del rango, no como fuera.
pub trait RangeTime {
    fn after(&self, other: Self) -> bool;
    fn before(&self, other: Self) -> bool;
    fn after_eq(&self, other: Self) -> bool;
    fn before_eq(&self, other: Self) -> bool;
    fn between(&self, start: Self, end: Self) -> bool;
    fn between_exclusive(&self, start: Self, end: Self) -> bool;
    fn outside(&self, start: Self, end: Self) -> bool;
}

/// Genera el impl completo de `RangeTime` para un tipo que ya implementa
/// `Ord` (o al menos `cmp`), usando siempre el mismo cuerpo. Cada línea de
/// invocación dice explícitamente qué tipo queda cubierto -- no hay nada
/// oculto, solo se evita repetir los 7 métodos por cada tipo.
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

                fn between(&self, start: Self, end: Self) -> bool {
                    self.cmp(&start).is_ge() && self.cmp(&end).is_le()
                }

                fn between_exclusive(&self, start: Self, end: Self) -> bool {
                    self.cmp(&start).is_gt() && self.cmp(&end).is_lt()
                }

                fn outside(&self, start: Self, end: Self) -> bool {
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

    fn between(&self, start: Self, end: Self) -> bool {
        self.cmp(&start).is_ge() && self.cmp(&end).is_le()
    }

    fn between_exclusive(&self, start: Self, end: Self) -> bool {
        self.cmp(&start).is_gt() && self.cmp(&end).is_lt()
    }

    fn outside(&self, start: Self, end: Self) -> bool {
        self.cmp(&start).is_lt() || self.cmp(&end).is_gt()
    }
}
