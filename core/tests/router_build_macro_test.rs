use http_body_util::Full;
use hyper::Response;
use hyper::body::Bytes;
use ironforge::server::{Request, RouterBuilder};
use ironforge::{HttpError, router_build};

// ─── Controllers ─────────────────────────────────────────────────────────────

async fn index(_Request: &mut Request) -> Result<Response<Full<Bytes>>, HttpError> {
    Ok(Response::new(Full::new(Bytes::from("ok"))))
}

// ─── Middlewares ──────────────────────────────────────────────────────────────

async fn auth(_Request: &mut Request) -> Result<(), HttpError> {
    Ok(())
}

async fn admin(_Request: &mut Request) -> Result<(), HttpError> {
    Ok(())
}

async fn rate_limit(_Request: &mut Request) -> Result<(), HttpError> {
    Ok(())
}

async fn throttle(_Request: &mut Request) -> Result<(), HttpError> {
    Ok(())
}

async fn log(_Request: &mut Request) -> Result<(), HttpError> {
    Ok(())
}

async fn cors(_Request: &mut Request) -> Result<(), HttpError> {
    Ok(())
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn no_double_slash(builder: &RouterBuilder) {
    for path in builder.paths.iter() {
        assert!(!path.path.contains("//"), "ruta con doble slash: {}", path.path);
    }
}

fn no_leading_slash(builder: &RouterBuilder) {
    for path in builder.paths.iter() {
        assert!(!path.path.starts_with('/'), "ruta inicia con slash: {}", path.path);
    }
}

fn no_trailing_slash(builder: &RouterBuilder) {
    for path in builder.paths.iter() {
        assert!(!path.path.ends_with('/'), "ruta termina con slash: {}", path.path);
    }
}

fn find_route<'a>(builder: &'a RouterBuilder, method: &str, path: &str) -> &'a ironforge::server::Path {
    builder
        .paths
        .iter()
        .find(|p| p.method == method && p.path == path)
        .unwrap_or_else(|| panic!("{} {} no encontrada", method, path))
}

// ─── Test principal — app completa ───────────────────────────────────────────

