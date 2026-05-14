use http_body_util::Full;
use hyper::Response;
use hyper::body::Bytes;
use ironforge::server::{Context, RouterBuilder};
use ironforge::{Error, router_build};

async fn index(_context: &mut Context) -> Result<Response<Full<Bytes>>, Error> {
    Ok(Response::new(Full::new(Bytes::from("ok"))))
}

async fn auth(_context: &mut Context) -> Result<(), Error> {
    Ok(())
}

async fn admin(_context: &mut Context) -> Result<(), Error> {
    Ok(())
}

async fn rate_limit(_context: &mut Context) -> Result<(), Error> {
    Ok(())
}

#[test]
fn router_build_adds_paths_to_router_builder() {
    let mut builder = RouterBuilder::new("web");

    router_build! {
        (mut builder) {
            get("/", index, "home");

            group("api") {
                middleware(auth) {
                    post("users", index, "users.store");
                    patch("users/{id}", index);
                }
            }
            group("dashboard") {
                middleware(auth) {
                    group("admin") {
                        middleware(auth) {
                            get("/", index);
                            post("/testa", index);
                            put("/testa/{fake}", index)[auth, admin, rate_limit]
                        }
                    }
                    group("users") {
                        get("/", index);
                        post("/testu", index);
                        put("/testu/{fake}", index);
                    }
                }
            }
            delete("/trash/resore", index, "recuperar");
        }
    };

    for path in builder.paths.iter() {
        println!(
            "{}: {} >> {} || {} || {}",
            path.method,
            path.path,
            path.name,
            path.middlewares.len(),
            path.is_dinamic
        );
    }

    assert!(builder.paths.iter().all(|path| !path.path.contains("//")));
    println!("{} >> {}", builder.prefixes.join("/"), builder.middlewares.len());
}

#[test]
fn router_build_adds_middlewares_to_single_route() {
    let mut builder = RouterBuilder::new("web");

    router_build! {
        (mut builder) {
            post("/users", index) [auth, admin, rate_limit];
            get("/users", index);
        }
    };

    let post_users = builder
        .paths
        .iter()
        .find(|path| path.method == "POST" && path.path == "users")
        .expect("POST /users route should exist");
    let get_users = builder
        .paths
        .iter()
        .find(|path| path.method == "GET" && path.path == "users")
        .expect("GET /users route should exist");

    assert_eq!(post_users.middlewares.len(), 3);
    assert_eq!(get_users.middlewares.len(), 0);
}
