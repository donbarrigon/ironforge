use crate::error::ForgeError;
use ahash::AHashMap;
use regex::Regex;
use std::sync::{OnceLock, RwLock};

/// Cuenta cuántas de las 4 categorías de carácter (mayúscula, minúscula,
/// dígito, símbolo) aparecen al menos una vez en `value`. Usado por
/// `re_password_mid`/`re_password_strong` -- una sola pasada por los
/// caracteres, no cuatro `.any()` separados.
fn password_category_count(value: &str) -> u8 {
    let (mut upper, mut lower, mut digit, mut symbol) = (false, false, false, false);
    for c in value.chars() {
        if c.is_uppercase() {
            upper = true;
        } else if c.is_lowercase() {
            lower = true;
        } else if c.is_ascii_digit() {
            digit = true;
        } else if !c.is_whitespace() {
            symbol = true;
        }
    }
    upper as u8 + lower as u8 + digit as u8 + symbol as u8
}

// ===============================================================================
// Mensajes por defecto (para la macro `rules!`; :field se inyecta después)
// ===============================================================================
pub const MSG_EMAIL: &str = "el campo :field debe ser un correo electrónico válido";
pub const MSG_PHONE: &str = "el campo :field debe ser un número de teléfono válido";
pub const MSG_PHONE_CODE: &str = "el campo :field debe incluir el código de país (ej: +57...)";
pub const MSG_ALPHA: &str = "el campo :field solo debe contener letras";
pub const MSG_ALPHA_DASH: &str = "el campo :field solo debe contener letras, guiones y guiones bajos";
pub const MSG_ALPHA_SPACES: &str = "el campo :field solo debe contener letras y espacios";
pub const MSG_ALPHA_NUM: &str = "el campo :field solo debe contener letras y números";
pub const MSG_ALPHA_NUM_DASH: &str = "el campo :field solo debe contener letras, números, guiones y guiones bajos";
pub const MSG_ALPHA_NUM_SPACES: &str = "el campo :field solo debe contener letras, números y espacios";
pub const MSG_ALPHA_NUM_DS: &str =
    "el campo :field solo debe contener letras, números, espacios, guiones y guiones bajos";
pub const MSG_SLUG: &str = "el campo :field debe ser un slug válido (minúsculas, números y guiones)";
pub const MSG_HEX_COLOR: &str = "el campo :field debe ser un color hexadecimal válido";
pub const MSG_UUID: &str = "el campo :field debe ser un UUID válido";
pub const MSG_URL: &str = "el campo :field debe ser una URL válida";
pub const MSG_IPV4: &str = "el campo :field debe ser una dirección IPv4 válida";
pub const MSG_IPV6: &str = "el campo :field debe ser una dirección IPv6 válida";
pub const MSG_POSTAL_CODE_CO: &str = "el campo :field debe ser un código postal colombiano válido";
pub const MSG_HASHTAG: &str = "el campo :field debe ser un hashtag válido";
pub const MSG_MENTION: &str = "el campo :field debe ser una mención válida";
pub const MSG_TIME_24H: &str = "el campo :field debe ser una hora válida (HH:MM, 24h)";
pub const MSG_ISO_DATE: &str = "el campo :field debe ser una fecha válida (YYYY-MM-DD)";
pub const MSG_REGEX: &str = "el campo :field tiene un formato inválido";
pub const MSG_JSON: &str = "el campo :field debe ser un JSON válido";
pub const MSG_USERNAME: &str = "el campo :field debe ser un nombre de usuario válido";
pub const MSG_PASSWORD: &str = "el campo :field debe tener al menos 8 caracteres, con letras y números";
pub const MSG_PASSWORD_MID: &str =
    "el campo :field debe tener al menos 8 caracteres y combinar mayúsculas, minúsculas, números o símbolos";
pub const MSG_PASSWORD_STRONG: &str =
    "el campo :field debe tener al menos 12 caracteres, con mayúsculas, minúsculas, números y símbolos";

/// Genera una función privada `$name() -> &'static Regex` que compila el
/// patrón la primera vez que se llama (`OnceLock`, lazy) y reusa la
/// instancia compilada en las llamadas siguientes. Estos patrones son
/// fijos y ya probados por el framework -- nunca fallan en runtime, por
/// eso los métodos del trait de abajo devuelven `bool` y no `Result`.
macro_rules! static_regex {
    ($name:ident, $pattern:expr) => {
        fn $name() -> &'static Regex {
            static RE: OnceLock<Regex> = OnceLock::new();
            RE.get_or_init(|| Regex::new($pattern).expect(concat!("regex inválido: ", stringify!($name))))
        }
    };
}

