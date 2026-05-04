mod runner;

pub use runner::{SubstituterActor, SubstituterRequest};

use selector4nix_actor::registry::Registry;

use crate::domain::substituter::model::Url;

pub type SubstituterActorRegistry = Registry<Url, SubstituterActor>;
