//! Conversión de formato de strings: snake_case, camelCase, PascalCase,
//! kebab-case, iniciales, plurales en inglés y títulos en español/inglés.
//!
//! Se usa importando el trait, que queda disponible en `String` y `&str`:
//!
//! ```ignore
//! use ironforge::utils::str::StrFormat;
//!
//! assert_eq!("ProductCategory".to_snake_case(), "product_category");
//! assert_eq!("user_profile".pluralize_identifier(), "user_profiles");
//! ```
//!
//! Rendimiento: las palabras se recorren como slices del string original
//! (sin `Vec<String>` intermedio) y cada conversión escribe en un único
//! `String` con capacidad reservada. El texto ASCII usa un camino rápido
//! (copia + minúsculas en el sitio).

pub trait StrFormat {
    /// 🐍 `snake_case`. Ej: `ProductCategory` => `product_category`.
    fn to_snake_case(&self) -> String;
    /// 🐫 `camelCase`. Ej: `product_category` => `productCategory`.
    fn to_camel_case(&self) -> String;
    /// 🐘 `PascalCase`. Ej: `product-category` => `ProductCategory`.
    fn to_pascal_case(&self) -> String;
    /// 🍢 `kebab-case`. Ej: `ProductCategory` => `product-category`.
    fn to_kebab_case(&self) -> String;
    /// Variable corta con las iniciales de cada palabra en minúscula.
    /// Ej: `ProductCategory` => `pc`, `user_profile` => `up`.
    fn to_var(&self) -> String;
    /// Pluraliza una palabra según las reglas del inglés (sensible a mayúsculas,
    /// igual que la versión de Go).
    fn pluralize(&self) -> String;
    /// Pluraliza la última palabra de un identificador manteniendo su formato
    /// (snake_case, camelCase, PascalCase o kebab-case).
    /// Ej: `ProductCategory` => `ProductCategories`.
    fn pluralize_identifier(&self) -> String;
    /// Título en español: primera letra de cada palabra en mayúscula, salvo
    /// artículos, preposiciones y conjunciones (excepto si es la primera palabra).
    /// Ej: `el señor de los anillos` => `El Señor de los Anillos`.
    fn to_title_case_es(&self) -> String;
    /// Título en inglés: igual que [`StrFormat::to_title_case_es`] con las
    /// palabras menores del inglés.
    /// Ej: `the lord of the rings` => `The Lord of the Rings`.
    fn to_title_case_en(&self) -> String;
}

impl StrFormat for str {
    fn to_snake_case(&self) -> String {
        join_words(self, Case::Snake)
    }

    fn to_camel_case(&self) -> String {
        join_words(self, Case::Camel)
    }

    fn to_pascal_case(&self) -> String {
        join_words(self, Case::Pascal)
    }

    fn to_kebab_case(&self) -> String {
        join_words(self, Case::Kebab)
    }

    fn to_var(&self) -> String {
        let mut out = String::new();
        for word in Words::new(self) {
            if let Some(c) = word.chars().next() {
                out.extend(c.to_lowercase());
            }
        }
        out
    }

    fn pluralize(&self) -> String {
        let w = self;
        if w.is_empty() {
            return String::new();
        }
        if let Some(plural) = irregular_plural(w) {
            return plural.to_string();
        }
        if let Some(stem) = w.strip_suffix('y') {
            if stem.chars().next_back().is_some_and(is_vowel) {
                return concat(w, "s");
            }
            return concat(stem, "ies");
        }
        if w.ends_with(['s', 'x', 'z']) || w.ends_with("ch") || w.ends_with("sh") {
            return concat(w, "es");
        }
        if let Some(stem) = w.strip_suffix('f') {
            return concat(stem, "ves");
        }
        if let Some(stem) = w.strip_suffix("fe") {
            return concat(stem, "ves");
        }
        concat(w, "s")
    }

    fn pluralize_identifier(&self) -> String {
        if self.is_empty() {
            return String::new();
        }
        let case = detect_case(self);
        let mut out = String::with_capacity(self.len() + 4);
        let mut words = Words::new(self).peekable();
        let mut index = 0;
        while let Some(word) = words.next() {
            if words.peek().is_none() {
                // Igual que en Go: se pluraliza la palabra ya en minúsculas.
                let mut lower = String::with_capacity(word.len());
                push_lower(&mut lower, word);
                push_word(&mut out, &lower.pluralize(), case, index);
            } else {
                push_word(&mut out, word, case, index);
            }
            index += 1;
        }
        if index == 0 {
            // Solo separadores: se devuelve tal cual, como en Go.
            return self.to_string();
        }
        out
    }

