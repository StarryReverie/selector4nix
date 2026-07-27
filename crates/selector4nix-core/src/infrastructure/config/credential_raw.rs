use std::fmt::{Debug, Formatter, Result as FmtResult};

use anyhow::Result as AnyhowResult;
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct AppRawCredential {
    pub credentials: Vec<AppRawCredentialEntry>,
}

impl AppRawCredential {
    pub fn deserialize(content: &str) -> AnyhowResult<Self> {
        toml::from_str(content)
            .map_err(|_| anyhow::anyhow!("could not deserialize credential content"))
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Deserialize)]
pub struct AppRawCredentialEntry {
    pub url: String,
    pub login: String,
    pub secret: Option<String>,
}

impl Debug for AppRawCredentialEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("AppRawCredentialEntry")
            .field("url", &self.url)
            .field("login", &self.login)
            .field(
                "secret",
                if self.secret.is_some() {
                    &"Some(_)"
                } else {
                    &"None"
                },
            )
            .finish()
    }
}
