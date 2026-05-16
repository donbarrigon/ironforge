use hyper::StatusCode;
use ironforge::error::HttpError;
use ironforge::load_env;
use ironforge_macros::create_error;

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn assert_status(err: &HttpError, status: StatusCode) {
    assert_eq!(err.status, status);
}

fn assert_message(err: &HttpError, message: &str) {
    assert_eq!(err.message, message);
}

fn assert_no_cause(err: &HttpError) {
    assert!(err.cause.is_none(), "se esperaba cause None pero tiene valor");
}

fn assert_has_cause(err: &HttpError) {
    assert!(err.cause.is_some(), "se esperaba cause Some pero es None");
}

fn assert_no_data(err: &HttpError) {
    assert!(err.data.is_none(), "se esperaba data None pero tiene valor");
}

fn assert_has_data(err: &HttpError) {
    assert!(err.data.is_some(), "se esperaba data Some pero es None");
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[test]
fn create_error_macro() {
    // ─── Sin kind — internal_server_error por defecto ────────────────────────
    load_env().unwrap();

    println!(">> default_kind_only_message");
    let err = create_error!("algo salio mal");
    assert_status(&err, StatusCode::INTERNAL_SERVER_ERROR);
    assert_message(&err, "algo salio mal");
    assert_no_cause(&err);
    assert_no_data(&err);

    println!(">> default_kind_with_empty_cause");
    let err = create_error!("algo salio mal", Empty);
    assert_status(&err, StatusCode::INTERNAL_SERVER_ERROR);
    assert_message(&err, "algo salio mal");
    assert_no_cause(&err);
    assert_no_data(&err);

    println!(">> default_kind_with_str_cause");
    let err = create_error!("algo salio mal", "detalle del error");
    assert_status(&err, StatusCode::INTERNAL_SERVER_ERROR);
    assert_message(&err, "algo salio mal");
    assert_no_data(&err);

    println!(">> default_kind_with_real_cause");
    let cause = std::io::Error::new(std::io::ErrorKind::NotFound, "archivo no encontrado");
    let err = create_error!("algo salio mal", cause);
    assert_status(&err, StatusCode::INTERNAL_SERVER_ERROR);
    assert_message(&err, "algo salio mal");
    assert_no_data(&err);

    println!(">> default_kind_with_empty_cause_and_data");
    let err = create_error!("algo salio mal", Empty, { "key": "value" });
    assert_status(&err, StatusCode::INTERNAL_SERVER_ERROR);
    assert_message(&err, "algo salio mal");
    assert_no_cause(&err);
    assert_has_data(&err);

    println!(">> default_kind_with_str_cause_and_data");
    let err = create_error!("algo salio mal", "detalle", { "key": "value" });
    assert_status(&err, StatusCode::INTERNAL_SERVER_ERROR);
    assert_message(&err, "algo salio mal");
    assert_has_data(&err);

    println!(">> default_kind_with_real_cause_and_data");
    let cause = std::io::Error::new(std::io::ErrorKind::Other, "fallo db");
    let err = create_error!("algo salio mal", cause, { "key": "value" });
    assert_status(&err, StatusCode::INTERNAL_SERVER_ERROR);
    assert_message(&err, "algo salio mal");
    assert_has_data(&err);

    // ─── Con kind especifico ──────────────────────────────────────────────────

    println!(">> specific_kind_only_message");
    let err = create_error!(not_found, "no existe");
    assert_status(&err, StatusCode::NOT_FOUND);
    assert_message(&err, "no existe");
    assert_no_cause(&err);
    assert_no_data(&err);

    println!(">> specific_kind_with_empty_cause");
    let err = create_error!(bad_request, "datos invalidos", Empty);
    assert_status(&err, StatusCode::BAD_REQUEST);
    assert_message(&err, "datos invalidos");
    assert_no_cause(&err);
    assert_no_data(&err);

    println!(">> specific_kind_with_str_cause");
    let err = create_error!(unauthorized, "no autorizado", "token invalido");
    assert_status(&err, StatusCode::UNAUTHORIZED);
    assert_message(&err, "no autorizado");
    assert_no_data(&err);

    println!(">> specific_kind_with_real_cause");
    let cause = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "sin permisos");
    let err = create_error!(forbidden, "acceso denegado", cause);
    assert_status(&err, StatusCode::FORBIDDEN);
    assert_message(&err, "acceso denegado");
    assert_no_data(&err);

    println!(">> specific_kind_with_empty_cause_and_data");
    let err = create_error!(conflict, "hay conflicto", Empty, { "field": "email" });
    assert_status(&err, StatusCode::CONFLICT);
    assert_message(&err, "hay conflicto");
    assert_no_cause(&err);
    assert_has_data(&err);

    println!(">> specific_kind_with_str_cause_and_data");
    let err = create_error!(unprocessable_entity, "entidad invalida", "fallo validacion", { "field": "name" });
    assert_status(&err, StatusCode::UNPROCESSABLE_ENTITY);
    assert_message(&err, "entidad invalida");
    assert_has_data(&err);

    println!(">> specific_kind_with_real_cause_and_data");
    let cause = std::io::Error::new(std::io::ErrorKind::Other, "fallo db");
    let err = create_error!(internal_server_error, "error interno", cause, { "query": "SELECT *" });
    assert_status(&err, StatusCode::INTERNAL_SERVER_ERROR);
    assert_message(&err, "error interno");
    assert_has_data(&err);

    // ─── Data ─────────────────────────────────────────────────────────────────

    println!(">> data_contains_correct_values");
    let user_id = 42u32;
    let err = create_error!(not_found, "usuario no existe", Empty, { "id": user_id });
    let data = err.data.unwrap();
    assert_eq!(data["id"], 42);

    println!(">> data_contains_multiple_fields");
    let err = create_error!(bad_request, "datos invalidos", Empty, {
        "field": "email",
        "reason": "formato invalido"
    });
    let data = err.data.unwrap();
    assert_eq!(data["field"], "email");
    assert_eq!(data["reason"], "formato invalido");

    // ─── Debug mode ───────────────────────────────────────────────────────────

    println!(">> debug_mode_has_stack");
    let err = create_error!("algo salio mal");
    if cfg!(debug_assertions) {
        assert!(!err.stack.is_empty(), "se esperaba stack en modo debug");
    }

    println!(">> debug_mode_has_cause_when_provided");
    let cause = std::io::Error::new(std::io::ErrorKind::Other, "fallo db");
    let err = create_error!("algo salio mal", cause);
    assert_has_cause(&err);

    // ─── Todos los 4xx ────────────────────────────────────────────────────────

    println!(">> all_4xx_kinds");
    assert_status(&create_error!(bad_request, "msg"), StatusCode::BAD_REQUEST);
    assert_status(&create_error!(unauthorized, "msg"), StatusCode::UNAUTHORIZED);
    assert_status(&create_error!(payment_required, "msg"), StatusCode::PAYMENT_REQUIRED);
    assert_status(&create_error!(forbidden, "msg"), StatusCode::FORBIDDEN);
    assert_status(&create_error!(not_found, "msg"), StatusCode::NOT_FOUND);
    assert_status(
        &create_error!(method_not_allowed, "msg"),
        StatusCode::METHOD_NOT_ALLOWED,
    );
    assert_status(&create_error!(not_acceptable, "msg"), StatusCode::NOT_ACCEPTABLE);
    assert_status(&create_error!(request_timeout, "msg"), StatusCode::REQUEST_TIMEOUT);
    assert_status(&create_error!(conflict, "msg"), StatusCode::CONFLICT);
    assert_status(&create_error!(gone, "msg"), StatusCode::GONE);
    assert_status(
        &create_error!(unprocessable_entity, "msg"),
        StatusCode::UNPROCESSABLE_ENTITY,
    );
    assert_status(&create_error!(too_many_requests, "msg"), StatusCode::TOO_MANY_REQUESTS);

    // ─── Todos los 5xx ────────────────────────────────────────────────────────

    println!(">> all_5xx_kinds");
    assert_status(
        &create_error!(internal_server_error, "msg"),
        StatusCode::INTERNAL_SERVER_ERROR,
    );
    assert_status(&create_error!(not_implemented, "msg"), StatusCode::NOT_IMPLEMENTED);
    assert_status(&create_error!(bad_gateway, "msg"), StatusCode::BAD_GATEWAY);
    assert_status(
        &create_error!(service_unavailable, "msg"),
        StatusCode::SERVICE_UNAVAILABLE,
    );
    assert_status(&create_error!(gateway_timeout, "msg"), StatusCode::GATEWAY_TIMEOUT);

    println!(">> todos los tests pasaron");
}
