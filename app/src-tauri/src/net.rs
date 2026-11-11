//! All network access goes through here: https only, exact host allowlist (also on redirects), size limits,
//! and every download checked against a hash before it is moved into place.

use futures_util::StreamExt;
use reqwest::{redirect::Policy, Client};
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};
use std::path::Path;
use std::time::Duration;
use tokio::io::AsyncWriteExt;

use crate::security::check_host;

#[derive(Clone, Debug)]
pub enum Hash {
    Sha1(String),
    Sha256(String),
    Sha512(String),
}

enum Hasher {
    S1(Sha1),
    S256(Sha256),
    S512(Sha512),
}

impl Hash {
    fn hasher(&self) -> Hasher {
        match self {
            Hash::Sha1(_) => Hasher::S1(Sha1::new()),
            Hash::Sha256(_) => Hasher::S256(Sha256::new()),
            Hash::Sha512(_) => Hasher::S512(Sha512::new()),
        }
    }
    fn expected(&self) -> &str {
        match self {
            Hash::Sha1(h) | Hash::Sha256(h) | Hash::Sha512(h) => h,
        }
    }
}

impl Hasher {
    fn update(&mut self, b: &[u8]) {
        match self {
            Hasher::S1(h) => h.update(b),
            Hasher::S256(h) => h.update(b),
            Hasher::S512(h) => h.update(b),
        }
    }
    fn finish(self) -> String {
        match self {
            Hasher::S1(h) => hex::encode(h.finalize()),
            Hasher::S256(h) => hex::encode(h.finalize()),
            Hasher::S512(h) => hex::encode(h.finalize()),
        }
    }
}

/// Hash of a byte slice, same algorithm as `h`.
pub fn matches(h: &Hash, data: &[u8]) -> bool {
    let mut hs = h.hasher();
    hs.update(data);
    hs.finish().eq_ignore_ascii_case(h.expected())
}

#[derive(Clone)]
pub struct Net {
    client: Client,
    hosts: Vec<String>,
}

impl Net {
    pub fn new(hosts: &'static [&'static str]) -> Result<Net, String> {
        let list: Vec<String> = hosts.iter().map(|s| s.to_string()).collect();
        let allowed = list.clone();
        let policy = Policy::custom(move |attempt| {
            let refs: Vec<&str> = allowed.iter().map(|s| s.as_str()).collect();
            if attempt.previous().len() >= 5 {
                attempt.error("För många omdirigeringar")
            } else if check_host(attempt.url().as_str(), &refs).is_ok() {
                attempt.follow()
            } else {
                attempt.error("Omdirigering till otillåten värd")
            }
        });
        let client = Client::builder()
            .https_only(true)
            .redirect(policy)
            .user_agent(concat!("LillypadLauncher/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(600))
            .build()
            .map_err(|_| "Kunde inte starta nätverket.".to_string())?;
        Ok(Net { client, hosts: list })
    }

    fn check(&self, url: &str) -> Result<(), String> {
        let refs: Vec<&str> = self.hosts.iter().map(|s| s.as_str()).collect();
        check_host(url, &refs).map(|_| ())
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Small text or json bodies (manifests). Size limited.
    pub async fn get_bytes(&self, url: &str, max: usize) -> Result<Vec<u8>, String> {
        self.check(url)?;
        let resp = self.client.get(url).send().await.map_err(|_| "Nätverksfel.".to_string())?;
        if !resp.status().is_success() {
            return Err(format!("Servern svarade {}.", resp.status().as_u16()));
        }
        let mut out = Vec::new();
        let mut s = resp.bytes_stream();
        while let Some(chunk) = s.next().await {
            let c = chunk.map_err(|_| "Nätverksfel.".to_string())?;
            if out.len() + c.len() > max {
                return Err("Svaret är för stort.".into());
            }
            out.extend_from_slice(&c);
        }
        Ok(out)
    }

    /// Download to `dest`, verify the hash, then move into place. A wrong hash leaves nothing behind.
    pub async fn download(&self, url: &str, dest: &Path, hash: &Hash, max: u64, mut progress: impl FnMut(u64)) -> Result<(), String> {
        self.check(url)?;
        if let Some(dir) = dest.parent() {
            tokio::fs::create_dir_all(dir).await.map_err(|_| "Kunde inte skapa mapp.".to_string())?;
        }
        // append, never replace the extension: "a.so" and "a.dll" must not share one temp name
        let part = { let mut o = dest.as_os_str().to_owned(); o.push(".part"); std::path::PathBuf::from(o) };
        let resp = self.client.get(url).send().await.map_err(|_| "Nätverksfel.".to_string())?;
        if !resp.status().is_success() {
            return Err(format!("Servern svarade {}.", resp.status().as_u16()));
        }
        let mut f = tokio::fs::File::create(&part).await.map_err(|_| "Kunde inte skriva fil.".to_string())?;
        let mut hs = hash.hasher();
        let mut total = 0u64;
        let mut s = resp.bytes_stream();
        let res: Result<(), String> = async {
            while let Some(chunk) = s.next().await {
                let c = chunk.map_err(|_| "Nätverksfel.".to_string())?;
                total += c.len() as u64;
                if total > max {
                    return Err("Filen är för stor.".to_string());
                }
                hs.update(&c);
                f.write_all(&c).await.map_err(|_| "Kunde inte skriva fil.".to_string())?;
                progress(c.len() as u64);
            }
            f.flush().await.map_err(|_| "Kunde inte skriva fil.".to_string())
        }
        .await;
        drop(f);
        if let Err(e) = res {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(e);
        }
        if !hs.finish().eq_ignore_ascii_case(hash.expected()) {
            let _ = tokio::fs::remove_file(&part).await;
            return Err("En nedladdad fil har fel kontrollsumma och kastades.".into());
        }
        tokio::fs::rename(&part, dest).await.map_err(|_| "Kunde inte spara fil.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hashes() {
        // sha1("abc"), sha256("abc")
        assert!(matches(&Hash::Sha1("a9993e364706816aba3e25717850c26c9cd0d89d".into()), b"abc"));
        assert!(matches(&Hash::Sha256("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into()), b"abc"));
        assert!(!matches(&Hash::Sha1("0000".into()), b"abc"));
    }
}
