use axum::extract::State;
use axum::response::Json;

use crate::api::state::AppState;
use crate::domain::substituter::index::SubstituterAvailabilityIndex;

#[derive(serde::Serialize)]
pub struct AvailableSubstitutersResponse {
    pub substituters: Vec<crate::domain::substituter::model::SubstituterMeta>,
}

pub async fn get_available(State(state): State<AppState>) -> Json<AvailableSubstitutersResponse> {
    let substituters = state.index_view.query_all();
    Json(AvailableSubstitutersResponse {
        substituters: (*substituters).clone(),
    })
}
