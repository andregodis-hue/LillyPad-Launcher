//! The signed release manifest and the modpack (Modrinth mrpack) install and update.
//! Chain of trust: embedded minisign key -> release.json -> mrpack sha256 -> per file sha512 from the index.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::config::{PACK_PUBKEY, RELEASE_SIG_URL, RELEASE_URL};
use crate::net::{Hash, Net};
use crate::security::safe_join;

#[derive(Deserialize, Serialize, Clone)]
pub struct NewsItem {
    pub version: String,
    pub date: String,
    pub title: String,
    pub changes: Vec<String>,
    pub url: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct MrpackRef {
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Deserialize, Clone)]
pub struct Release {
    pub version: String,
    pub minecraft: String,
    pub neoforge: String,
    pub neoforge_installer_sha256: String,
    pub mrpack: MrpackRef,
    pub news: Vec<NewsItem>,
}

/// Checks a minisign signature over `data` with the embedded key.
pub fn verify_signature(data: &[u8], sig: &str, pubkey: &str) -> Result<(), String> {
    let fail = || "Releasefilen har ingen giltig signatur och används inte.".to_string();
    let pk = minisign_verify::PublicKey::decode(pubkey.lines().last().unwrap_or("").trim())
        .or_else(|_| minisign_verify::PublicKey::decode(pubkey))
        .map_err(|_| fail())?;
    let s = minisign_verify::Signature::decode(sig).map_err(|_| fail())?;
    pk.verify(data, &s, false).map_err(|_| fail())
}

pub fn parse_release(data: &[u8]) -> Result<Release, String> {
    let r: Release = serde_json::from_slice(data).map_err(|_| "Releasefilen är trasig.".to_string())?;
    let hex64 = |s: &str| s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit());
    let safe_ver = |s: &str| !s.is_empty() && s.len() < 40 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '+');
    if !hex64(&r.mrpack.sha256) || !hex64(&r.neoforge_installer_sha256) || !safe_ver(&r.version) || !safe_ver(&r.neoforge) || !safe_ver(&r.minecraft) || r.mrpack.size > 200 * 1024 * 1024 {
        return Err("Releasefilen har ogiltiga värden.".into());
    }
    Ok(r)
}

pub async fn fetch_release(net: &Net) -> Result<Release, String> {
    let data = net.get_bytes(RELEASE_URL, 256 * 1024).await?;
    let sig = String::from_utf8(net.get_bytes(RELEASE_SIG_URL, 4096).await?).map_err(|_| "Ogiltig signatur.".to_string())?;
    verify_signature(&data, &sig, PACK_PUBKEY)?;
    parse_release(&data)
}

#[derive(Serialize, Deserialize, Default)]
pub struct Installed {
    pub version: Option<String>,
    pub files: Vec<String>,
}

pub fn load_installed(dir: &Path) -> Installed {
    std::fs::read_to_string(dir.join("installed.json")).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

#[derive(Deserialize)]
struct Index {
    files: Vec<IndexFile>,
    dependencies: std::collections::HashMap<String, String>,
}
#[derive(Deserialize)]
struct IndexFile {
    path: String,
    hashes: std::collections::HashMap<String, String>,
    #[serde(default)]
    env: Option<std::collections::HashMap<String, String>>,
    downloads: Vec<String>,
    #[serde(rename = "fileSize", default)]
    file_size: u64,
}

/// Progress in bytes for the pack phase.
#[derive(Default)]
pub struct Counter {
    pub done: AtomicU64,
    pub total: AtomicU64,
}

fn file_sha512(p: &Path) -> Option<String> {
    use sha2::{Digest, Sha512};
    let mut f = std::fs::File::open(p).ok()?;
    let mut h = Sha512::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = f.read(&mut buf).ok()?;
        if n == 0 { break; }
        h.update(&buf[..n]);
    }
    Some(hex::encode(h.finalize()))
}