#[test]
fn create_router_build() {
    let mut builder = RouterBuilder::new("web");

    router_build! {
        (mut builder) {
            // rutas simples en la raiz
            get("/", index, "home");
            delete("/trash/restore", index, "trash.restore");

            // group simple sin middlewares
            group("public") {
                get("/about", index, "about");
                get("/contact", index, "contact");
            }

            // group con un middleware
            group("api")[cors] {
                get("/status", index, "api.status");

                // group anidado con multiples paths y middlewares
                group("api", "v1")[auth, rate_limit] {
                    post("/users", index, "api.v1.users.store");
                    get("/users", index, "api.v1.users.index");
                    patch("/users/{id}", index, "api.v1.users.update");
                    delete("/users/{id}", index, "api.v1.users.destroy");
                }

                // group anidado con multiples paths sin middlewares
                group("api", "v2") {
                    get("/users", index, "api.v2.users.index");
                }
            }

            // group con multiples paths y middlewares
            group("dashboard", "admin")[auth, admin] {
                get("/", index, "dashboard.admin.home");
                get("/users", index, "dashboard.admin.users");

                // middleware adicional dentro del group
                middleware(rate_limit, throttle) {
                    post("/users", index, "dashboard.admin.users.store");
                    delete("/users/{id}", index, "dashboard.admin.users.destroy");
                }

                // ruta con middlewares propios encima de los del group
                put("/settings", index, "dashboard.admin.settings")[log];
            }

            // group con middleware que contiene grupos anidados
            group("dashboard")[auth] {
                group("users") {
                    get("/", index, "dashboard.users.home");
                    post("/", index, "dashboard.users.store");
                    put("/{id}", index, "dashboard.users.update")[log];
                }

                middleware(admin) {
                    get("/reports", index, "dashboard.reports");
                    post("/reports", index, "dashboard.reports.store")[rate_limit, log];
                }
            }

            // ruta dinamica en raiz con middlewares
            get("/profile/{id}", index, "profile")[auth, log];
        }
    };

    // imprimir todas las rutas
    for path in builder.paths.iter() {
        println!(
            "{}: {} >> {} || middlewares: {} || dinamica: {}",
            path.method,
            path.path,
            path.name,
            path.middlewares.len(),
            path.is_dinamic
        );
    }

    println!(
        "prefixes restantes: {} >> middlewares restantes: {}",
        builder.prefixes.join("/"),
        builder.middlewares.len()
    );

    // // el stack de prefixes y middlewares debe estar vacio al terminar
    // assert!(builder.prefixes.is_empty(), "prefixes no vaciados correctamente");
    // assert_eq!(builder.middlewares.len(), 0, "middlewares no vaciados correctamente");

    // // validaciones de formato
    // no_double_slash(&builder);
    // no_leading_slash(&builder);
    // no_trailing_slash(&builder);

    // // numero total de rutas
    // assert_eq!(builder.paths.len(), 21, "numero incorrecto de rutas");

    // // rutas dinamicas
    // let dynamic_routes: Vec<_> = builder.paths.iter().filter(|p| p.is_dinamic).collect();
    // assert_eq!(dynamic_routes.len(), 5, "numero incorrecto de rutas dinamicas");

    // // rutas estaticas
    // let static_routes: Vec<_> = builder.paths.iter().filter(|p| !p.is_dinamic).collect();
    // assert_eq!(static_routes.len(), 16, "numero incorrecto de rutas estaticas");

    // // verificar middlewares de rutas especificas
    // let home = find_route(&builder, "GET", "");
    // assert_eq!(home.middlewares.len(), 0, "home no debe tener middlewares");
    // assert_eq!(home.name, "home");

    // let api_v1_users_store = find_route(&builder, "POST", "api/v1/users");
    // assert_eq!(
    //     api_v1_users_store.middlewares.len(),
    //     2,
    //     "api v1 users store debe tener cors + auth + rate_limit"
    // );

    // let admin_users_store = find_route(&builder, "POST", "dashboard/admin/users");
    // assert_eq!(
    //     admin_users_store.middlewares.len(),
    //     4,
    //     "admin users store debe tener auth + admin + rate_limit + throttle"
    // );

    // let admin_settings = find_route(&builder, "PUT", "dashboard/admin/settings");
    // assert_eq!(
    //     admin_settings.middlewares.len(),
    //     3,
    //     "admin settings debe tener auth + admin + log"
    // );

    // let profile = find_route(&builder, "GET", "profile/{id}");
    // assert_eq!(profile.middlewares.len(), 2, "profile debe tener auth + log");
    // assert!(profile.is_dinamic);
}

// ─── Tests especificos ────────────────────────────────────────────────────────

#[test]
fn route_without_middlewares_has_zero() {
    let mut builder = RouterBuilder::new("web");
    router_build! {
        (mut builder) {
            get("/users", index);
        }
    };
    let route = find_route(&builder, "GET", "users");
    assert_eq!(route.middlewares.len(), 0);
}

#[test]
fn route_with_single_middleware() {
    let mut builder = RouterBuilder::new("web");
    router_build! {
        (mut builder) {
            get("/users", index)[auth];
        }
    };
    let route = find_route(&builder, "GET", "users");
    assert_eq!(route.middlewares.len(), 1);
}

#[test]
fn route_with_multiple_middlewares() {
    let mut builder = RouterBuilder::new("web");
    router_build! {
        (mut builder) {
            post("/users", index)[auth, admin, rate_limit];
        }
    };
    let route = find_route(&builder, "POST", "users");
    assert_eq!(route.middlewares.len(), 3);
}

#[test]
fn group_middlewares_apply_to_all_children() {
    let mut builder = RouterBuilder::new("web");
    router_build! {
        (mut builder) {
            group("api")[auth] {
                get("/users", index);
                post("/users", index);
                delete("/users/{id}", index);
            }
        }
    };
    for path in builder.paths.iter() {
        assert_eq!(path.middlewares.len(), 1, "todas las rutas del group deben tener auth");
    }
}

