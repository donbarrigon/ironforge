use crate::{error::ForgeError, lang::lang::translate};
use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};

pub type Placeholder = (&'static str, String);
pub type Placeholders = Vec<Placeholder>;

pub struct FieldError {
    pub field: String,
    pub messages: Vec<(String, Placeholders)>,
}

impl FieldError {
    pub fn new(field: String) -> FieldError {
        FieldError {
            field,
            messages: Vec::new(),
        }
    }

    pub fn push_message(&mut self, message: String, placeholders: Placeholders) {
        self.messages.push((message, placeholders));
    }

    pub fn push(&mut self, e: (String, Placeholders)) {
        self.messages.push(e);
    }
}

pub struct ValidationError {
    errors: Vec<FieldError>,                   // error de validación para un solo struct
    collection: Vec<(usize, ValidationError)>, // error de validación para una colección de structs
}

impl ValidationError {
    pub fn new() -> ValidationError {
        ValidationError {
            errors: Vec::new(),
            collection: Vec::new(),
        }
    }

    /// Agrega un nuevo campo a la validación retorna la referencia mutable al FieldError creado o existente
    pub fn push_field(&mut self, field: String) -> &mut FieldError {
        if let Some(idx) = self.errors.iter().position(|e| e.field == field) {
            return &mut self.errors[idx];
        }
        self.errors.push(FieldError::new(field));
        self.errors.last_mut().unwrap()
    }

    /// Agrega un nuevo error a un campo
    pub fn push(&mut self, field: String, message: String, placeholders: Placeholders) {
        self.push_field(field).push((message, placeholders));
    }

    /// Agrega un nuevo error a la colección de errores
    pub fn push_error(&mut self, i: usize, ve: ValidationError) {
        self.collection.push((i, ve));
    }

    fn translate(&mut self, locale: String) {
        for error in &mut self.errors {
            for message in &mut error.messages {
                message.0 = translate(locale.clone(), message.0.clone());
                for placeholder in &mut message.1 {
                    let ph = format!(":{}", placeholder.0);
                    message.0 = message.0.replace(&ph, &placeholder.1);
                }
            }
        }
        // NOTA: no traduce recursivamente self.collection[..].1 -- si un
        // ValidationError de colección tiene sus propios mensajes, quedan
        // sin traducir a menos que se agregue ese recorrido acá también.
    }

    pub fn errors(&mut self, locale: String) -> Result<(), ForgeError> {
        if self.errors.len() > 0 {
            self.translate(locale);
            return Err(ForgeError::unprocessable_entity(ForgeError::UNPROCESSABLE_ENTITY_MSG).with_data(&*self));
        }

        if self.collection.len() > 0 {
            for col in self.collection.iter_mut() {
                col.1.translate(locale.clone());
            }
            return Err(ForgeError::unprocessable_entity(ForgeError::UNPROCESSABLE_ENTITY_MSG).with_data(&*self));
        }

        Ok(())
    }

    pub fn has_errors(&self) -> bool {
        self.errors.len() > 0 || self.collection.len() > 0
    }
}

/// Helper interno: serializa un slice de FieldError como
/// {"campo1": ["msg1", "msg2"], "campo2": [...]}. Se reusa tanto para el
/// caso simple como para el campo "errors" de cada entrada de colección,
/// para no duplicar la lógica de armado del map.
struct FieldMap<'a>(&'a [FieldError]);

impl<'a> Serialize for FieldMap<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for field_error in self.0 {
            let messages: Vec<&str> = field_error.messages.iter().map(|(text, _)| text.as_str()).collect();
            map.serialize_entry(&field_error.field, &messages)?;
        }
        map.end()
    }
}

/// Helper interno: una entrada de colección, {"index": i, "errors": {...}}.
/// NOTA: "errors" acá solo trae los campos propios (ve.errors) del
/// ValidationError anidado -- no baja recursivamente por ve.collection.
struct CollectionEntry<'a> {
    index: usize,
    errors: FieldMap<'a>,
}

impl<'a> Serialize for CollectionEntry<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("index", &self.index)?;
        map.serialize_entry("errors", &self.errors)?;
        map.end()
    }
}

/// Dos formas posibles de salida:
/// - Caso simple:     {"errors": {"campo1": [...], "campo2": [...]}}
/// - Caso colección:  [{"index": 0, "errors": {...}}, {"index": 2, "errors": {...}}]
/// - Sin nada:         {} (no debería llegar acá en la práctica -- errors()
///                      corta antes con Ok(()) si no hay nada que reportar)
impl Serialize for ValidationError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.errors.len() > 0 {
            let mut map = serializer.serialize_map(Some(1))?;
            map.serialize_entry("errors", &FieldMap(&self.errors))?;
            return map.end();
        }

        if self.collection.len() > 0 {
            let mut seq = serializer.serialize_seq(Some(self.collection.len()))?;
            for (index, ve) in &self.collection {
                seq.serialize_element(&CollectionEntry {
                    index: *index,
                    errors: FieldMap(&ve.errors),
                })?;
            }
            return seq.end();
        }

        let map = serializer.serialize_map(Some(0))?;
        map.end()
    }
}