    fn to_title_case_es(&self) -> String {
        title_case(self, is_minor_es)
    }

    fn to_title_case_en(&self) -> String {
        title_case(self, is_minor_en)
    }
}

// ===============================================================================
// Plurales irregulares
// ===============================================================================

/// Plural irregular en inglés, o `None` si la palabra sigue las reglas normales.
/// Un `match` sobre `&str` no necesita inicializar un mapa ni hashear.
pub fn irregular_plural(word: &str) -> Option<&'static str> {
    Some(match word {
        "person" => "people",
        "child" => "children",
        "foot" => "feet",
        "tooth" => "teeth",
        "mouse" => "mice",
        "man" => "men",
        "woman" => "women",
        "ox" => "oxen",
        "cactus" => "cacti",
        "focus" => "foci",
        "analysis" => "analyses",
        "thesis" => "theses",
        "crisis" => "crises",
        "diagnosis" => "diagnoses",
        "appendix" => "appendices",
        "vertex" => "vertices",
        "index" => "indices",
        "matrix" => "matrices",
        "axis" => "axes",
        "basis" => "bases",
        "fungus" => "fungi",
        "radius" => "radii",
        "alumnus" => "alumni",
        "curriculum" => "curricula",
        "datum" => "data",
        "medium" => "media",
        "forum" => "fora",
        "bacterium" => "bacteria",
        "syllabus" => "syllabi",
        "criterion" => "criteria",
        "aquarium" => "aquaria",
        "stadium" => "stadia",
        "stimulus" => "stimuli",
        "die" => "dice",
        "formula" => "formulae",
        "genus" => "genera",
        // No cambian
        "bison" => "bison",
        "deer" => "deer",
        "sheep" => "sheep",
        "salmon" => "salmon",
        "aircraft" => "aircraft",
        "series" => "series",
        "species" => "species",
        "fish" => "fish",
        "trousers" => "trousers",
        "scissors" => "scissors",
        "clothes" => "clothes",
        "news" => "news",
        _ => return None,
    })
}

// ===============================================================================
// Palabras menores para títulos
// ===============================================================================

/// Artículos, preposiciones y conjunciones del español (en minúscula).
fn is_minor_es(w: &str) -> bool {
    matches!(
        w,
        // Artículos y contracciones
        "el" | "la" | "los" | "las" | "lo" | "un" | "una" | "unos" | "unas" | "al" | "del"
        // Preposiciones
        | "a" | "ante" | "bajo" | "con" | "contra" | "de" | "desde" | "durante" | "en" | "entre"
        | "hacia" | "hasta" | "mediante" | "para" | "por" | "según" | "sin" | "so" | "sobre"
        | "tras" | "versus" | "vía"
        // Conjunciones
        | "y" | "e" | "o" | "u" | "ni" | "que" | "pero" | "sino"
    )
}

/// Artículos, preposiciones cortas y conjunciones del inglés (en minúscula).
fn is_minor_en(w: &str) -> bool {
    matches!(
        w,
        // Artículos
        "a" | "an" | "the"
        // Conjunciones
        | "and" | "but" | "or" | "nor" | "for" | "so" | "yet"
        // Preposiciones
        | "as" | "at" | "by" | "from" | "in" | "into" | "of" | "off" | "on" | "onto" | "over"
        | "per" | "to" | "up" | "via" | "vs" | "with"
    )
}

// ===============================================================================
// Internos
// ===============================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
enum Case {
    Snake,
    Kebab,
    Camel,
    Pascal,
}

/// Recorre las palabras de un identificador sin importar su formato, como
/// `splitWords` en Go: separa por `-`, `_`, espacios y antes de una mayúscula
/// precedida de minúscula o dígito. Devuelve slices del string original (sin
/// pasar a minúsculas): cada conversión aplica el caso al escribir.
struct Words<'a> {
    rest: &'a str,
}

impl<'a> Words<'a> {
    fn new(s: &'a str) -> Self {
        Self { rest: s }
    }
}

