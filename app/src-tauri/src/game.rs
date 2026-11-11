//! Game files: Mojang version, libraries, assets, Java runtime, NeoForge. Then the launch command.
//! Every file Mojang serves is checked against the sha1 in Mojang's own manifests. The NeoForge installer is checked
//! against the sha256 in our signed release file.

use futures_util::{stream, StreamExt};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::net::{Hash, Net};
use crate::security::safe_join;

pub struct Item {
    pub url: String,
    pub dest: PathBuf,
    pub sha1: String,
    pub size: u64,
    pub exec: bool,
}

#[derive(Default)]
pub struct Counter {
    pub done: AtomicU64,
    pub total: AtomicU64,
}

pub const OS_NAME: &str = if cfg!(target_os = "windows") { "windows" } else if cfg!(target_os = "macos") { "osx" } else { "linux" };
const CP_SEP: &str = if cfg!(target_os = "windows") { ";" } else { ":" };

fn runtime_platform() -> &'static str {
    match (OS_NAME, cfg!(target_arch = "aarch64")) {
        ("windows", false) => "windows-x64",
        ("windows", true) => "windows-arm64",
        ("osx", false) => "mac-os",
        ("osx", true) => "mac-os-arm64",
        _ => "linux",
    }
}

/// Mojang rule list. No rules means allowed. Otherwise the last matching rule decides, default is no.
pub fn rules_allow(rules: Option<&Value>) -> bool {
    let Some(arr) = rules.and_then(|r| r.as_array()) else { return true };
    if arr.is_empty() { return true; }
    let mut allowed = false;
    for r in arr {
        let os_ok = match r.get("os") {
            None => true,
            Some(o) => {
                let name_ok = o.get("name").and_then(|n| n.as_str()).map(|n| n == OS_NAME).unwrap_or(true);
                let arch_ok = o.get("arch").and_then(|a| a.as_str()).map(|a| a != "x86").unwrap_or(true);
                name_ok && arch_ok
            }
        };
        let feat_ok = r.get("features").is_none(); // no optional features (demo, custom resolution) are ever on
        if os_ok && feat_ok { allowed = r["action"] == "allow"; }
    }
    allowed
}

async fn get_json(net: &Net, url: &str, sha1: Option<&str>) -> Result<Value, String> {
    let b = net.get_bytes(url, 32 * 1024 * 1024).await?;
    if let Some(h) = sha1 {
        if !crate::net::matches(&Hash::Sha1(h.into()), &b) { return Err("Mojangs fil har fel kontrollsumma.".into()); }
    }
    serde_json::from_slice(&b).map_err(|_| "Mojangs fil är trasig.".to_string())
}

pub async fn run_downloads(net: &Net, items: Vec<Item>, c: &Counter) -> Result<(), String> {
    c.total.fetch_add(items.iter().map(|i| i.size).sum::<u64>(), Ordering::Relaxed);
    let results: Vec<Result<(), String>> = stream::iter(items).map(|it| async move {
        if tokio::fs::metadata(&it.dest).await.map(|m| m.len() == it.size).unwrap_or(false) {
            c.done.fetch_add(it.size, Ordering::Relaxed);
            return Ok(());
        }
        net.download(&it.url, &it.dest, &Hash::Sha1(it.sha1.clone()), it.size.max(1) * 2 + 1024, |n| { c.done.fetch_add(n, Ordering::Relaxed); }).await?;
        #[cfg(unix)]
        if it.exec {
            use std::os::unix::fs::PermissionsExt;
            let _ = tokio::fs::set_permissions(&it.dest, std::fs::Permissions::from_mode(0o755)).await;
        }
        Ok(())
    }).buffer_unordered(12).collect().await;
    results.into_iter().collect()
}

pub struct Paths {
    pub root: PathBuf, // game files shared by all launches
}

