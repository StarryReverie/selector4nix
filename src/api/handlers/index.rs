use axum::response::Html;

const INDEX_HTML: &str = include_str!("index.html");

pub async fn get_index() -> Html<&'static str> {
    INDEX_HTML.into()
}
