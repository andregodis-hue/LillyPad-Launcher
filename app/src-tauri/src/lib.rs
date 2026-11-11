#![cfg_attr(feature = "testmode", allow(dead_code, unused_imports))]
//! Lillypad Launcher backend. The twelve commands below are the whole surface the web view can reach.

mod auth;
mod config;
mod game;
mod head;
mod net;
mod pack;
mod security;
mod settings;

use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::State;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use auth::{AuthStatus, Inner, LoginPrompt, Shared, TokenStore};
use pack::{NewsItem, Release};

#[derive(Serialize)]
struct PackStatus {
    installed_version: Option<String>,
    latest_version: String,
    update_available: bool,
    updating: bool,
    progress: u32,
    game_running: bool,
}

#[derive(Default)]
struct Upd {
    running: AtomicBool,
    /// 0 = game files, 1 = modpack
    phase: AtomicU64,
    game: game::Counter,
    pack: pack::Counter,
    error: Mutex<Option<String>>,
}

struct Running {
    kill: Option<tokio::sync::oneshot::Sender<()>>,
}

struct App {
    root: PathBuf,
    dl: net::Net,
    auth_net: net::Net,
    auth: Shared,
    store: Arc<dyn TokenStore>,
    release: tokio::sync::Mutex<Option<(Instant, Release)>>,
    upd: Arc<Upd>,
    game_running: Arc<AtomicBool>,
    child: Arc<Mutex<Running>>,
    restored: tokio::sync::watch::Sender<bool>,
}

impl App {
    fn game_paths(&self) -> game::Paths { game::Paths { root: self.root.join("game") } }
    fn instance(&self) -> PathBuf { pack::instance_dir(&self.root) }
    fn settings_path(&self) -> PathBuf { self.root.join("settings.json") }

    /// Latest signed release, cached for five minutes. When offline an older copy is still used.
    async fn release(&self) -> Result<Release, String> {
        let mut g = self.release.lock().await;
        if let Some((t, r)) = g.as_ref() {
            if t.elapsed() < Duration::from_secs(300) { return Ok(r.clone()); }
        }
        match pack::fetch_release(&self.dl).await {
            Ok(r) => { *g = Some((Instant::now(), r.clone())); Ok(r) }
            Err(e) => g.as_ref().map(|(_, r)| r.clone()).ok_or(e),
        }
    }
}

type St<'a> = State<'a, Arc<App>>;

#[tauri::command]
async fn auth_status(app: St<'_>) -> Result<AuthStatus, String> {
    let mut rx = app.restored.subscribe();
    let _ = tokio::time::timeout(Duration::from_secs(12), rx.wait_for(|v| *v)).await;
    auth::status(&app.auth)
}

#[tauri::command]
async fn auth_login(app: St<'_>) -> Result<LoginPrompt, String> {
    #[cfg(feature = "testmode")]
    {
        let s = auth::offline_session();
        let mut g = app.auth.lock().unwrap();
        g.player = Some(auth::Player { name: s.name.clone(), head_data_url: None });
        g.session = Some(s);
        // Empty code means no Microsoft dialog; the UI just refreshes.
        return Ok(LoginPrompt { user_code: String::new(), verification_uri: String::new(), expires_in_s: 0 });
    }
    #[cfg(not(feature = "testmode"))]
    auth_login_real(app).await
}

async fn auth_login_real(app: St<'_>) -> Result<LoginPrompt, String> {
    if let Some((p, started)) = app.auth.lock().unwrap().login.as_ref() {
        return Ok(LoginPrompt { user_code: p.user_code.clone(), verification_uri: p.verification_uri.clone(), expires_in_s: p.expires_in_s.saturating_sub(started.elapsed().as_secs()) });
    }
    let (prompt, device_code, interval) = auth::start_device_code(&app.auth_net).await?;
    let gen = {
        let mut g = app.auth.lock().unwrap();
        g.generation += 1;
        g.login = Some((prompt.clone(), Instant::now()));
        g.generation
    };
    let a = app.inner().clone();
    let expires = prompt.expires_in_s;
    tauri::async_runtime::spawn(async move {
        let alive = { let sh = a.auth.clone(); move || sh.lock().map(|g| g.generation == gen && g.login.is_some()).unwrap_or(false) };
        let res = match auth::poll_device_code(&a.auth_net, &device_code, interval, expires, alive).await {
            Ok(t) => auth::finish(&a.auth_net, a.store.as_ref(), &a.auth, gen, t).await,
            Err(e) => Err(e),
        };
        if let Err(e) = res {
            let mut g = a.auth.lock().unwrap();
            if g.generation == gen { g.login = None; g.last_error = Some(e); }
        }
    });
    Ok(prompt)
}