// Los *alpha* usan \p{L}/\p{M} (letra + marca diacrítica Unicode), no
// [a-zA-Z] -- así "José", "Muñoz" pasan, igual que el default de Laravel.
static_regex!(re_email_pat, r"^[^\s@]+@[^\s@]+\.[^\s@]+$");
static_regex!(re_phone_pat, r"^\d{7,15}$");
static_regex!(re_phone_code_pat, r"^\+\d{1,3}\d{7,15}$");
static_regex!(re_alpha_pat, r"^[\p{L}\p{M}]+$");
static_regex!(re_alpha_dash_pat, r"^[\p{L}\p{M}_-]+$");
static_regex!(re_alpha_spaces_pat, r"^[\p{L}\p{M} ]+$");
static_regex!(re_alpha_num_pat, r"^[\p{L}\p{M}0-9]+$");
static_regex!(re_alpha_num_dash_pat, r"^[\p{L}\p{M}0-9_-]+$");
static_regex!(re_alpha_num_spaces_pat, r"^[\p{L}\p{M}0-9 ]+$");
static_regex!(re_alpha_num_ds_pat, r"^[\p{L}\p{M}0-9 _-]+$");
static_regex!(re_slug_pat, r"^[a-z0-9]+(-[a-z0-9]+)*$");
static_regex!(re_hex_color_pat, r"^#([A-Fa-f0-9]{6}|[A-Fa-f0-9]{3})$");
static_regex!(
    re_uuid_pat,
    r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$"
);
static_regex!(re_url_pat, r"^https?://[^\s/$.?#].[^\s]*$");
static_regex!(
    re_ipv4_pat,
    r"^(25[0-5]|2[0-4]\d|1\d{2}|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d{2}|[1-9]?\d)){3}$"
);
static_regex!(re_ipv6_pat, r"^([0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}$");
static_regex!(re_postal_code_co_pat, r"^\d{6}$");
static_regex!(re_hashtag_pat, r"^#[\p{L}\p{M}0-9_]+$");
static_regex!(re_mention_pat, r"^@[\p{L}\p{M}0-9_]+$");
static_regex!(re_time_24h_pat, r"^([01]\d|2[0-3]):[0-5]\d$");
static_regex!(re_iso_date_pat, r"^\d{4}-\d{2}-\d{2}$");

/// Reglas de formato basadas en regex, precompiladas. Implementado solo
/// para `str` -- `String` y `Cow<'_, str>` lo heredan gratis vía deref
/// coercion (`Deref<Target = str>`), no hace falta repetir el impl.
pub trait ValidatorRegex {
    fn re_email(&self) -> bool;
    fn re_phone(&self) -> bool;
    fn re_phone_code(&self) -> bool;
    fn re_alpha(&self) -> bool;
    fn re_alpha_dash(&self) -> bool;
    fn re_alpha_spaces(&self) -> bool;
    fn re_alpha_num(&self) -> bool;
    fn re_alpha_num_dash(&self) -> bool;
    fn re_alpha_num_spaces(&self) -> bool;
    fn re_alpha_num_ds(&self) -> bool;
    fn re_slug(&self) -> bool;
    fn re_hex_color(&self) -> bool;
    fn re_uuid(&self) -> bool;
    fn re_url(&self) -> bool;
    fn re_ipv4(&self) -> bool;
    fn re_ipv6(&self) -> bool;
    fn re_postal_code_co(&self) -> bool;
    fn re_hashtag(&self) -> bool;
    fn re_mention(&self) -> bool;
    fn re_time_24h(&self) -> bool;
    fn re_iso_date(&self) -> bool;

    /// No es regex -- parsea con `serde_json` y descarta el resultado.
    /// Vive acá para que toda regla de "formato de string" quede bajo un
    /// solo trait, sin importar el mecanismo interno de cada una.
    fn re_json(&self) -> bool;

    /// Empieza con letra, solo permite [letra/dígito/`.`/`_`/`-`], y no
    /// permite puntuación repetida ni al borde (ej: `__`, `..`, `--`,
    /// `_juan`, `juan_`). Un regex podría cubrir el charset, pero las
    /// reglas anti-spam de repetición/bordes quedan ilegibles en regex
    /// -- por eso es imperativa.
    fn re_username(&self) -> bool;

    /// Mínimo 8 caracteres, con al menos una letra y un número.
    fn re_password(&self) -> bool;

    /// Mínimo 8 caracteres, combinando al menos 3 de las 4 categorías:
    /// mayúscula, minúscula, dígito, símbolo.
    fn re_password_mid(&self) -> bool;

    /// Mínimo 12 caracteres, con las 4 categorías presentes: mayúscula,
    /// minúscula, dígito, símbolo.
    fn re_password_strong(&self) -> bool;

    /// Patrón dinámico, dado por el desarrollador con un `name` propio
    /// para cachear. Es el ÚNICO método de este trait que retorna
    /// `Result<bool, ForgeError>`, porque es el único cuyo patrón puede
    /// no compilar (viene como &str en tiempo de llamada, no fue
    /// validado de antemano por el framework como los de arriba).
    ///
    /// - `Err(ForgeError)` -> el patrón no compiló (bug del desarrollador).
    /// - `Ok(false)`       -> compiló bien, pero `self` no matchea.
    /// - `Ok(true)`        -> compiló bien y matchea.
    fn re_ex(&self, name: &str, pattern: &str) -> Result<bool, ForgeError>;
}

