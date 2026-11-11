"use strict";
// Mock backend for browser preview. Loaded before api.ts. It does nothing when the Tauri bridge exists.
// Preview states: add ?state=signed_out | device_code | ready | updating | playing | settings | error
// and ?theme=light | dark to the address. Without ?state the mock is interactive from the signed out state.
// Nothing here talks to a network. All values are invented.
(function () {
    if (typeof window.__TAURI__ !== "undefined") {
        return;
    }
    const params = new URLSearchParams(window.location.search);
    const forced = params.get("state");
    const start = forced ?? "signed_out";
    const MOCK_HEAD = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAYAAACqaXHeAAABBElEQVR4Xu2azQ0BURRGjajEYoqwEg3YWUwLWpAgahCJPQs7DehDA9ogUcC7iy8zeXO/c7b+zz1vyHuabjH/ToyZGn/2PwigAHMowDwACqAAloA5FGAeAAVQgPsSmEUC9tsuukvVHM/34vuzLwABVfc7AAgYQHLVIKDq8QxA877tOBdwxn4JIMA5f47GOBtkRyjeD2i7U/EysTxcirdf2490men79bkISuNJAAISDFECAZK+BNgLYD8gQcUSCJD0JQABCYYogQBJXwLC/weoPB8v6SnWm5X0+Aj7AhAQJZIdBGSfcAQCIkPZ6f13QN/f4yr2BSBATWjsIGDsE1RBgGpw7PwA/jMXFBjcwXUAAAAASUVORK5CYII=";
    const player = { name: "Grodprinsen", head_data_url: MOCK_HEAD };
    const prompt = { user_code: "F7KQ2XMD", verification_uri: "https://www.microsoft.com/link", expires_in_s: 900 };
    const signedInStates = ["ready", "updating", "playing", "settings", "error"];
    let signedIn = signedInStates.indexOf(start) >= 0;
    let login = start === "device_code" ? prompt : null;
    let installed = start === "updating" ? "1.4.1" : "1.4.2";
    const latest = "1.4.2";
    let updating = start === "updating";
    let progress = start === "updating" ? 62 : 0;
    let running = start === "playing";
    let ramMb = 6144;
    let failing = start === "error";
    const news = [
        { version: "1.4.2", date: "2026-11-08", title: "Höstuppdateringen är här", url: "https://lillypadnetwork.se",
            changes: ["Create uppdaterad till senaste versionen", "Tolv nya Cobblemon i träsket och vid dammen", "Bättre prestanda i stora fabriker", "Rättat en krasch när man öppnade tågkartan"] },
        { version: "1.4.1", date: "2026-10-24", title: "Små fixar", url: null,
            changes: ["Rättat recept för mässingsrör", "Servern startar om klockan 05 varje natt"] },
        { version: "1.4.0", date: "2026-10-10", title: "Tåg och nya biomer", url: null,
            changes: ["Nya biomer runt spawn", "Tågstationen vid spawn är öppen", "Flera moddar uppdaterade"] },
    ];
    function later(value, ms = 120) {
        return new Promise((resolve) => { window.setTimeout(() => resolve(value), ms); });
    }
    window.__LILLYPAD_MOCK__ = function (cmd, args) {
        switch (cmd) {
            case "auth_status": {
                const status = { signed_in: signedIn, player: signedIn ? player : null, login: signedIn ? null : login };
                return later(status);
            }
            case "auth_login":
                login = prompt;
                if (forced === null) { // interactive preview: pretend the player finished on the Microsoft page
                    window.setTimeout(() => { if (login) {
                        signedIn = true;
                        login = null;
                        installed = "1.4.1";
                    } }, 6000);
                }
                return later(prompt);
            case "auth_logout":
                signedIn = false;
                login = null;
                running = false;
                updating = false;
                return later(null);
            case "pack_status": {
                if (failing) {
                    return new Promise((_, reject) => { window.setTimeout(() => reject("Kunde inte nå uppdateringsservern. Kontrollera din internetanslutning."), 120); });
                }
                if (updating && forced === null) {
                    progress = Math.min(100, progress + 4);
                    if (progress >= 100) {
                        updating = false;
                        installed = latest;
                        progress = 0;
                    }
                }
                const status = { installed_version: installed, latest_version: latest, update_available: installed !== latest, updating, progress, game_running: running };
                return later(status);
            }
            case "pack_update":
                updating = true;
                progress = forced === "updating" ? 62 : 0;
                return later(null);
            case "game_play":
                running = true;
                return later(null, 400);
            case "game_stop":
                running = false;
                return later(null);
            case "settings_get": {
                const s = { ram_mb: ramMb, ram_min_mb: 2048, ram_max_mb: 16384, java: "auto" };
                return later(s);
            }
            case "settings_set":
                if (args && typeof args["ram_mb"] === "number") {
                    ramMb = args["ram_mb"];
                }
                return later(null);
            case "open_game_folder":
                return later(null);
            case "news_get":
                return later(news);
            case "open_url":
                return later(null); // the preview opens nothing
            default:
                return Promise.reject("Okänt kommando: " + cmd);
        }
    };
    // In the error preview, Försök igen works on the second attempt.
    if (failing) {
        window.addEventListener("click", (e) => {
            const t = e.target;
            if (t && t.id === "retry") {
                failing = false;
            }
        }, true);
    }
})();
