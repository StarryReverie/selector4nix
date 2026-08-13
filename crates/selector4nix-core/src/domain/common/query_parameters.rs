use crate::domain::common::url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QueryParameters(String);

impl QueryParameters {
    pub fn try_extract(url: &Url) -> Option<Self> {
        url.inner().query().map(|qp| Self(qp.into()))
    }

    pub fn try_from_raw(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if !raw.is_empty() {
            // Use `url::Url::set_query()` to encode the raw string to safe URL string.
            Self::try_extract(&Url::new(&format!("https://example.com/?{raw}")).ok()?)
        } else {
            None
        }
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}
