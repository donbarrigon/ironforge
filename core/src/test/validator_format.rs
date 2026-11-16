use crate::validator::format::Format;

#[test]
fn re_phone_code_e164() {
    assert!("+573001234567".re_phone_code()); // Colombia, 12 dígitos
    assert!("+12025550123".re_phone_code()); // EE. UU., código de 1 dígito
    assert!("+593991234567".re_phone_code()); // Ecuador, código de 3 dígitos
    assert!("+123456789012345".re_phone_code()); // 15 dígitos, el máximo

    assert!(!"+1234567890123456".re_phone_code()); // 16 dígitos
    assert!(!"+1234567".re_phone_code()); // 7 dígitos
    assert!(!"+0573001234567".re_phone_code()); // código empieza en 0
    assert!(!"573001234567".re_phone_code()); // sin `+`
    assert!(!"+57 300 123 4567".re_phone_code()); // con espacios
}
