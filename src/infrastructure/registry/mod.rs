use selector4nix_actor::registry::Registry;

use crate::application::nar::actor::NarActor;
use crate::application::substituter::actor::SubstituterActor;
use crate::domain::nar::model::StorePathHash;
use crate::domain::substituter::model::Url;

pub type SubstituterActorRegistry = Registry<Url, SubstituterActor>;
pub type NarActorRegistry = Registry<StorePathHash, NarActor>;
