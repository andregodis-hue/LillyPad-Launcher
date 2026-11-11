//! Launcher settings: RAM only. Stored as a small json file, never any secret.

use serde::{Deserialize, Serialize};
use std::path::Path;
use sysinfo::System;

use crate::config::MIN_RAM_MB;

#[derive(Serialize, Deserialize, Clone)]
pub struct LauncherSettings {
    pub ram_mb: u64,
    pub ram_min_mb: u64,
    pub ram_max_mb: u64,
    pub java: String,
}

#[derive(Serialize, Deserialize)]
struct Stored {
    ram_mb: u64,
}

/// About 75 percent of the machine, never below the minimum, rounded to 256 MB.
pub fn max_ram_mb() -> u64 {
    let mut sys = System::new();
    sys.refresh_memory();
    let total = sys.total_memory() / (1024 * 1024);
    ((total * 3 / 4) / 256 * 256).max(MIN_RAM_MB)
}

pub fn clamp(ram: u64, max: u64) -> u64 {
    ram.clamp(MIN_RAM_MB, max.max(MIN_RAM_MB))
}

pub fn load(path: &Path) -> LauncherSettings {
    let max = max_ram_mb();
    let stored = std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str::<Stored>(&s).ok());
    let default = clamp(6144, max);
    LauncherSettings { ram_mb: stored.map(|s| clamp(s.ram_mb, max)).unwrap_or(default), ram_min_mb: MIN_RAM_MB, ram_max_mb: max, java: "auto".into() }
}

pub fn save(path: &Path, ram_mb: u64) -> Result<LauncherSettings, String> {
    let max = max_ram_mb();
    let v = clamp(ram_mb, max);
    let json = serde_json::to_string(&Stored { ram_mb: v }).map_err(|_| "Kunde inte spara.".to_string())?;
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|_| "Kunde inte spara.".to_string())?;
    }
    std::fs::write(path, json).map_err(|_| "Kunde inte spara.".to_string())?;
    Ok(load(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clamps() {
        assert_eq!(clamp(100, 8192), 2048);
        assert_eq!(clamp(99999, 8192), 8192);
        assert_eq!(clamp(4096, 8192), 4096);
        assert_eq!(clamp(4096, 1000), 2048);
    }
    #[test]
    fn roundtrip() {
        let d = std::env::temp_dir().join(format!("lp-set-{}", std::process::id()));
        let p = d.join("settings.json");
        let s = save(&p, 3000).unwrap();
        assert_eq!(s.ram_mb, 3000.min(s.ram_max_mb).max(2048));
        assert_eq!(load(&p).ram_mb, s.ram_mb);
        let _ = std::fs::remove_dir_all(d);
    }
}