impl<'a> Iterator for Words<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let rest = self.rest.trim_start_matches(is_separator);
        if rest.is_empty() {
            self.rest = rest;
            return None;
        }
        let mut prev: Option<char> = None;
        for (i, c) in rest.char_indices() {
            let boundary =
                is_separator(c) || (c.is_uppercase() && prev.is_some_and(|p| p.is_lowercase() || p.is_numeric()));
            if boundary && i > 0 {
                self.rest = &rest[i..];
                return Some(&rest[..i]);
            }
            prev = Some(c);
        }
        self.rest = "";
        Some(rest)
    }
}

#[inline]
fn is_separator(c: char) -> bool {
    c == '-' || c == '_' || c.is_whitespace()
}

fn join_words(s: &str, case: Case) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for (index, word) in Words::new(s).enumerate() {
        push_word(&mut out, word, case, index);
    }
    out
}

/// Escribe una palabra con su separador y su caso según el formato.
#[inline]
fn push_word(out: &mut String, word: &str, case: Case, index: usize) {
    match case {
        Case::Snake | Case::Kebab => {
            if index > 0 {
                out.push(if case == Case::Snake { '_' } else { '-' });
            }
            push_lower(out, word);
        }
        Case::Camel if index == 0 => push_lower(out, word),
        Case::Camel | Case::Pascal => push_capitalized(out, word),
    }
}

/// Agrega `s` en minúsculas. ASCII: copia + minúsculas en el sitio.
#[inline]
fn push_lower(out: &mut String, s: &str) {
    if s.is_ascii() {
        let start = out.len();
        out.push_str(s);
        out[start..].make_ascii_lowercase();
    } else {
        for c in s.chars() {
            out.extend(c.to_lowercase());
        }
    }
}

/// Agrega `s` con la primera letra en mayúscula y el resto en minúscula.
#[inline]
fn push_capitalized(out: &mut String, s: &str) {
    let mut chars = s.chars();
    if let Some(first) = chars.next() {
        out.extend(first.to_uppercase());
        push_lower(out, chars.as_str());
    }
}

fn detect_case(s: &str) -> Case {
    if s.contains('_') {
        return Case::Snake;
    }
    if s.contains('-') {
        return Case::Kebab;
    }
    match s.char_indices().find(|(_, c)| c.is_uppercase()) {
        Some((0, _)) => Case::Pascal,
        Some(_) => Case::Camel,
        None => Case::Snake,
    }
}

/// Título: conserva los espacios y la puntuación originales; cada palabra
/// (separada por espacios) queda con mayúscula inicial salvo las menores.
/// La primera palabra siempre va en mayúscula. La puntuación pegada a la
/// palabra (`¿`, `(`, `,`...) no impide reconocerla: `¿de` => `¿de`.
fn title_case(s: &str, is_minor: fn(&str) -> bool) -> String {
    let mut out = String::with_capacity(s.len());
    let mut core_lower = String::new(); // buffer reutilizado entre palabras
    let mut first = true;
    let mut rest = s;

    while !rest.is_empty() {
        let ws = rest.find(|c: char| !c.is_whitespace()).unwrap_or(rest.len());
        out.push_str(&rest[..ws]);
        rest = &rest[ws..];
        if rest.is_empty() {
            break;
        }

        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let word = &rest[..end];
        rest = &rest[end..];

        let core = word.trim_matches(|c: char| !c.is_alphanumeric());
        core_lower.clear();
        push_lower(&mut core_lower, core);

        if !first && is_minor(&core_lower) {
            push_lower(&mut out, word);
        } else {
            push_title_word(&mut out, word);
        }
        if !core.is_empty() {
            first = false;
        }
    }
    out
}

/// Mayúscula en la primera letra o dígito (saltando puntuación inicial) y el resto en minúscula.
/// También va en mayúscula la letra que sigue a:
/// - un guion: `maría-josé` => `María-José`.
/// - un apóstrofo precedido de una sola letra: `o'brien` => `O'Brien`, `d'angelo` => `D'Angelo`.
///   Con más letras antes no aplica, para no romper contracciones: `don't` => `Don't`.
#[inline]
fn push_title_word(out: &mut String, word: &str) {
    let mut upper_next = true;
    let mut segment_len = 0usize; // letras desde el inicio o el último guion/apóstrofo
    for c in word.chars() {
        if c.is_alphanumeric() {
            if upper_next {
                out.extend(c.to_uppercase());
                upper_next = false;
            } else {
                out.extend(c.to_lowercase());
            }
            segment_len += 1;
        } else {
            out.push(c);
            let starts_segment = match c {
                '-' => true,
                '\'' | '’' => segment_len == 1,
                _ => false,
            };
            if starts_segment {
                upper_next = true;
                segment_len = 0;
            }
        }
    }
}