#[test]
fn group_middlewares_stack_with_route_middlewares() {
    let mut builder = RouterBuilder::new("web");
    router_build! {
        (mut builder) {
            group("api")[auth] {
                get("/users", index)[rate_limit, log];
            }
        }
    };
    let route = find_route(&builder, "GET", "api/users");
    assert_eq!(route.middlewares.len(), 3, "debe tener auth + rate_limit + log");
}

#[test]
fn nested_groups_stack_prefixes_correctly() {
    let mut builder = RouterBuilder::new("web");
    router_build! {
        (mut builder) {
            group("api", "v1") {
                get("/users", index);
            }
        }
    };
    let route = find_route(&builder, "GET", "api/v1/users");
    assert!(!route.path.contains("//"));
}

#[test]
fn nested_groups_stack_middlewares_correctly() {
    let mut builder = RouterBuilder::new("web");
    router_build! {
        (mut builder) {
            group("api")[auth] {
                group("admin")[admin] {
                    get("/users", index)[log];
                }
            }
        }
    };
    let route = find_route(&builder, "GET", "api/admin/users");
    assert_eq!(route.middlewares.len(), 3, "debe tener auth + admin + log");
}

#[test]
fn middleware_block_applies_to_all_children() {
    let mut builder = RouterBuilder::new("web");
    router_build! {
        (mut builder) {
            middleware(auth, admin) {
                get("/a", index);
                post("/b", index);
                delete("/c/{id}", index);
            }
        }
    };
    for path in builder.paths.iter() {
        assert_eq!(path.middlewares.len(), 2, "todas las rutas deben tener auth + admin");
    }
}

#[test]
fn stack_is_clean_after_build() {
    let mut builder = RouterBuilder::new("web");
    router_build! {
        (mut builder) {
            group("api", "v1")[auth, rate_limit] {
                middleware(admin, throttle) {
                    get("/users", index)[log];
                }
            }
        }
    };
    assert!(builder.prefixes.is_empty(), "prefixes debe estar vacio");
    assert_eq!(builder.middlewares.len(), 0, "middlewares debe estar vacio");
}

#[test]
fn dynamic_routes_detected_correctly() {
    let mut builder = RouterBuilder::new("web");
    router_build! {
        (mut builder) {
            get("/users", index);
            get("/users/{id}", index);
            get("/users/{id}/posts/{post_id}", index);
        }
    };
    let static_routes: Vec<_> = builder.paths.iter().filter(|p| !p.is_dinamic).collect();
    let dynamic_routes: Vec<_> = builder.paths.iter().filter(|p| p.is_dinamic).collect();
    assert_eq!(static_routes.len(), 1);
    assert_eq!(dynamic_routes.len(), 2);
}

#[test]
fn no_double_slash_in_any_route() {
    let mut builder = RouterBuilder::new("web");
    router_build! {
        (mut builder) {
            get("/", index);
            group("/api/") {
                get("/", index);
                group("/v1/") {
                    get("/users/", index);
                }
            }
        }
    };
    no_double_slash(&builder);
    no_leading_slash(&builder);
    no_trailing_slash(&builder);
    find_route(&builder, "GET", "");
    find_route(&builder, "GET", "api");
    find_route(&builder, "GET", "api/v1/users");

    for path in builder.paths.iter() {
        println!(
            "{}: {} >> {} || middlewares: {} || dinamica: {}",
            path.method,
            path.path,
            path.name,
            path.middlewares.len(),
            path.is_dinamic
        );
    }
}

#[test]
fn named_routes_are_set_correctly() {
    let mut builder = RouterBuilder::new("web");
    router_build! {
        (mut builder) {
            get("/", index, "home");
            group("api") {
                post("/users", index, "users.store");
            }
        }
    };
    let home = find_route(&builder, "GET", "");
    assert_eq!(home.name, "home");

    let users_store = find_route(&builder, "POST", "api/users");
    assert_eq!(users_store.name, "users.store");
}