/// Downloads and checks the mrpack, then installs it into `instance`. Returns the paths it manages.
pub async fn install(net: &Net, rel: &Release, instance: &Path, cache: &Path, counter: &Counter) -> Result<Vec<String>, String> {
    let zip_path = cache.join(format!("Lulumon-{}.mrpack", rel.version));
    net.download(&rel.mrpack.url, &zip_path, &Hash::Sha256(rel.mrpack.sha256.clone()), 200 * 1024 * 1024, |_| {}).await?;
    let bytes = tokio::fs::read(&zip_path).await.map_err(|_| "Kunde inte läsa modpacket.".to_string())?;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|_| "Modpacket är trasigt.".to_string())?;
    let index: Index = {
        let mut e = zip.by_name("modrinth.index.json").map_err(|_| "Modpacket saknar index.".to_string())?;
        if e.size() > 4 * 1024 * 1024 { return Err("Modpackets index är för stort.".into()); }
        let mut s = String::new();
        e.read_to_string(&mut s).map_err(|_| "Modpacket är trasigt.".to_string())?;
        serde_json::from_str(&s).map_err(|_| "Modpackets index är trasigt.".to_string())?
    };
    if index.dependencies.get("minecraft").map(|v| v != &rel.minecraft).unwrap_or(true) || index.dependencies.get("neoforge").map(|v| v != &rel.neoforge).unwrap_or(true) {
        return Err("Modpacket passar inte release filen.".into());
    }
    let wanted: Vec<&IndexFile> = index.files.iter().filter(|f| f.env.as_ref().and_then(|e| e.get("client")).map(|c| c != "unsupported").unwrap_or(true)).collect();
    counter.total.store(wanted.iter().map(|f| f.file_size).sum::<u64>().max(1), Ordering::Relaxed);
    let mut managed: Vec<String> = Vec::new();
    for f in wanted {
        let dest = safe_join(instance, &f.path)?;
        if !(f.path.starts_with("mods/") || f.path.starts_with("config/") || f.path.starts_with("resourcepacks/") || f.path.starts_with("shaderpacks/")) {
            return Err("Modpacket vill skriva till en otillåten mapp.".into());
        }
        let h512 = f.hashes.get("sha512").ok_or_else(|| "En fil i modpacket saknar sha512.".to_string())?.to_lowercase();
        managed.push(f.path.clone());
        if file_sha512(&dest).as_deref() == Some(h512.as_str()) {
            counter.done.fetch_add(f.file_size, Ordering::Relaxed);
            continue;
        }
        let url = f.downloads.first().ok_or_else(|| "En fil i modpacket saknar adress.".to_string())?;
        net.download(url, &dest, &Hash::Sha512(h512), 512 * 1024 * 1024, |n| { counter.done.fetch_add(n, Ordering::Relaxed); }).await?;
    }
    // Overrides: mods are always replaced, other files (configs) only written when missing so player tweaks stay.
    let names: Vec<String> = zip.file_names().map(|s| s.map(|c| c.to_string())).collect::<Result<_, _>>().map_err(|_| "Modpacket har ogiltiga filnamn.".to_string())?;
    for name in names {
        let rel_path = match name.strip_prefix("client-overrides/").or_else(|| name.strip_prefix("overrides/")) { Some(r) if !r.is_empty() && !name.ends_with('/') => r.to_string(), _ => continue };
        let dest = safe_join(instance, &rel_path)?;
        let is_mod = rel_path.starts_with("mods/");
        if !(is_mod || rel_path.starts_with("config/")) { return Err("Modpacket vill skriva till en otillåten mapp.".into()); }
        if !is_mod && dest.exists() { continue; }
        let mut e = zip.by_name(&name).map_err(|_| "Modpacket är trasigt.".to_string())?;
        if e.size() > 64 * 1024 * 1024 { return Err("En fil i modpacket är för stor.".into()); }
        let mut data = Vec::new();
        (&mut e).take(64 * 1024 * 1024).read_to_end(&mut data).map_err(|_| "Modpacket är trasigt.".to_string())?;
        if let Some(d) = dest.parent() { std::fs::create_dir_all(d).map_err(|_| "Kunde inte skapa mapp.".to_string())?; }
        std::fs::write(&dest, data).map_err(|_| "Kunde inte skriva fil.".to_string())?;
        if is_mod { managed.push(rel_path); }
    }
    Ok(managed)
}

/// Removes mods that an earlier version installed and the new version no longer has. Only touches tracked paths.
pub fn remove_stale(instance: &Path, old: &[String], new: &[String]) {
    let keep: HashSet<&String> = new.iter().collect();
    for p in old {
        if !keep.contains(p) && p.starts_with("mods/") {
            if let Ok(path) = safe_join(instance, p) { let _ = std::fs::remove_file(path); }
        }
    }
}

pub fn save_installed(dir: &Path, version: &str, files: Vec<String>) -> Result<(), String> {
    let s = serde_json::to_string(&Installed { version: Some(version.into()), files }).map_err(|_| "Internt fel.".to_string())?;
    std::fs::write(dir.join("installed.json"), s).map_err(|_| "Kunde inte spara.".to_string())
}

pub fn instance_dir(root: &Path) -> PathBuf { root.join("instance") }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_checks() {
        let ok = br#"{"version":"1.1.1","minecraft":"1.21.1","neoforge":"21.1.259","neoforge_installer_sha256":"0000000000000000000000000000000000000000000000000000000000000000","mrpack":{"url":"https://lillypadnetwork.se/a","sha256":"0000000000000000000000000000000000000000000000000000000000000000","size":5},"news":[]}"#;
        assert!(parse_release(ok).is_ok());
        let bad = String::from_utf8_lossy(ok).replace("1.1.1", "../x");
        assert!(parse_release(bad.as_bytes()).is_err());
        assert!(parse_release(b"{}").is_err());
    }

    #[test]
    fn signature_roundtrip_with_real_key() {
        // Fixture signed with keys/pack.key by `minisign -S` (see tests/fixture). Tampering must fail.
        let data = include_bytes!("../tests/fixture/release.json");
        let sig = include_str!("../tests/fixture/release.json.minisig");
        assert!(verify_signature(data, sig, PACK_PUBKEY).is_ok());
        let mut tampered = data.to_vec();
        tampered[10] ^= 1;
        assert!(verify_signature(&tampered, sig, PACK_PUBKEY).is_err());
        assert!(verify_signature(data, "garbage", PACK_PUBKEY).is_err());
    }
}
