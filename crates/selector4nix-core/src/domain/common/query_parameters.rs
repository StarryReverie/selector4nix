use std::string::FromUtf8Error;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::{DecodeError, Engine};
use snafu::{ResultExt, Snafu};

use crate::domain::common::url::Url;
use crate::{AppError, AppErrorKind};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QueryParameters(String);

impl QueryParameters {
    pub fn try_extract(url: &Url) -> Option<Self> {
        url.inner()
            .query()
            .filter(|qp| !qp.is_empty())
            .map(|qp| Self(qp.into()))
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

    pub fn encode(&self) -> String {
        URL_SAFE_NO_PAD.encode(&self.0)
    }

    pub fn decode(encoded: &str) -> Result<Self, DecodeQueryParametersError> {
        let bytes = URL_SAFE_NO_PAD
            .decode(encoded)
            .context(InvalidEncodedSnafu)?;
        let qp_str = String::from_utf8(bytes).context(InvalidUtf8Snafu)?;
        let Some(qp) = Self::try_from_raw(&qp_str) else {
            return EmptySnafu.fail();
        };
        Ok(qp)
    }
}

#[derive(Debug, Snafu, Clone, PartialEq, Eq)]
pub enum DecodeQueryParametersError {
    #[snafu(display("the encoded query parameter is not valid URL-safe Base64 string"))]
    InvalidEncoded { source: DecodeError },
    #[snafu(display("the decoded query parameter is not valid UTF-8 string"))]
    InvalidUtf8 { source: FromUtf8Error },
    #[snafu(display("the decoded query parameter should not be empty"))]
    Empty,
}

impl From<DecodeQueryParametersError> for AppError {
    fn from(error: DecodeQueryParametersError) -> Self {
        Self::new(AppErrorKind::Input, error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_extract_returns_none_given_url_without_query() {
        let url = Url::new("https://example.com/nar/abc.nar.xz").unwrap();
        assert!(QueryParameters::try_extract(&url).is_none());

        let url = Url::new("https://example.com/nar/abc.nar.xz?").unwrap();
        assert!(QueryParameters::try_extract(&url).is_none());
    }

    #[test]
    fn try_extract_returns_query_preserving_multiple_params() {
        let url = Url::new(
            "https://storage.example.com/nar/abc.nar.xz?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Signature=f776",
        )
        .unwrap();
        let qp = QueryParameters::try_extract(&url).unwrap();
        assert_eq!(
            qp.value(),
            "X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Signature=f776"
        );
    }

    #[test]
    fn try_from_raw_returns_none_given_empty_or_whitespace() {
        assert!(QueryParameters::try_from_raw("").is_none());
        assert!(QueryParameters::try_from_raw("   ").is_none());
        assert!(QueryParameters::try_from_raw("\t\n").is_none());
    }

    #[test]
    fn try_from_raw_normalizes_value() {
        let qp = QueryParameters::try_from_raw("  a=1&name=café world  ").unwrap();
        assert_eq!(qp.value(), "a=1&name=caf%C3%A9%20world");
    }

    #[test]
    fn encode_decode_round_trips() {
        let simple = QueryParameters::try_from_raw("a=1").unwrap();
        assert_eq!(simple.encode(), "YT0x");
        assert_eq!(QueryParameters::decode("YT0x").unwrap(), simple);

        let qp =
            QueryParameters::try_from_raw("X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Signature=f776")
                .unwrap();
        assert_eq!(QueryParameters::decode(&qp.encode()).unwrap(), qp);
    }

    #[test]
    fn decode_returns_invalid_encoded_error_given_non_base64() {
        assert!(matches!(
            QueryParameters::decode("not@valid"),
            Err(DecodeQueryParametersError::InvalidEncoded { .. }),
        ));
    }

    #[test]
    fn decode_returns_invalid_utf8_error_given_non_utf8_bytes() {
        let encoded = URL_SAFE_NO_PAD.encode([0xFF_u8, 0xFE]);
        assert!(matches!(
            QueryParameters::decode(&encoded),
            Err(DecodeQueryParametersError::InvalidUtf8 { .. }),
        ));
    }

    #[test]
    fn decode_returns_empty_error_given_empty_or_whitespace_payload() {
        assert!(matches!(
            QueryParameters::decode(""),
            Err(DecodeQueryParametersError::Empty),
        ));
        let encoded = URL_SAFE_NO_PAD.encode("   ");
        assert!(matches!(
            QueryParameters::decode(&encoded),
            Err(DecodeQueryParametersError::Empty),
        ));
    }

    #[test]
    fn decode_normalizes_value_via_try_from_raw() {
        let encoded = URL_SAFE_NO_PAD.encode("a=b c");
        let qp = QueryParameters::decode(&encoded).unwrap();
        assert_eq!(qp.value(), "a=b%20c");
    }
}
