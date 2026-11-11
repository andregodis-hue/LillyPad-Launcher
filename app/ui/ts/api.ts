// The only place the UI talks to the backend. No fetch, no XMLHttpRequest, no WebSocket anywhere in the UI.

const IS_TAURI = typeof window.__TAURI__ !== "undefined" && typeof window.__TAURI__.core !== "undefined";

function call<K extends keyof CommandMap>(cmd: K, args?: CommandMap[K]["args"]): Promise<CommandMap[K]["result"]> {
  const payload = args as Record<string, unknown> | undefined;
  if (IS_TAURI) {
    return window.__TAURI__!.core!.invoke(cmd, payload) as Promise<CommandMap[K]["result"]>;
  }
  if (window.__LILLYPAD_MOCK__) {
    return window.__LILLYPAD_MOCK__(cmd, payload) as Promise<CommandMap[K]["result"]>;
  }
  return Promise.reject("Ingen koppling till launcherns bakdel.");
}

/** Turns whatever the backend rejected with into a short plain string. Never logged. */
function errorText(e: unknown): string {
  let s = "";
  if (typeof e === "string") {
    s = e;
  } else if (e && typeof e === "object" && "message" in e && typeof (e as { message: unknown }).message === "string") {
    s = (e as { message: string }).message;
  }
  s = s.trim();
  if (s.length === 0) {
    return "Okänt fel.";
  }
  return s.length > 240 ? s.slice(0, 240) + "…" : s;
}
