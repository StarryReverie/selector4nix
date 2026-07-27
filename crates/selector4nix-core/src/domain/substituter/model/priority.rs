use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};
use snafu::{OptionExt, Snafu};

use crate::{AppError, AppErrorKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Priority(NonZeroU32);

impl Priority {
    pub fn new(value: u32) -> Result<Self, TryNewPriorityError> {
        Ok(Self(NonZeroU32::new(value).context(ZeroSnafu)?))
    }

    pub fn value(&self) -> u32 {
        self.0.into()
    }

    pub fn grace(&self, tolerance: i64) -> i64 {
        -(tolerance * self.value() as i64)
    }
}

#[derive(Snafu, Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TryNewPriorityError {
    #[snafu(display("priority should be positive"))]
    Zero,
}

impl From<TryNewPriorityError> for AppError {
    fn from(error: TryNewPriorityError) -> Self {
        Self::new(AppErrorKind::Rule, error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_succeeds() {
        assert_eq!(Priority::new(1).unwrap().value(), 1);
        assert_eq!(Priority::new(40).unwrap().value(), 40);
    }

    #[test]
    fn new_fails_given_zero_value() {
        assert!(matches!(Priority::new(0), Err(TryNewPriorityError::Zero)));
    }
}
