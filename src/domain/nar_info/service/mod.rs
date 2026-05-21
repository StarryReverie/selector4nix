mod resolution;
mod util;

pub use resolution::{NarInfoResolutionEvent, NarInfoResolutionService, ResolveNarInfoError};

use util::DeadlineGroup;