#[tauri::command]
async fn auth_logout(app: St<'_>) -> Result<(), String> {
    auth::logout(&app.auth, app.store.as_ref());
    Ok(())
}

#[tauri::command]
async fn pack_status(app: St<'_>) -> Result<PackStatus, String> {
    if let Some(e) = app.upd.error.lock().unwrap().take() { return Err(e); }
    let installed = pack::load_installed(&app.instance()).version;
    let latest = match app.release().await {
        Ok(r) => r.version,
        Err(e) => match &installed { Some(v) => v.clone(), None => return Err(e) },
    };
    let updating = app.upd.running.load(Ordering::Relaxed);
    let frac = |c_done: &AtomicU64, c_total: &AtomicU64| -> f64 { let t = c_total.load(Ordering::Relaxed); if t == 0 { 0.0 } else { (c_done.load(Ordering::Relaxed) as f64 / t as f64).min(1.0) } };
    let progress = if updating {
        if app.upd.phase.load(Ordering::Relaxed) == 0 { (frac(&app.upd.game.done, &app.upd.game.total) * 45.0) as u32 } else { 50 + (frac(&app.upd.pack.done, &app.upd.pack.total) * 50.0) as u32 }
    } else if installed.as_deref() == Some(latest.as_str()) { 100 } else { 0 };
    Ok(PackStatus { update_available: installed.as_deref() != Some(latest.as_str()), installed_version: installed, latest_version: latest, updating, progress: progress.min(100), game_running: app.game_running.load(Ordering::Relaxed) })
}

#[tauri::command]
async fn pack_update(app: St<'_>) -> Result<(), String> {
    if app.game_running.load(Ordering::Relaxed) { return Err("Stäng spelet innan du uppdaterar.".into()); }
    if app.upd.running.swap(true, Ordering::SeqCst) { return Ok(()); }
    let rel = match app.release().await { Ok(r) => r, Err(e) => { app.upd.running.store(false, Ordering::SeqCst); return Err(e); } };
    let a = app.inner().clone();
    tauri::async_runtime::spawn(async move {
        let u = a.upd.clone();
        u.phase.store(0, Ordering::Relaxed);
        u.game.done.store(0, Ordering::Relaxed); u.game.total.store(0, Ordering::Relaxed);
        u.pack.done.store(0, Ordering::Relaxed); u.pack.total.store(0, Ordering::Relaxed);
        let res = do_update(&a, &rel, &u).await;
        if let Err(e) = res { *u.error.lock().unwrap() = Some(e); }
        u.running.store(false, Ordering::SeqCst);
    });
    Ok(())
}

/// Game files first, then the modpack. Shared by the command and the live test.
async fn do_update(a: &App, rel: &Release, u: &Upd) -> Result<(), String> {
    let paths = a.game_paths();
    let (_, component) = game::ensure_vanilla(&a.dl, &paths, &rel.minecraft, &u.game).await?;
    game::ensure_neoforge(&a.dl, &paths, &rel.neoforge, &rel.neoforge_installer_sha256, &component).await?;
    u.phase.store(1, Ordering::Relaxed);
    let inst = a.instance();
    let old = pack::load_installed(&inst);
    tokio::fs::create_dir_all(&inst).await.map_err(|_| "Kunde inte skapa spelmappen.".to_string())?;
    let managed = pack::install(&a.dl, rel, &inst, &a.root.join("cache"), &u.pack).await?;
    pack::remove_stale(&inst, &old.files, &managed);
    pack::save_installed(&inst, &rel.version, managed)
}

