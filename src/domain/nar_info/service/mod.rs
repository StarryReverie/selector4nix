mod resolution;
mod util;

pub use resolution::{NarInfoService, ResolveNarInfoError, ResolveNarInfoEvent};

use util::DeadlineGroup;
