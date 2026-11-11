"use strict";
// Lillypad Launcher UI. Plain TypeScript, no framework.
// All text from the backend is written with textContent. innerHTML is never used.
function el(id) {
    const node = document.getElementById(id);
    if (!node) {
        throw new Error("Saknar element: " + id);
    }
    return node;
}
const ui = {
    app: el("app"),
    dock: el("dock"),
    title: el("status-title"),
    text: el("status-text"),
    note: el("status-note"),
    progressRow: el("progress-row"),
    progress: el("progress"),
    progressLabel: el("progress-label"),
    primary: el("primary"),
    retry: el("retry"),
    loginDialog: el("login-dialog"),
    loginUri: el("login-uri"),
    loginCode: el("login-code"),
    loginOpen: el("login-open"),
    loginCopy: el("login-copy"),
    loginCancel: el("login-cancel"),
    loginWait: el("login-wait"),
    head: el("head"),
    playerName: el("player-name"),
    playerSub: el("player-sub"),
    openSettings: el("open-settings"),
    closeSettings: el("close-settings"),
    newsPanel: el("news-panel"),
    newsVersion: el("news-version"),
    newsList: el("news-list"),
    settingsPanel: el("settings-panel"),
    ram: el("ram"),
    ramValue: el("ram-value"),
    javaValue: el("java-value"),
    themeChoice: el("theme-choice"),
    openFolder: el("open-folder"),
    signOut: el("sign-out"),
};
const state = {
    auth: null,
    pack: null,
    error: null,
    action: "none",
    busy: false,
    updateRequested: false,
    settingsOpen: false,
    loginUri: "",
};
// Links in the footer of the side panel. They are opened by the backend (open_url), never inside the app window.
// Every address here must also be on the backend allowlist. Replace rules and discord with the exact pages.
const LINKS = {
    site: "https://lillypadnetwork.se",
    rules: "https://lillypadnetwork.se",
    discord: "https://lillypadnetwork.se",
};
const DEFAULT_HEAD = "assets/img/head_default.png";
const MONTHS = ["januari", "februari", "mars", "april", "maj", "juni", "juli", "augusti", "september", "oktober", "november", "december"];
const preview = new URLSearchParams(window.location.search);
// ---------------------------------------------------------------- helpers
function formatDate(iso) {
    const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(iso);
    if (!m) {
        return "";
    }
    const month = MONTHS[Number(m[2]) - 1];
    return month ? Number(m[3]) + " " + month + " " + m[1] : "";
}
function formatGb(mb) {
    const gb = Math.round((mb / 1024) * 10) / 10;
    return String(gb).replace(".", ",") + " GB";
}
/** Only a PNG data URL is accepted as a head. Anything else falls back to the bundled default. */
function safeHead(src) {
    return typeof src === "string" && /^data:image\/png;base64,[A-Za-z0-9+/=]+$/.test(src) ? src : DEFAULT_HEAD;
}
/** Shows a link without the scheme. The real opening is done by the backend allowlist. */
function prettyUri(uri) {
    return uri.replace(/^https:\/\//, "").replace(/\/$/, "");
}
function setHidden(node, hidden) {
    node.hidden = hidden;
}
// ---------------------------------------------------------------- theme and scale
function applyTheme(choice) {
    const dark = choice === "dark" || (choice === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
    document.documentElement.setAttribute("data-theme", dark ? "dark" : "light");
    for (const b of Array.from(ui.themeChoice.querySelectorAll("button"))) {
        b.setAttribute("aria-pressed", b.getAttribute("data-theme-choice") === choice ? "true" : "false");
    }
}
function loadTheme() {
    const forced = preview.get("theme");
    if (forced === "light" || forced === "dark") {
        return forced;
    }
    try {
        // The theme choice is the only thing kept in localStorage. Nothing about the account is stored by the UI.
        const saved = window.localStorage.getItem("lillypad.theme");
        if (saved === "light" || saved === "dark" || saved === "system") {
            return saved;
        }
    }
    catch {
        // storage may be unavailable, the default is fine
    }
    return "system";
}
function saveTheme(choice) {
    try {
        window.localStorage.setItem("lillypad.theme", choice);
    }
    catch {
        // ignore
    }
}
function fitWindow() {
    const s = Math.min(window.innerWidth / 960, window.innerHeight / 600);
    ui.app.style.setProperty("--scale", String(Math.max(0.5, s)));
}
// ---------------------------------------------------------------- rendering
function renderAccount() {
    const a = state.auth;
    if (a && a.signed_in && a.player) {
        ui.playerName.textContent = a.player.name;
        ui.playerSub.textContent = a.test_mode ? "Testläge, offline spelare" : "Inloggad med Microsoft";
        ui.head.src = safeHead(a.player.head_data_url);
    }
    else {
        ui.playerName.textContent = "Inte inloggad";
        ui.playerSub.textContent = "Logga in för att spela";
        ui.head.src = DEFAULT_HEAD;
    }
    setHidden(ui.signOut, !(a && a.signed_in));
}
function renderLogin() {
    const prompt = state.auth && !state.auth.signed_in ? state.auth.login : null;
    setHidden(ui.loginDialog, prompt === null);
    if (prompt) {
        state.loginUri = prompt.verification_uri;
        ui.loginUri.textContent = prettyUri(prompt.verification_uri);
        ui.loginCode.textContent = prompt.user_code;
        const min = Math.max(1, Math.ceil(prompt.expires_in_s / 60));
        ui.loginWait.textContent = "Väntar på att du loggar in. Koden gäller i " + min + " min.";
    }
}
function setDock(kind, title, text, opts) {
    ui.dock.setAttribute("data-state", kind);
    ui.title.textContent = title;
    ui.text.textContent = text;
    ui.note.textContent = opts.note ?? "";
    setHidden(ui.note, !opts.note);
    setHidden(ui.progressRow, opts.progress === undefined);
    if (opts.progress !== undefined) {
        const p = Math.max(0, Math.min(100, Math.round(opts.progress)));
        ui.progress.value = p;
        ui.progressLabel.textContent = p + " %";
    }
    ui.primary.textContent = opts.label;
    ui.primary.disabled = !opts.enabled || state.busy;
    ui.primary.classList.toggle("small-text", opts.small === true);
    ui.primary.classList.toggle("stop", opts.action === "stop");
    state.action = opts.action;
    setHidden(ui.retry, opts.retry !== true);
}
function renderDock() {
    const a = state.auth;
    const p = state.pack;
    if (state.error !== null) {
        setDock("error", "Något gick fel", state.error, { label: "Spela", action: "none", enabled: false, retry: true });
        return;
    }
    if (!a) {
        setDock("loading", "Startar", "Kontrollerar konto och modpack.", { label: "Spela", action: "none", enabled: false });
        return;
    }
    if (!a.signed_in) {
        setDock("signed_out", "Välkommen", "Logga in med ditt Microsoftkonto för att spela på Lulumon Create.", {
            note: "Inloggningen sker på Microsofts officiella sida. Launchern ser aldrig ditt lösenord.",
            label: "Logga in med Microsoft", action: "login", enabled: a.login === null, small: true,
        });
        return;
    }
    if (!p) {
        setDock("loading", "Startar", "Kontrollerar modpacket.", { label: "Spela", action: "none", enabled: false });
        return;
    }
    if (p.game_running) {
        setDock("playing", "Spelet körs", "Ha det så kul på Lulumon Create. Du kan stänga spelet här om det hänger sig.", { label: "Stoppa", action: "stop", enabled: true });
        return;
    }
    if (p.updating || p.update_available) {
        const from = p.installed_version ? "Uppdaterar från " + p.installed_version + " till " + p.latest_version + "." : "Installerar modpack " + p.latest_version + ".";
        setDock("updating", "Uppdaterar", from, { progress: p.updating ? p.progress : 0, label: "Uppdaterar", action: "none", enabled: false, small: true });
        return;
    }
    setDock("ready", "Redo att spela", "Modpack " + (p.installed_version ?? p.latest_version) + " är installerat och aktuellt.", { label: "Spela", action: "play", enabled: true });
}
function renderPanels() {
    setHidden(ui.settingsPanel, !state.settingsOpen);
    setHidden(ui.newsPanel, state.settingsOpen);
    ui.openSettings.setAttribute("aria-pressed", state.settingsOpen ? "true" : "false");
}
function render() {
    renderAccount();
    renderLogin();
    renderDock();
    renderPanels();
}
function renderNews(items) {
    ui.newsList.replaceChildren();
    if (items.length === 0) {
        const empty = document.createElement("p");
        empty.className = "news-empty";
        empty.textContent = "Inga nyheter just nu.";
        ui.newsList.append(empty);
        ui.newsVersion.textContent = "";
        return;
    }
    ui.newsVersion.textContent = "Modpack " + items[0].version;
    for (const item of items.slice(0, 10)) {
        const card = document.createElement("article");
        card.className = "news-item";
        const meta = document.createElement("div");
        meta.className = "news-meta";
        const ver = document.createElement("span");
        ver.textContent = "Version " + item.version;
        const date = document.createElement("span");
        date.textContent = formatDate(item.date);
        meta.append(ver, date);
        const title = document.createElement("h3");
        title.className = "news-title";
        title.textContent = item.title;
        const list = document.createElement("ul");
        for (const change of item.changes.slice(0, 12)) {
            const li = document.createElement("li");
            li.textContent = change;
            list.append(li);
        }
        card.append(meta, title, list);
        if (typeof item.url === "string" && item.url.startsWith("https://")) {
            const url = item.url;
            const more = document.createElement("button");
            more.type = "button";
            more.className = "btn-secondary news-more";
            more.textContent = "Läs mer";
            more.addEventListener("click", () => { void run(() => call("open_url", { url })); });
            card.append(more);
        }
        ui.newsList.append(card);
    }
}
// ---------------------------------------------------------------- backend flow
/** Runs one backend command and shows a plain error in the dock if it fails. */
async function run(fn) {
    try {
        return await fn();
    }
    catch (e) {
        state.error = errorText(e);
        render();
        return undefined;
    }
}
async function refresh() {
    try {
        state.auth = await call("auth_status");
        setHidden(document.getElementById("test-banner"), !state.auth.test_mode);
        if (state.auth.signed_in) {
            state.pack = await call("pack_status");
            if (state.pack.update_available && !state.pack.updating && !state.pack.game_running && !state.updateRequested) {
                state.updateRequested = true;
                await call("pack_update");
                state.pack = await call("pack_status");
            }
            if (!state.pack.update_available && !state.pack.updating) {
                state.updateRequested = false;
            }
        }
        else {
            state.pack = null;
            state.updateRequested = false;
        }
        state.error = null;
    }
    catch (e) {
        state.error = errorText(e);
    }
    render();
}
let timer;
function schedule() {
    window.clearTimeout(timer);
    const waitingLogin = !!state.auth && !state.auth.signed_in && state.auth.login !== null;
    const updating = !!state.pack && (state.pack.updating || state.pack.update_available);
    const running = !!state.pack && state.pack.game_running;
    if (state.error !== null) {
        return; // wait for the player to press Försök igen
    }
    const delay = updating ? 500 : waitingLogin ? 2000 : running ? 2000 : 0;
    if (delay > 0) {
        timer = window.setTimeout(() => { void tick(); }, delay);
    }
}
async function tick() {
    await refresh();
    schedule();
}
async function onPrimary() {
    if (state.busy) {
        return;
    }
    state.busy = true;
    render();
    const action = state.action;
    if (action === "login") {
        const prompt = await run(() => call("auth_login"));
        if (prompt && prompt.user_code && state.auth) {
            state.auth = { signed_in: false, player: null, login: prompt };
        }
    }
    else if (action === "play") {
        await run(() => call("game_play"));
    }
    else if (action === "stop") {
        await run(() => call("game_stop"));
    }
    state.busy = false;
    if (state.error === null) {
        await tick();
    }
    else {
        render();
    }
}
async function loadSettings() {
    const s = await run(() => call("settings_get"));
    if (!s) {
        return;
    }
    ui.ram.min = String(s.ram_min_mb);
    ui.ram.max = String(s.ram_max_mb);
    ui.ram.value = String(s.ram_mb);
    ui.ramValue.textContent = formatGb(s.ram_mb);
    ui.javaValue.textContent = s.java === "auto" ? "Automatiskt" : s.java;
}
async function loadNews() {
    try {
        renderNews(await call("news_get"));
    }
    catch {
        renderNews([]);
    }
}
// ---------------------------------------------------------------- events
ui.primary.addEventListener("click", () => { void onPrimary(); });
ui.retry.addEventListener("click", () => { state.error = null; render(); void tick(); });
ui.loginOpen.addEventListener("click", () => {
    if (state.loginUri) {
        void run(() => call("open_url", { url: state.loginUri }));
    }
});
ui.loginCopy.addEventListener("click", () => {
    const code = ui.loginCode.textContent ?? "";
    const done = () => { ui.loginCopy.textContent = "Kopierad"; window.setTimeout(() => { ui.loginCopy.textContent = "Kopiera"; }, 1600); };
    const fallback = () => {
        // Select the visible code and use the old copy command if the async clipboard is refused.
        const r = document.createRange();
        r.selectNodeContents(ui.loginCode);
        const sel = window.getSelection();
        if (sel) { sel.removeAllRanges(); sel.addRange(r); if (document.execCommand("copy")) { done(); } }
    };
    if (code && !navigator.clipboard) { fallback(); }
    else if (navigator.clipboard && code) {
        navigator.clipboard.writeText(code).then(done, fallback);
    }
});
ui.loginCancel.addEventListener("click", () => { void run(() => call("auth_logout")).then(() => tick()); });
ui.openSettings.addEventListener("click", () => {
    state.settingsOpen = !state.settingsOpen;
    renderPanels();
    if (state.settingsOpen) {
        void loadSettings();
    }
});
ui.closeSettings.addEventListener("click", () => { state.settingsOpen = false; renderPanels(); });
ui.ram.addEventListener("input", () => { ui.ramValue.textContent = formatGb(Number(ui.ram.value)); });
ui.ram.addEventListener("change", () => { void run(() => call("settings_set", { ram_mb: Number(ui.ram.value) })); });
ui.openFolder.addEventListener("click", () => { void run(() => call("open_game_folder")); });
ui.signOut.addEventListener("click", () => {
    void run(() => call("auth_logout")).then(() => { state.settingsOpen = false; return tick(); });
});
for (const b of Array.from(ui.themeChoice.querySelectorAll("button"))) {
    b.addEventListener("click", () => {
        const choice = b.getAttribute("data-theme-choice");
        if (choice === "system" || choice === "light" || choice === "dark") {
            saveTheme(choice);
            applyTheme(choice);
        }
    });
}
for (const b of Array.from(document.querySelectorAll(".link-btn"))) {
    b.addEventListener("click", () => {
        const url = LINKS[b.getAttribute("data-link") ?? ""];
        if (url) {
            void run(() => call("open_url", { url }));
        }
    });
}
window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => { applyTheme(loadTheme()); });
window.addEventListener("resize", fitWindow);
window.addEventListener("contextmenu", (e) => { e.preventDefault(); });
document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && state.settingsOpen) {
        state.settingsOpen = false;
        renderPanels();
    }
});
// ---------------------------------------------------------------- start
applyTheme(loadTheme());
fitWindow();
render();
if (!IS_TAURI && preview.get("state") === "settings") {
    state.settingsOpen = true;
    void loadSettings();
}
void loadNews();
void tick();