impl Paths {
    pub fn libraries(&self) -> PathBuf { self.root.join("libraries") }
    pub fn assets(&self) -> PathBuf { self.root.join("assets") }
    pub fn versions(&self) -> PathBuf { self.root.join("versions") }
    pub fn runtime(&self, component: &str) -> PathBuf { self.root.join("runtime").join(component) }
    pub fn java_bin(&self, component: &str) -> PathBuf {
        let r = self.runtime(component);
        if cfg!(target_os = "windows") { r.join("bin").join("javaw.exe") } else if cfg!(target_os = "macos") { r.join("jre.bundle/Contents/Home/bin/java") } else { r.join("bin").join("java") }
    }
    pub fn neoforge_json(&self, nf: &str) -> PathBuf { self.versions().join(format!("neoforge-{nf}")).join(format!("neoforge-{nf}.json")) }
}

/// Vanilla version json, client jar, libraries, assets, Java runtime. Returns (version json, java component).
pub async fn ensure_vanilla(net: &Net, p: &Paths, mc: &str, c: &Counter) -> Result<(Value, String), String> {
    let manifest = get_json(net, "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json", None).await?;
    let entry = manifest["versions"].as_array().and_then(|v| v.iter().find(|e| e["id"] == mc)).ok_or_else(|| "Minecraft versionen finns inte hos Mojang.".to_string())?;
    let vjson = get_json(net, entry["url"].as_str().unwrap_or(""), entry["sha1"].as_str()).await?;
    let vdir = p.versions().join(mc);
    tokio::fs::create_dir_all(&vdir).await.map_err(|_| "Kunde inte skapa mapp.".to_string())?;
    tokio::fs::write(vdir.join(format!("{mc}.json")), serde_json::to_vec(&vjson).unwrap_or_default()).await.map_err(|_| "Kunde inte skriva fil.".to_string())?;

    let mut items = Vec::new();
    let client = &vjson["downloads"]["client"];
    items.push(Item { url: s(client, "url"), dest: vdir.join(format!("{mc}.jar")), sha1: s(client, "sha1"), size: client["size"].as_u64().unwrap_or(0), exec: false });
    for lib in vjson["libraries"].as_array().ok_or_else(|| "Mojangs fil är trasig.".to_string())? {
        if !rules_allow(lib.get("rules")) { continue; }
        let a = &lib["downloads"]["artifact"];
        if a.is_null() { continue; }
        let path = a["path"].as_str().unwrap_or("");
        items.push(Item { url: s(a, "url"), dest: safe_join(&p.libraries(), path)?, sha1: s(a, "sha1"), size: a["size"].as_u64().unwrap_or(0), exec: false });
    }
    // log4j config named in the version file
    let lc = &vjson["logging"]["client"]["file"];
    if !lc.is_null() {
        items.push(Item { url: s(lc, "url"), dest: safe_join(&p.root.join("assets/log_configs"), lc["id"].as_str().unwrap_or("log.xml"))?, sha1: s(lc, "sha1"), size: lc["size"].as_u64().unwrap_or(0), exec: false });
    }
    // asset index and objects
    let ai = &vjson["assetIndex"];
    let ai_id = ai["id"].as_str().unwrap_or("");
    let idx = get_json(net, ai["url"].as_str().unwrap_or(""), ai["sha1"].as_str()).await?;
    let idx_path = safe_join(&p.assets().join("indexes"), &format!("{ai_id}.json"))?;
    tokio::fs::create_dir_all(idx_path.parent().unwrap()).await.map_err(|_| "Kunde inte skapa mapp.".to_string())?;
    tokio::fs::write(&idx_path, serde_json::to_vec(&idx).unwrap_or_default()).await.map_err(|_| "Kunde inte skriva fil.".to_string())?;
    if let Some(objs) = idx["objects"].as_object() {
        for (_, o) in objs {
            let h = o["hash"].as_str().unwrap_or("");
            if h.len() != 40 || !h.chars().all(|c| c.is_ascii_hexdigit()) { return Err("Mojangs assetlista är trasig.".into()); }
            items.push(Item { url: format!("https://resources.download.minecraft.net/{}/{}", &h[..2], h), dest: p.assets().join("objects").join(&h[..2]).join(h), sha1: h.into(), size: o["size"].as_u64().unwrap_or(0), exec: false });
        }
    }
    // Java runtime named by the version file
    let component = vjson["javaVersion"]["component"].as_str().unwrap_or("java-runtime-delta").to_string();
    items.extend(java_items(net, p, &component).await?);
    run_downloads(net, items, c).await?;
    Ok((vjson, component))
}