#[inline]
fn is_vowel(c: char) -> bool {
    matches!(c, 'a' | 'e' | 'i' | 'o' | 'u')
}

#[inline]
fn concat(a: &str, b: &str) -> String {
    let mut s = String::with_capacity(a.len() + b.len());
    s.push_str(a);
    s.push_str(b);
    s
}

// ===============================================================================
// Tests
// ===============================================================================

#[cfg(test)]
mod tests {
    use super::StrFormat;

    #[test]
    fn cases() {
        for input in [
            "ProductCategory",
            "productCategory",
            "product_category",
            "product-category",
            "product category",
        ] {
            assert_eq!(input.to_snake_case(), "product_category", "{input}");
            assert_eq!(input.to_kebab_case(), "product-category", "{input}");
            assert_eq!(input.to_camel_case(), "productCategory", "{input}");
            assert_eq!(input.to_pascal_case(), "ProductCategory", "{input}");
            assert_eq!(input.to_var(), "pc", "{input}");
        }
        // Mayúsculas seguidas no se separan (igual que en Go).
        assert_eq!("HTTPServer".to_snake_case(), "httpserver");
        assert_eq!("v2Api".to_snake_case(), "v2_api");
        assert_eq!("  __user--Profile  ".to_snake_case(), "user_profile");
        assert_eq!("añoNuevo".to_pascal_case(), "AñoNuevo");
        assert_eq!("".to_snake_case(), "");
        assert_eq!(String::from("UserProfile").to_var(), "up");
    }

    #[test]
    fn pluralize() {
        let cases = [
            ("person", "people"),
            ("sheep", "sheep"),
            ("news", "news"),
            ("city", "cities"),
            ("day", "days"),
            ("y", "ies"),
            ("bus", "buses"),
            ("box", "boxes"),
            ("church", "churches"),
            ("dish", "dishes"),
            ("leaf", "leaves"),
            ("knife", "knives"),
            ("user", "users"),
            ("", ""),
        ];
        for (input, expected) in cases {
            assert_eq!(input.pluralize(), expected, "{input}");
        }
    }

    #[test]
    fn pluralize_identifier() {
        assert_eq!("product_category".pluralize_identifier(), "product_categories");
        assert_eq!("product-category".pluralize_identifier(), "product-categories");
        assert_eq!("productCategory".pluralize_identifier(), "productCategories");
        assert_eq!("ProductCategory".pluralize_identifier(), "ProductCategories");
        assert_eq!("Person".pluralize_identifier(), "People");
        assert_eq!("user".pluralize_identifier(), "users");
        assert_eq!("--".pluralize_identifier(), "--");
    }

    #[test]
    fn title_case() {
        assert_eq!("el señor de los anillos".to_title_case_es(), "El Señor de los Anillos");
        assert_eq!("LA CASA DE PAPEL".to_title_case_es(), "La Casa de Papel");
        assert_eq!("¿dónde está el baño?".to_title_case_es(), "¿Dónde Está el Baño?");
        assert_eq!(
            "uñas y pestañas en  medellín".to_title_case_es(),
            "Uñas y Pestañas en  Medellín"
        );
        assert_eq!("the lord of the rings".to_title_case_en(), "The Lord of the Rings");
        assert_eq!("a tale of two cities".to_title_case_en(), "A Tale of Two Cities");
        assert_eq!("".to_title_case_en(), "");
        // Guiones y apóstrofos
        assert_eq!("maría-josé de la cruz".to_title_case_es(), "María-José de la Cruz");
        assert_eq!("o'brien".to_title_case_en(), "O'Brien");
        assert_eq!("D’ANGELO".to_title_case_es(), "D’Angelo");
        assert_eq!("don't stop".to_title_case_en(), "Don't Stop");
        assert_eq!("it's a sin".to_title_case_en(), "It's a Sin");
    }
}
