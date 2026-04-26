use crate::infrastructure::index::substituter_availability::SubstituterAvailabilityIndexView;

#[derive(Clone)]
pub struct AppState {
    pub index_view: SubstituterAvailabilityIndexView,
}