impl ValidatorRegex for str {
    fn re_email(&self) -> bool {
        re_email_pat().is_match(self)
    }
    fn re_phone(&self) -> bool {
        re_phone_pat().is_match(self)
    }
    fn re_phone_code(&self) -> bool {
        re_phone_code_pat().is_match(self)
    }
    fn re_alpha(&self) -> bool {
        re_alpha_pat().is_match(self)
    }
    fn re_alpha_dash(&self) -> bool {
        re_alpha_dash_pat().is_match(self)
    }
    fn re_alpha_spaces(&self) -> bool {
        re_alpha_spaces_pat().is_match(self)
    }
    fn re_alpha_num(&self) -> bool {
        re_alpha_num_pat().is_match(self)
    }
    fn re_alpha_num_dash(&self) -> bool {
        re_alpha_num_dash_pat().is_match(self)
    }
    fn re_alpha_num_spaces(&self) -> bool {
        re_alpha_num_spaces_pat().is_match(self)
    }
    fn re_alpha_num_ds(&self) -> bool {
        re_alpha_num_ds_pat().is_match(self)
    }
    fn re_slug(&self) -> bool {
        re_slug_pat().is_match(self)
    }
    fn re_hex_color(&self) -> bool {
        re_hex_color_pat().is_match(self)
    }
    fn re_uuid(&self) -> bool {
        re_uuid_pat().is_match(self)
    }
    fn re_url(&self) -> bool {
        re_url_pat().is_match(self)
    }
    fn re_ipv4(&self) -> bool {
        re_ipv4_pat().is_match(self)
    }
    fn re_ipv6(&self) -> bool {
        re_ipv6_pat().is_match(self)
    }
    fn re_postal_code_co(&self) -> bool {
        re_postal_code_co_pat().is_match(self)
    }
    fn re_hashtag(&self) -> bool {
        re_hashtag_pat().is_match(self)
    }
    fn re_mention(&self) -> bool {
        re_mention_pat().is_match(self)
    }
    fn re_time_24h(&self) -> bool {
        re_time_24h_pat().is_match(self)
    }
    fn re_iso_date(&self) -> bool {
        re_iso_date_pat().is_match(self)
    }

    fn re_json(&self) -> bool {
        serde_json::from_str::<serde_json::Value>(self).is_ok()
    }

    fn re_username(&self) -> bool {
        let mut chars = self.chars().peekable();

        // Debe empezar con una letra Unicode.
        match chars.peek() {
            Some(c) if c.is_alphabetic() => {}
            _ => return false,
        }

        let mut prev: Option<char> = None;
        for c in self.chars() {
            let is_allowed = c.is_alphanumeric() || c == '.' || c == '_' || c == '-';
            if !is_allowed {
                return false;
            }
            // Sin puntuación repetida consecutiva (`__`, `..`, `--`, `._`, etc.)
            if !c.is_alphanumeric() {
                if let Some(p) = prev {
                    if !p.is_alphanumeric() {
                        return false;
                    }
                }
            }
            prev = Some(c);
        }

        // No puede terminar en puntuación.
        !matches!(prev, Some('.') | Some('_') | Some('-'))
    }

    fn re_password(&self) -> bool {
        if self.chars().count() < 8 {
            return false;
        }
        let has_letter = self.chars().any(|c| c.is_alphabetic());
        let has_digit = self.chars().any(|c| c.is_ascii_digit());
        has_letter && has_digit
    }

    fn re_password_mid(&self) -> bool {
        if self.chars().count() < 8 {
            return false;
        }
        password_category_count(self) >= 3
    }

    fn re_password_strong(&self) -> bool {
        if self.chars().count() < 12 {
            return false;
        }
        password_category_count(self) == 4
    }

    fn re_ex(&self, name: &str, pattern: &str) -> Result<bool, ForgeError> {
        // Ruta rápida: ya está cacheado, solo necesitamos lectura.
        {
            let cache = regex_cache().read().expect("regex cache lock envenenado");
            if let Some(re) = cache.get(name) {
                return Ok(re.is_match(self));
            }
        }

        // No estaba cacheado: compilar y registrar. Puede correr más de
        // una vez en carreras concurrentes para el mismo `name` nuevo --
        // no pasa nada, `entry().or_insert()` deja una sola versión al
        // final y el trabajo duplicado ocasional es preferible a
        // serializar todo detrás de un solo write-lock por cada
        // validación.
        let re = Regex::new(pattern)
            .map_err(|e| ForgeError::internal_server_error(ForgeError::INTERNAL_SERVER_ERROR_MSG).caused_by(e))?;

        let is_match = re.is_match(self);

        let mut cache = regex_cache().write().expect("regex cache lock envenenado");
        cache.entry(name.to_string()).or_insert(re);

        Ok(is_match)
    }
}

// ===============================================================================
// Cache global para `re_ex`: patrones dinámicos compilados una sola vez
// por `name` y reusados después. Separado en su propia sección porque es
// estado compartido, a diferencia de los `static_regex!` de arriba que
// son cada uno independiente y fijo.
// ===============================================================================

fn regex_cache() -> &'static RwLock<AHashMap<String, Regex>> {
    static CACHE: OnceLock<RwLock<AHashMap<String, Regex>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(AHashMap::new()))
}
