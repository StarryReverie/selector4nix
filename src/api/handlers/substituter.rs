use std::sync::Arc;

use axum::extract::State;
use axum::response::Json;
use serde::Serialize;

use crate::api::state::AppContext;
use crate::domain::substituter::model::SubstituterMeta;

#[derive(Serialize)]
pub struct AvailableSubstitutersResponse {
    substituters: Vec<SubstituterMeta>,
}

pub async fn get_available_substituters(
    State(ctx): State<Arc<AppContext>>,
) -> Json<AvailableSubstitutersResponse> {
    let substituters = ctx.substituter_usecase().get_available();
    Json(AvailableSubstitutersResponse { substituters })
}
