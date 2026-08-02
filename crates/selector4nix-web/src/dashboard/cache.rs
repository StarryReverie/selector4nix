use std::sync::Arc;

use axum::extract::State;
use axum::response::Html;
use http::HeaderMap;
use selector4nix_core::AppContext;

use crate::dashboard::VIEW_ENVIRONMENT;

pub async fn get_cache_page(
    State(ctx): State<Arc<AppContext>>,
    headers: HeaderMap,
) -> Html<String> {
    let model = ctx.get_dashboard_cache_stats_usecase.run().await;

    let environment = VIEW_ENVIRONMENT.acquire_env().unwrap();
    let view = environment.get_template("cache.html.jinja").unwrap();

    let model = minijinja::context! { model };
    let rendered = if headers.contains_key("hx-request") && !headers.contains_key("hx-boosted") {
        view.render_captured(model)
            .and_then(|mut v| v.with_state_mut(|state| state.render_block("content")))
            .unwrap()
    } else {
        view.render(model).unwrap()
    };

    Html(rendered)
}
