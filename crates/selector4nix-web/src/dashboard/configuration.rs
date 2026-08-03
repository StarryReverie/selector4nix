use std::sync::Arc;

use axum::extract::State;
use axum::response::Html;
use selector4nix_core::AppContext;

use crate::dashboard::VIEW_ENVIRONMENT;

pub async fn get_configuration_page(State(ctx): State<Arc<AppContext>>) -> Html<String> {
    let model = ctx.get_dashboard_config_summary_usecase.run();

    let environment = VIEW_ENVIRONMENT.acquire_env().unwrap();
    let view = environment
        .get_template("configuration.html.jinja")
        .unwrap();

    let model = minijinja::context! { model };
    let rendered = view.render(model).unwrap();
    Html(rendered)
}
