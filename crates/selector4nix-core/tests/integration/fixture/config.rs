pub fn make_config_string_minimal() -> String {
    r#"
[server]
ip = "127.0.0.1"

[[substituters]]
url = "https://cache.nixos.org/"
"#
    .to_string()
}

pub fn make_config_string_overriden(extra: &str) -> String {
    format!(
        r#"
[server]
ip = "127.0.0.1"

[[substituters]]
url = "https://cache.nixos.org/"

{extra}
"#
    )
}
