use std::sync::LazyLock;

use http::{HeaderMap, HeaderValue, header};

pub static SELF_USER_AGENT: LazyLock<String> =
    LazyLock::new(|| format!("{}/{}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"),));

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PassthroughHeaders {
    user_agent: Option<HeaderValue>,
}

impl PassthroughHeaders {
    pub fn empty() -> Self {
        Self { user_agent: None }
    }

    pub fn extract(mut headers: HeaderMap) -> Self {
        Self {
            user_agent: headers.remove(header::USER_AGENT),
        }
    }

    pub fn proxyed(mut self) -> Self {
        if let Some(value) = &mut self.user_agent
            && !value.is_empty()
        {
            let mut bytes = value.as_bytes().to_owned();
            bytes.extend(b" ");
            bytes.extend(SELF_USER_AGENT.as_bytes());
            *value =
                HeaderValue::from_bytes(&bytes).expect("`bytes` should be a valid `HeaderValue`");
        }

        self
    }

    pub fn to_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();

        if let Some(value) = &self.user_agent {
            headers.append(header::USER_AGENT, value.clone());
        }

        headers
    }
}