#[tauri::command]
async fn game_play(app: St<'_>) -> Result<(), String> {
    if app.game_running.load(Ordering::Relaxed) { return Ok(()); }
    if app.upd.running.load(Ordering::Relaxed) { return Err("Modpacket uppdateras fortfarande.".into()); }
    let inst = app.instance();
    let installed = pack::load_installed(&inst).version.ok_or_else(|| "Modpacket är inte installerat ännu.".to_string())?;
    let rel = app.release().await.ok().filter(|r| r.version == installed);
    let rel = match rel { Some(r) => r, None => app.release().await.map_err(|_| "Kunde inte läsa release filen.".to_string())? };
    #[cfg(feature = "testmode")]
    let (session, user_type) = (app.auth.lock().unwrap().session.clone().ok_or_else(|| "Du behöver logga in.".to_string())?, "legacy");
    #[cfg(not(feature = "testmode"))]
    let (session, user_type) = {
        let rt = app.store.get().ok_or_else(|| "Du behöver logga in.".to_string())?;
        // Fresh game token for every start, and the rotated refresh token goes straight back to the credential store.
        let tokens = auth::refresh(&app.auth_net, &rt).await?;
        let (session, _) = auth::minecraft_session(&app.auth_net, &tokens.access_token).await?;
        if let Some(n) = &tokens.refresh_token { app.store.set(n)?; }
        (session, "msa")
    };
    let st = settings::load(&app.settings_path());
    let info = game::build_launch(&app.game_paths(), &rel.minecraft, &rel.neoforge, &inst, st.ram_mb, &session.name, &session.uuid, &session.access_token, user_type)?;
    let logs = inst.join("logs");
    tokio::fs::create_dir_all(&logs).await.map_err(|_| "Kunde inte skapa loggmapp.".to_string())?;
    let mut cmd = tokio::process::Command::new(&info.java);
    cmd.args(&info.args).current_dir(&inst).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
    let mut child = cmd.spawn().map_err(|_| "Kunde inte starta Java.".to_string())?;
    app.game_running.store(true, Ordering::SeqCst);
    let (tx, rx) = tokio::sync::oneshot::channel::<()>();
    app.child.lock().unwrap().kill = Some(tx);
    let secrets = info.secrets.clone();
    let log_path = logs.join("launcher.log");
    let out = child.stdout.take();
    let err = child.stderr.take();
    let running = app.game_running.clone();
    tauri::async_runtime::spawn(async move {
        let file = Arc::new(tokio::sync::Mutex::new(tokio::fs::File::create(&log_path).await.ok()));
        let pump = |r: Option<Box<dyn tokio::io::AsyncRead + Unpin + Send>>, f: Arc<tokio::sync::Mutex<Option<tokio::fs::File>>>, sec: Vec<String>| async move {
            if let Some(r) = r {
                let mut lines = BufReader::new(r).lines();
                while let Ok(Some(l)) = lines.next_line().await {
                    let refs: Vec<&str> = sec.iter().map(|s| s.as_str()).collect();
                    let clean = security::redact(&l, &refs);
                    if let Some(fh) = f.lock().await.as_mut() { let _ = fh.write_all(format!("{clean}\n").as_bytes()).await; }
                }
            }
        };
        let o: Option<Box<dyn tokio::io::AsyncRead + Unpin + Send>> = out.map(|x| Box::new(x) as _);
        let e: Option<Box<dyn tokio::io::AsyncRead + Unpin + Send>> = err.map(|x| Box::new(x) as _);
        let t1 = tokio::spawn(pump(o, file.clone(), secrets.clone()));
        let t2 = tokio::spawn(pump(e, file.clone(), secrets));
        tokio::select! { _ = child.wait() => {}, _ = rx => { let _ = child.kill().await; } }
        let _ = t1.await; let _ = t2.await;
        running.store(false, Ordering::SeqCst);
    });
    Ok(())
}

#[tauri::command]
async fn game_stop(app: St<'_>) -> Result<(), String> {
    if let Some(tx) = app.child.lock().unwrap().kill.take() { let _ = tx.send(()); }
    Ok(())
}

#[tauri::command]
async fn settings_get(app: St<'_>) -> Result<settings::LauncherSettings, String> {
    Ok(settings::load(&app.settings_path()))
}

// The UI sends snake case keys ({ ram_mb }), Tauri expects camel case unless told otherwise.
#[tauri::command(rename_all = "snake_case")]
async fn settings_set(app: St<'_>, ram_mb: u64) -> Result<(), String> {
    settings::save(&app.settings_path(), ram_mb).map(|_| ())
}

#[tauri::command]
async fn open_game_folder(app: St<'_>) -> Result<(), String> {
    let d = app.instance();
    std::fs::create_dir_all(&d).map_err(|_| "Kunde inte skapa spelmappen.".to_string())?;
    open::that_detached(&d).map_err(|_| "Kunde inte öppna mappen.".to_string())
}

#[tauri::command]
async fn news_get(app: St<'_>) -> Result<Vec<NewsItem>, String> {
    match app.release().await { Ok(r) => Ok(r.news), Err(_) => Ok(vec![]) }
}

