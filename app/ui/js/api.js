"use strict";
// The only place the UI talks to the backend. No fetch, no XMLHttpRequest, no WebSocket anywhere in the UI.
const IS_TAURI = typeof window.__TAURI__ !== "undefined" && typeof window.__TAURI__.core !== "undefined";
function call(cmd, args) {
    const payload = args;
    if (IS_TAURI) {
        return window.__TAURI__.core.invoke(cmd, payload);
    }
    if (window.__LILLYPAD_MOCK__) {
        return window.__LILLYPAD_MOCK__(cmd, payload);
    }
    return Promise.reject("Ingen koppling till launcherns bakdel.");
}
/** Turns whatever the backend rejected with into a short plain string. Never logged. */
function errorText(e) {
    let s = "";
    if (typeof e === "string") {
        s = e;
    }
    else if (e && typeof e === "object" && "message" in e && typeof e.message === "string") {
        s = e.message;
    }
    s = s.trim();
    if (s.length === 0) {
        return "Okänt fel.";
    }
    return s.length > 240 ? s.slice(0, 240) + "…" : s;
}
