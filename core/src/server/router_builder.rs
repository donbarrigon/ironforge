use crate::server::router::{Controller, Middleware, Router};

pub struct Path {
    pub method: String,
    pub path: String,
    pub name: String,
    pub params: Vec<String>,
    pub controller_name: String,
    pub controller: Controller,
    pub middlewares: Vec<Middleware>,
}

impl Path {
    pub fn new(
        method: impl Into<String>,
        path: impl Into<String>,
        name: impl Into<String>,
        params: Vec<String>,
        controller_name: impl Into<String>,
        controller: Controller,
        middlewares: Vec<Middleware>,
    ) -> Self {
        return Self {
            method: method.into(),
            path: path.into(),
            name: name.into(),
            params: params,
            controller_name: controller_name.into(),
            controller,
            middlewares,
        };
    }

    pub fn name(&mut self, name: impl Into<String>) -> &mut Self {
        self.name = name.into().to_lowercase();
        return self;
    }

    pub fn use_middleware(&mut self, middleware: Middleware) -> &mut Self {
        self.middlewares.push(middleware);
        return self;
    }
}

pub struct RouterBuilder {
    pub paths: Vec<Path>,
    pub prefixes: Vec<String>,
    pub middlewares: Vec<Middleware>,
}

impl RouterBuilder {
    pub fn new() -> Self {
        return Self {
            paths: Vec::new(),
            prefixes: Vec::new(),
            middlewares: Vec::new(),
        };
    }

    pub fn add_path(&mut self, method: String, path: String, controller_name: String, controller: Controller) {
        let p = format!("{}/{}", self.prefixes.join("/"), path)
            .trim()
            .trim_matches('/')
            .to_lowercase();

        let parts: Vec<&str> = p.split('/').collect();
        let mut params: Vec<String> = Vec::new();
        let mut name_parts: Vec<String> = Vec::new();

        for part in parts {
            if part.starts_with('{') && part.ends_with('}') {
                params.push(part[1..part.len() - 1].to_string());
            } else if part.starts_with(':') {
                params.push(part[1..part.len()].to_string());
            } else {
                name_parts.push(part.to_string());
            }
        }

        let name = format!("{}.{}", name_parts.join("."), controller_name);
        self.paths.push(Path::new(
            method,
            p,
            name,
            params,
            controller_name,
            controller,
            self.middlewares.clone(),
        ));
    }

    pub fn build(&self) -> Router {
        // TODO: proceso de construir el router
        return Router::new();
    }
}
