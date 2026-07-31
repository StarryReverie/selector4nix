use axum::body::Body;
use http::Response;

pub async fn get_health() -> Response<Body> {
    Response::builder().body(Body::from("OK")).unwrap()
}
