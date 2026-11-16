use crate::utils::str::formatter::StrFormat;

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
