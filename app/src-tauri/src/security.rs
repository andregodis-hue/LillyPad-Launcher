//! The few rules that decide what the launcher may open, download or write. Kept small and tested.

use std::path::{Component, Path, PathBuf};
use url::Url;

use crate::config::{OPEN_URLS};

/// Plain https url, no user info, no odd port. Returns the parsed url.
fn strict_https(s: &str) -> Result<Url, String> {
    let u = Url::parse(s).map_err(|_| "Ogiltig adress.".to_string())?;
    if u.scheme() != "https" || !u.username().is_empty() || u.password().is_some() || u.port().is_some() {
        return Err("Adressen är inte tillåten.".into());
    }
    Ok(u)
}

/// For open_url: https and an allowlisted host and path.
pub fn check_open_url(s: &str) -> Result<Url, String> {
    let u = strict_https(s)?;
    let host = u.host_str().unwrap_or("").to_ascii_lowercase();
    let ok = OPEN_URLS.iter().any(|(h, p)| *h == host && u.path().starts_with(p));
    if ok { Ok(u) } else { Err("Adressen är inte tillåten.".into()) }
}

/// For downloads and api calls: https and the host is in the given list.
pub fn check_host(s: &str, hosts: &[&str]) -> Result<Url, String> {
    let u = strict_https(s)?;
    let host = u.host_str().unwrap_or("").to_ascii_lowercase();
    if hosts.contains(&host.as_str()) { Ok(u) } else { Err("Nedladdningen kommer från en värd som inte är tillåten.".into()) }
}

/// Joins a relative path from an archive or index under base. Rejects absolute paths, "..", drive letters
/// and anything that would land outside base.
pub fn safe_join(base: &Path, rel: &str) -> Result<PathBuf, String> {
    let bad = || "Filsökvägen är inte tillåten.".to_string();
    if rel.is_empty() || rel.contains('\\') || rel.contains('\0') || rel.contains(':') { return Err(bad()); }
    let mut out = base.to_path_buf();
    for c in Path::new(rel).components() {
        match c {
            Component::Normal(p) => out.push(p),
            _ => return Err(bad()),
        }
    }
    if !out.starts_with(base) || out == base { return Err(bad()); }
    Ok(out)
}

/// Removes anything that looks like a token from a log line.
pub fn redact(line: &str, secrets: &[&str]) -> String {
    let mut s = line.to_string();
    for sec in secrets {
        if sec.len() >= 8 { s = s.replace(sec, "***"); }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_url_allowlist() {
        assert!(check_open_url("https://lillypadnetwork.se").is_ok());
        assert!(check_open_url("https://lillypadnetwork.se/regler").is_ok());
        assert!(check_open_url("https://www.microsoft.com/link?otc=ABCD").is_ok());
        assert!(check_open_url("https://microsoft.com/link").is_ok());
        assert!(check_open_url("https://discord.gg/abc").is_ok());
        assert!(check_open_url("https://discord.com/invite/abc").is_ok());
        for bad in [
            "http://lillypadnetwork.se", "https://evil.example", "https://lillypadnetwork.se.evil.example",
            "https://user@lillypadnetwork.se", "https://lillypadnetwork.se:8443/", "file:///etc/passwd",
            "javascript:alert(1)", "https://www.microsoft.com/other", "https://discord.com/channels/1",
            "https://lillypadnetwork.se@evil.example/", "", "lillypadnetwork.se",
        ] {
            assert!(check_open_url(bad).is_err(), "should reject {bad}");
        }
    }

    #[test]
    fn host_allowlist() {
        let h = ["cdn.modrinth.com"];
        assert!(check_host("https://cdn.modrinth.com/data/x/y.jar", &h).is_ok());
        assert!(check_host("http://cdn.modrinth.com/x", &h).is_err());
        assert!(check_host("https://cdn.modrinth.com.evil.io/x", &h).is_err());
        assert!(check_host("https://other.modrinth.com/x", &h).is_err());
    }

    #[test]
    fn safe_join_rules() {
        let base = Path::new("/tmp/inst");
        assert!(safe_join(base, "mods/a.jar").is_ok());
        for bad in ["../x", "/etc/passwd", "mods/../../x", "C:/x", "a\\b", "", ".", "mods/./..", "a\0b"] {
            assert!(safe_join(base, bad).is_err(), "should reject {bad:?}");
        }
    }

    #[test]
    fn redacts() {
        assert_eq!(redact("token=abcdefgh12345 done", &["abcdefgh12345"]), "token=*** done");
        assert_eq!(redact("short", &["ab"]), "short");
    }
}
