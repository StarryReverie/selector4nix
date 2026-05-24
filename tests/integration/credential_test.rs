use selector4nix::infrastructure::config::AppCredential;

#[test]
fn example_credential_file_is_valid() {
    let content = include_str!("../../docs/credentials.example.toml");
    AppCredential::deserialize(content).unwrap();
}

#[test]
fn credential_with_all_fields() {
    let cred = AppCredential::deserialize(
        r#"
[[credentials]]
url = "https://cache.example.org/"
login = "user"
secret = "pass"
"#,
    )
    .unwrap();

    assert_eq!(cred.credentials.len(), 1);
    assert_eq!(
        cred.credentials[0].url.value(),
        "https://cache.example.org/"
    );
    assert_eq!(cred.credentials[0].login, "user");
    assert_eq!(cred.credentials[0].secret.as_deref(), Some("pass"));
}

#[test]
fn credential_without_secret() {
    let cred = AppCredential::deserialize(
        r#"
[[credentials]]
url = "https://cache.example.org/"
login = "user"
"#,
    )
    .unwrap();

    assert_eq!(cred.credentials[0].secret, None);
}

#[test]
fn empty_credentials_file() {
    let cred = AppCredential::empty();
    assert!(cred.credentials.is_empty());
}

#[test]
fn invalid_url_is_rejected() {
    let result = AppCredential::deserialize(
        r#"
[[credentials]]
url = "not-a-url"
login = "user"
"#,
    );

    assert!(result.is_err());
}

#[test]
fn unsupported_scheme_is_rejected() {
    let result = AppCredential::deserialize(
        r#"
[[credentials]]
url = "ftp://cache.example.org/"
login = "user"
"#,
    );

    assert!(result.is_err());
}
