mod runner;

pub use runner::{NarInfoActor, NarInfoRequest, ResolveNarInfoResponse};

use selector4nix_actor::registry::Registry;

use crate::domain::nar_info::model::StorePathHash;

pub type NarInfoActorRegistry = Registry<StorePathHash, NarInfoActor>;
