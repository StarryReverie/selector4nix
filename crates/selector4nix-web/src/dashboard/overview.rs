use std::sync::Arc;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Html;
use minijinja::context;
use selector4nix_core::AppContext;

use crate::dashboard::VIEW_ENVIRONMENT;

pub async fn get_overview_page(
    State(ctx): State<Arc<AppContext>>,
    headers: HeaderMap,
) -> Html<String> {
    let model = ctx.get_dashboard_overview_usecase.run().await;

    let environment = VIEW_ENVIRONMENT.acquire_env().unwrap();
    let view = environment.get_template("overview.html.jinja").unwrap();

    let model = context! { model };
    let rendered = if headers.contains_key("hx-request") && !headers.contains_key("hx-boosted") {
        view.render_captured(model)
            .and_then(|mut v| v.with_state_mut(|state| state.render_block("content")))
            .unwrap()
    } else {
        view.render(model).unwrap()
    };

    Html(rendered)
}
