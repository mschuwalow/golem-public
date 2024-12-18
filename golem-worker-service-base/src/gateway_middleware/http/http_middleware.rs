use crate::gateway_middleware::http::cors::Cors;
use http::header::{
    ACCESS_CONTROL_ALLOW_CREDENTIALS, ACCESS_CONTROL_ALLOW_ORIGIN, ACCESS_CONTROL_EXPOSE_HEADERS,
};

#[derive(Debug, Clone, PartialEq)]
pub enum HttpMiddleware {
    Cors(Cors),
}

impl HttpMiddleware {
    pub fn cors(cors: Cors) -> Self {
        HttpMiddleware::Cors(cors)
    }

    pub fn transform_response(&self, response: &mut poem::Response) {
        match self {
            // if CORS is applied as a middleware, we need to return a response with specific CORS headers
            HttpMiddleware::Cors(cors) => {
                Self::apply_cors(response, cors);
            }
        }
    }

    fn apply_cors(response: &mut poem::Response, cors: &Cors) {
        response.headers_mut().insert(
            ACCESS_CONTROL_ALLOW_ORIGIN,
            // hot path, and this unwrap will not fail unless we bypassed it during configuration
            cors.get_allow_origin().clone().parse().unwrap(),
        );

        if let Some(allow_credentials) = &cors.get_allow_credentials() {
            response.headers_mut().insert(
                ACCESS_CONTROL_ALLOW_CREDENTIALS,
                // hot path, and this unwrap will not fail unless we bypassed it during configuration
                allow_credentials.to_string().clone().parse().unwrap(),
            );
        }

        if let Some(expose_headers) = &cors.get_expose_headers() {
            response.headers_mut().insert(
                ACCESS_CONTROL_EXPOSE_HEADERS,
                // hot path, and this unwrap will not fail unless we bypassed it during configuration
                expose_headers.clone().parse().unwrap(),
            );
        }
    }
}
