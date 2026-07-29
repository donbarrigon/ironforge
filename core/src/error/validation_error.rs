use crate::{error::ForgeError, lang::lang::translate};
use serde::ser::{Serialize, SerializeMap, Serializer};

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

    pub fn push(&mut self, message: String, placeholders: Placeholders) {
        self.messages.push((message, placeholders));
    }
}

pub struct ValidationError {
    pub errors: Vec<FieldError>,
    pub locale: String,
}

impl ValidationError {
    pub fn new(locale: String) -> ValidationError {
        ValidationError {
            errors: Vec::new(),
            locale,
        }
    }

    pub fn push(&mut self, field: String) -> &mut FieldError {
        if let Some(idx) = self.errors.iter().position(|e| e.field == field) {
            return &mut self.errors[idx];
        }
        self.errors.push(FieldError::new(field));
        self.errors.last_mut().unwrap()
    }

    fn translate(&mut self) {
        for error in &mut self.errors {
            for message in &mut error.messages {
                message.0 = translate(self.locale.clone(), message.0.clone());
                for placeholder in &mut message.1 {
                    let ph = format!(":{}", placeholder.0);
                    message.0 = message.0.replace(&ph, &placeholder.1);
                }
            }
        }
    }

    pub fn errors(&mut self) -> Result<(), ForgeError> {
        if self.errors.len() > 0 {
            self.translate();
            return Err(ForgeError::unprocessable_entity(ForgeError::UNPROCESSABLE_ENTITY_MSG).with_data(&*self));
        }
        Ok(())
    }
}

/// Serializa como {"campo1": ["msg1", "msg2"], "campo2": [...]}.
/// Solo se emite el texto ya traducido e interpolado (message.0) -- los
/// placeholders (message.1) ya cumplieron su función dentro de translate()
/// y no tienen motivo para viajar en la respuesta.
impl Serialize for ValidationError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.errors.len()))?;
        for field_error in &self.errors {
            let messages: Vec<&str> = field_error.messages.iter().map(|(text, _)| text.as_str()).collect();
            map.serialize_entry(&field_error.field, &messages)?;
        }
        map.end()
    }
}