#[tauri::command]
async fn open_url(url: String) -> Result<(), String> {
    let u = security::check_open_url(&url)?;
    open::that_detached(u.as_str()).map_err(|_| "Kunde inte öppna adressen.".to_string())
}

/// Launcher self update: the Tauri updater checks a minisign signed manifest over https and refuses anything
/// that does not verify against the public key in tauri.conf.json. Runs once at start, before any game runs.
async fn self_update(handle: tauri::AppHandle) {
    use tauri_plugin_updater::UpdaterExt;
    if let Ok(u) = handle.updater() {
        if let Ok(Some(up)) = u.check().await {
            if up.download_and_install(|_, _| {}, || {}).await.is_ok() { handle.restart(); }
        }
    }
}

pub fn run() {
    let root = dirs::data_dir().unwrap_or_else(std::env::temp_dir).join("LillypadLauncher");
    let (tx, _rx) = tokio::sync::watch::channel(false);
    let app = Arc::new(App {
        root,
        dl: net::Net::new(config::DOWNLOAD_HOSTS).expect("net"),
        auth_net: net::Net::new(config::AUTH_HOSTS).expect("net"),
        auth: Arc::new(Mutex::new(Inner::default())),
        store: Arc::new(auth::KeychainStore),
        release: tokio::sync::Mutex::new(None),
        upd: Arc::new(Upd::default()),
        game_running: Arc::new(AtomicBool::new(false)),
        child: Arc::new(Mutex::new(Running { kill: None })),
        restored: tx,
    });
    let restore = app.clone();
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(app)
        .setup(move |tauri_app| {
            tauri::async_runtime::spawn(self_update(tauri_app.handle().clone()));
            // Silent sign in from the stored refresh token. A network failure keeps the token, a refusal deletes it.
            let a = restore.clone();
            tauri::async_runtime::spawn(async move {
                if cfg!(feature = "testmode") { let _ = a.restored.send(true); return; }
                if let Some(rt) = a.store.get() {
                    let gen = a.auth.lock().unwrap().generation;
                    match auth::refresh(&a.auth_net, &rt).await {
                        Ok(t) => { let _ = auth::finish(&a.auth_net, a.store.as_ref(), &a.auth, gen, t).await; }
                        Err(e) if e.contains("logga in igen") => a.store.clear(),
                        Err(_) => {}
                    }
                }
                let _ = a.restored.send(true);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            auth_status, auth_login, auth_logout, pack_status, pack_update, game_play, game_stop,
            settings_get, settings_set, open_game_folder, news_get, open_url
        ])
        .run(tauri::generate_context!())
        .expect("error while running the launcher");
}

#[cfg(test)]
mod live {
    use super::*;

    fn test_app(root: PathBuf) -> App {
        let (tx, _rx) = tokio::sync::watch::channel(true);
        App { root, dl: net::Net::new(config::DOWNLOAD_HOSTS).unwrap(), auth_net: net::Net::new(config::AUTH_HOSTS).unwrap(),
            auth: Arc::new(Mutex::new(Inner::default())), store: Arc::new(auth::MemStore(Mutex::new(None))),
            release: tokio::sync::Mutex::new(None), upd: Arc::new(Upd::default()), game_running: Arc::new(AtomicBool::new(false)),
            child: Arc::new(Mutex::new(Running { kill: None })), restored: tx }
    }

    /// Needs the network and about 1.5 GB of disk. Run with: LILLYPAD_TEST_ROOT=/some/dir cargo test live -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn full_install_and_launch_command() {
        let root = PathBuf::from(std::env::var("LILLYPAD_TEST_ROOT").expect("LILLYPAD_TEST_ROOT"));
        let a = test_app(root);
        let rel = pack::fetch_release(&a.dl).await.expect("signed release");
        println!("release {} nf {}", rel.version, rel.neoforge);
        do_update(&a, &rel, &a.upd).await.expect("update");
        let inst = a.instance();
        let installed = pack::load_installed(&inst);
        println!("installed {:?}, {} files", installed.version, installed.files.len());
        assert_eq!(installed.version.as_deref(), Some(rel.version.as_str()));
        let info = game::build_launch(&a.game_paths(), &rel.minecraft, &rel.neoforge, &inst, 4096, "Tester", "00000000000000000000000000000000", "faketoken12345", "msa").unwrap();
        println!("java {:?}", info.java);
        assert!(info.java.exists());
        std::fs::write(a.root.join("launch_args.txt"), info.args.join("\n")).unwrap();
        // a second run must not download again
        do_update(&a, &rel, &a.upd).await.expect("second update");
    }
}
