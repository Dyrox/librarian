//! Asks GitHub for the newest release and compares it with this build.
//!
//! Only the public "latest release" endpoint is read; nothing about the user
//! or their library is sent.

use std::time::Duration;

const API: &str = "https://api.github.com/repos/Dyrox/librarian/releases/latest";
pub const RELEASES: &str = "https://github.com/Dyrox/librarian/releases/latest";

#[derive(Debug, Clone, PartialEq)]
pub enum State {
    Idle,
    Checking,
    UpToDate,
    Available(String),
    Failed,
}

/// The newest release's version (`0.1.12`), or why it couldn't be fetched.
pub fn latest() -> Result<String, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .build()
        .into();
    let mut res = agent
        .get(API)
        .header("User-Agent", "Librarian")
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?;
    let body = res.body_mut().read_to_string().map_err(|e| e.to_string())?;
    tag_name(&body).map(|t| t.trim_start_matches('v').to_string()).ok_or_else(|| "no tag_name".into())
}

/// Pulls `"tag_name": "v0.1.12"` out of the JSON without a JSON parser.
fn tag_name(json: &str) -> Option<&str> {
    let rest = &json[json.find("\"tag_name\"")? + "\"tag_name\"".len()..];
    let rest = rest.trim_start().strip_prefix(':')?.trim_start().strip_prefix('"')?;
    Some(&rest[..rest.find('"')?])
}

fn parts(v: &str) -> Vec<u64> {
    v.split('.').map(|p| p.trim().parse().unwrap_or(0)).collect()
}

/// Whether `remote` is a later version than `local`.
pub fn is_newer(remote: &str, local: &str) -> bool {
    parts(remote) > parts(local)
}

pub fn current() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer("0.1.12", "0.1.9"));
        assert!(is_newer("0.2.0", "0.1.40"));
        assert!(!is_newer("0.1.9", "0.1.9"));
        assert!(!is_newer("0.1.8", "0.1.9"));
    }

    #[test]
    fn reads_the_tag() {
        let json = r#"{"url":"x","tag_name": "v0.1.12","name":"Librarian 0.1.12"}"#;
        assert_eq!(tag_name(json), Some("v0.1.12"));
    }
}

#[cfg(test)]
mod live {
    /// Hits the real API: `cargo test -- --ignored fetches_latest`.
    #[test]
    #[ignore]
    fn fetches_latest() {
        let v = super::latest().unwrap();
        println!("latest = {v}");
        assert!(v.split('.').count() == 3);
    }
}
