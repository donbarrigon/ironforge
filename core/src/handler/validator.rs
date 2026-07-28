use crate::error::ForgeError;

/// Cualquier struct que `get_body`/`get_body::<Vec<T>>` pueda recibir como
/// destino de deserialización, con validación propia.
///
/// - `prepare_for_validation`: hook para normalizar datos antes de validar
///   (trim, defaults, casing, etc). Tiene implementación por defecto (no-op)
///   -- no todo struct necesita normalizar nada.
/// - `rules`: acá van las reglas de validación reales. La macro `rules!`
///   genera el cuerpo de esta función, y es responsabilidad de ese cuerpo
///   (generado o escrito a mano) llamar `self.prepare_for_validation()`
///   como primer paso. `get_body` solo llama `rules()`, nunca
///   `prepare_for_validation()` directamente.
pub trait Validator {
    fn prepare_for_validation(&mut self) -> Result<(), ForgeError> {
        Ok(())
    }

    fn rules(&mut self) -> Result<(), ForgeError>;
}

/// Permite `get_body::<Vec<Item>>()` sin necesitar una función aparte para
/// colecciones: cada elemento corre sus propias reglas (que a su vez llaman
/// su propio `prepare_for_validation`), y el primer error corta el resto.
impl<T: Validator> Validator for Vec<T> {
    fn rules(&mut self) -> Result<(), ForgeError> {
        for item in self.iter_mut() {
            item.rules()?;
        }
        Ok(())
    }
}