fn s(v: &Value, k: &str) -> String { v[k].as_str().unwrap_or("").to_string() }

async fn java_items(net: &Net, p: &Paths, component: &str) -> Result<Vec<Item>, String> {
    let all = get_json(net, "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json", None).await?;
    let m = &all[runtime_platform()][component][0]["manifest"];
    let man = get_json(net, m["url"].as_str().ok_or_else(|| "Java finns inte för det här systemet.".to_string())?, m["sha1"].as_str()).await?;
    let base = p.runtime(component);
    let mut out = Vec::new();
    for (path, f) in man["files"].as_object().ok_or_else(|| "Mojangs javalista är trasig.".to_string())? {
        if f["type"] != "file" { continue; }
        let raw = &f["downloads"]["raw"];
        out.push(Item { url: s(raw, "url"), dest: safe_join(&base, path)?, sha1: s(raw, "sha1"), size: raw["size"].as_u64().unwrap_or(0), exec: f["executable"] == true });
    }
    Ok(out)
}

/// Runs the official NeoForge installer (client mode) with the Mojang Java. Needs the vanilla files in place.
pub async fn ensure_neoforge(net: &Net, p: &Paths, nf: &str, installer_sha256: &str, component: &str) -> Result<(), String> {
    if p.neoforge_json(nf).exists() { return Ok(()); }
    let jar = p.root.join("installers").join(format!("neoforge-{nf}-installer.jar"));
    let url = format!("https://maven.neoforged.net/releases/net/neoforged/neoforge/{nf}/neoforge-{nf}-installer.jar");
    net.download(&url, &jar, &Hash::Sha256(installer_sha256.into()), 256 * 1024 * 1024, |_| {}).await?;
    tokio::fs::write(p.root.join("launcher_profiles.json"), br#"{"profiles":{}}"#).await.map_err(|_| "Kunde inte skriva fil.".to_string())?;
    let java = if cfg!(target_os = "windows") { p.runtime(component).join("bin").join("java.exe") } else { p.java_bin(component) };
    let out = tokio::process::Command::new(java).arg("-jar").arg(&jar).arg("--installClient").arg(&p.root)
        .current_dir(&p.root).stdin(std::process::Stdio::null()).output().await.map_err(|_| "Kunde inte köra NeoForge installationen.".to_string())?;
    if !out.status.success() || !p.neoforge_json(nf).exists() {
        let tail: String = String::from_utf8_lossy(&out.stdout).lines().rev().take(3).collect::<Vec<_>>().join(" | ");
        return Err(format!("NeoForge installationen misslyckades. {}", tail.chars().take(160).collect::<String>()));
    }
    Ok(())
}

/// Replaces ${name} in an argument.
fn subst(arg: &str, vars: &[(&str, String)]) -> String {
    let mut s = arg.to_string();
    for (k, v) in vars { s = s.replace(&format!("${{{k}}}"), v); }
    s
}

fn collect_args(v: &Value, out: &mut Vec<String>) {
    if let Some(arr) = v.as_array() {
        for a in arr {
            if let Some(st) = a.as_str() { out.push(st.to_string()); continue; }
            if !rules_allow(a.get("rules")) { continue; }
            match &a["value"] {
                Value::String(x) => out.push(x.clone()),
                Value::Array(xs) => out.extend(xs.iter().filter_map(|x| x.as_str().map(|s| s.to_string()))),
                _ => {}
            }
        }
    }
}

pub struct LaunchInfo {
    pub java: PathBuf,
    pub args: Vec<String>,
    pub secrets: Vec<String>,
}

#[allow(clippy::too_many_arguments)]
pub fn build_launch(p: &Paths, mc: &str, nf: &str, instance: &Path, ram_mb: u64, name: &str, uuid: &str, token: &str, user_type: &str) -> Result<LaunchInfo, String> {
    let rd = |path: PathBuf| -> Result<Value, String> {
        serde_json::from_slice(&std::fs::read(path).map_err(|_| "Spelfilerna saknas. Kör uppdatering.".to_string())?).map_err(|_| "Spelfilerna är trasiga.".to_string())
    };
    let vanilla = rd(p.versions().join(mc).join(format!("{mc}.json")))?;
    let neo = rd(p.neoforge_json(nf))?;
    // The Java runtime is named by Mojang's own version file, never hardcoded.
    let component = vanilla["javaVersion"]["component"].as_str().unwrap_or("java-runtime-delta").to_string();
    let component = component.as_str();
    let mut libs: Vec<PathBuf> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for src in [&neo, &vanilla] {
        for lib in src["libraries"].as_array().into_iter().flatten() {
            if !rules_allow(lib.get("rules")) { continue; }
            let Some(path) = lib["downloads"]["artifact"]["path"].as_str() else { continue };
            if seen.insert(path.to_string()) { libs.push(safe_join(&p.libraries(), path)?); }
        }
    }
    // The vanilla client jar must NOT be on the classpath: NeoForge supplies its own patched client as a module,
    // and both together fail with a module resolution error (seen in a real launch).
    let classpath = libs.iter().map(|l| l.to_string_lossy().to_string()).collect::<Vec<_>>().join(CP_SEP);
    let natives = p.root.join("natives");
    let vars: Vec<(&str, String)> = vec![
        ("auth_player_name", name.into()), ("version_name", format!("neoforge-{nf}")), ("game_directory", instance.to_string_lossy().into()),
        ("assets_root", p.assets().to_string_lossy().into()), ("assets_index_name", vanilla["assetIndex"]["id"].as_str().unwrap_or("").into()),
        ("auth_uuid", uuid.into()), ("auth_access_token", token.into()), ("clientid", "".into()), ("auth_xuid", "".into()),
        ("user_type", user_type.into()), ("version_type", "release".into()), ("launcher_name", "LillypadLauncher".into()),
        ("launcher_version", env!("CARGO_PKG_VERSION").into()), ("classpath", classpath), ("natives_directory", natives.to_string_lossy().into()),
        ("library_directory", p.libraries().to_string_lossy().into()), ("classpath_separator", CP_SEP.into()),
    ];
    let mut jvm = Vec::new();
    collect_args(&vanilla["arguments"]["jvm"], &mut jvm);
    collect_args(&neo["arguments"]["jvm"], &mut jvm);
    let mut game = Vec::new();
    collect_args(&vanilla["arguments"]["game"], &mut game);
    collect_args(&neo["arguments"]["game"], &mut game);
    let mut args = vec![format!("-Xmx{ram_mb}M"), format!("-Xms{}M", ram_mb.min(1024))];
    args.extend(jvm.iter().map(|a| subst(a, &vars)));
    args.push(neo["mainClass"].as_str().or(vanilla["mainClass"].as_str()).unwrap_or("").to_string());
    args.extend(game.iter().map(|a| subst(a, &vars)));
    if args.iter().any(|a| a.contains("${")) { return Err("Startargumenten är ofullständiga.".into()); }
    Ok(LaunchInfo { java: p.java_bin(component), args, secrets: vec![token.to_string()] })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rules() {
        assert!(rules_allow(None));
        assert!(rules_allow(Some(&json!([{"action":"allow"}]))));
        let other = if OS_NAME == "windows" { "linux" } else { "windows" };
        assert!(!rules_allow(Some(&json!([{"action":"allow","os":{"name":other}}]))));
        assert!(rules_allow(Some(&json!([{"action":"allow","os":{"name":OS_NAME}}]))));
        assert!(!rules_allow(Some(&json!([{"action":"allow"},{"action":"disallow","os":{"name":OS_NAME}}]))));
        assert!(!rules_allow(Some(&json!([{"action":"allow","features":{"is_demo_user":true}}]))));
    }

    #[test]
    fn substitution_and_args() {
        let vars = vec![("a", "1".to_string())];
        assert_eq!(subst("x${a}y${a}", &vars), "x1y1");
        let mut out = vec![];
        collect_args(&json!(["--x", {"rules":[{"action":"allow","features":{"has_custom_resolution":true}}],"value":["--w","1"]}, {"value":"--y"}]), &mut out);
        assert_eq!(out, vec!["--x", "--y"]);
    }
}
