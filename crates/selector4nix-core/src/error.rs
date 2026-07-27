use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

use anyhow::Error as AnyhowError;
use getset::CopyGetters;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppErrorKind {
    Input,
    NotFound,
    Rule,
    Infrastructure,
    Catastrophic,
}

#[derive(Debug, CopyGetters)]
pub struct AppError {
    #[getset(get_copy = "pub")]
    kind: AppErrorKind,
    error: AnyhowError,
}

impl AppError {
    #[inline]
    pub fn new(kind: AppErrorKind, error: impl Into<AnyhowError>) -> Self {
        Self {
            kind,
            error: error.into(),
        }
    }

    #[inline]
    pub fn message(kind: AppErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            error: AnyhowError::msg(message.into()),
        }
    }

    #[inline]
    pub fn input(message: impl Into<String>) -> Self {
        Self::message(AppErrorKind::Input, message)
    }

    #[inline]
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::message(AppErrorKind::NotFound, message)
    }

    #[inline]
    pub fn rule(message: impl Into<String>) -> Self {
        Self::message(AppErrorKind::Rule, message)
    }

    #[inline]
    pub fn infrastructure(message: impl Into<String>) -> Self {
        Self::message(AppErrorKind::Infrastructure, message)
    }

    #[inline]
    pub fn catastrophic(message: impl Into<String>) -> Self {
        Self::message(AppErrorKind::Catastrophic, message)
    }

    #[inline]
    pub fn chain_infrastructure(
        source: impl Into<AnyhowError>,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            AppErrorKind::Infrastructure,
            source.into().context(message.into()),
        )
    }

    #[inline]
    pub fn chain_catastrophic(source: impl Into<AnyhowError>, message: impl Into<String>) -> Self {
        Self::new(
            AppErrorKind::Catastrophic,
            source.into().context(message.into()),
        )
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.error.fmt(f)
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.error.source()
    }
}

pub trait AppResultExt {
    type Output;

    fn throw_not_found(self, message: impl Into<String>) -> <Self as HasOption>::Unwrapped
    where
        Self: HasOption;

    fn throw_infrastructure(self, message: impl Into<String>) -> Self::Output;

    fn throw_catastrophic(self, message: impl Into<String>) -> Self::Output;

    fn chain_infrastructure(self, message: impl Into<String>) -> Self::Output
    where
        Self: HasIntoAnyhowError;

    fn chain_catastrophic(self, message: impl Into<String>) -> Self::Output
    where
        Self: HasIntoAnyhowError;
}

impl<T, E> AppResultExt for Result<T, E> {
    type Output = Result<T, AppError>;

    #[inline]
    fn throw_not_found(self, message: impl Into<String>) -> <Self as HasOption>::Unwrapped
    where
        Self: HasOption,
    {
        self.unwrap_none_to_err(|| AppError::not_found(message))
    }

    #[inline]
    fn throw_infrastructure(self, message: impl Into<String>) -> Self::Output {
        self.map_err(|_| AppError::infrastructure(message))
    }

    #[inline]
    fn throw_catastrophic(self, message: impl Into<String>) -> Self::Output {
        self.map_err(|_| AppError::catastrophic(message))
    }

    #[inline]
    fn chain_infrastructure(self, message: impl Into<String>) -> Self::Output
    where
        Self: HasIntoAnyhowError,
    {
        self.convert_err(|source| AppError::chain_infrastructure(source, message))
    }

    #[inline]
    fn chain_catastrophic(self, message: impl Into<String>) -> Self::Output
    where
        Self: HasIntoAnyhowError,
    {
        self.convert_err(|source| AppError::chain_catastrophic(source, message))
    }
}

pub trait HasOption: AppResultExt {
    type Unwrapped;

    fn unwrap_none_to_err<F>(self, f: F) -> Self::Unwrapped
    where
        F: FnOnce() -> AppError;
}

impl<T> HasOption for Result<Option<T>, AppError> {
    type Unwrapped = Result<T, AppError>;

    #[inline]
    fn unwrap_none_to_err<F>(self, f: F) -> Self::Unwrapped
    where
        F: FnOnce() -> AppError,
    {
        self.and_then(|opt| opt.map_or_else(|| Err(f()), &Ok))
    }
}

pub trait HasIntoAnyhowError: AppResultExt {
    fn convert_err<F>(self, f: F) -> Self::Output
    where
        F: FnOnce(AnyhowError) -> AppError;
}

impl<T, E> HasIntoAnyhowError for Result<T, E>
where
    E: Into<AnyhowError>,
{
    #[inline]
    fn convert_err<F>(self, f: F) -> Self::Output
    where
        F: FnOnce(AnyhowError) -> AppError,
    {
        self.map_err(|source| f(source.into()))
    }
}
