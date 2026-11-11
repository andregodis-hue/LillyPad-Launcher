# Lillypad Launcher: front end

UI for a Tauri 2 launcher for the Lulumon Create server. Swedish interface, plain HTML, CSS and TypeScript, no framework.
This package holds only front end files and design assets. It contains no login, token or network code.

## What is in the package

| Path | What it is |
|---|---|
| `ui/index.html` | The single page. No inline scripts or styles. |
| `ui/css/app.css`, `ui/css/fonts.css` | Styles, light and dark theme, local font faces. |
| `ui/ts/*.ts`, `ui/ts/tsconfig.json` | TypeScript source. `types.d.ts` is the command contract. |
| `ui/js/*.js` | Compiled output of the TypeScript, already built. |
| `ui/assets/img/` | Pixel art pond (light and dark), logo, default head. |
| `ui/assets/fonts/` | Nunito and Pixelify Sans (woff2) with their SIL Open Font License texts. |
| `icons/` | App icons in the sizes Tauri expects, plus `icon.ico` and `icon.icns`. |
| `logo/` | Logo at 512 px, with and without the pond tile. |
| `screenshots/` | Every state in light and dark, 960 x 600, taken from the mock backend. |

## Preview in a normal browser

Open `ui/index.html` directly, no server needed. Without the Tauri bridge the UI uses `js/mock.js`.

Add to the address to jump to a state:
`?state=signed_out`, `device_code`, `ready`, `updating`, `playing`, `settings`, `error`, and `&theme=light` or `&theme=dark`.
Without `?state` the mock is interactive: sign in completes after six seconds, the pack updates, Play and Stop work.

## Rebuild the TypeScript

```
npm install --save-dev typescript
npx tsc -p ui/ts
```

The files are plain scripts (no modules, no bundler). Load order in `index.html`: `mock.js`, `api.js`, `main.js`.

## Tauri setup the UI expects

- `app.withGlobalTauri: true` in `tauri.conf.json`. The UI calls `window.__TAURI__.core.invoke` and imports nothing.
- `build.frontendDist` pointing at the `ui` folder.
- Window 960 x 600. The layout is fixed at that size and scales as a whole when the window is resized
  (keep the 16:10 ratio for a resizable window, or set `resizable: false`).
- Content Security Policy. The page carries this as a meta tag; set the same in `app.security.csp`:

```
default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self';
connect-src 'self' ipc: http://ipc.localhost; base-uri 'none'; form-action 'none'; object-src 'none'
```

- `mock.js` returns immediately when the Tauri bridge exists. You can also delete its script tag for release builds.

## Command contract

The UI calls exactly these commands through `invoke`. Types are in `ui/ts/types.d.ts`. Field names are snake case.
The commands were fixed by the brief; the argument and result shapes below are what this UI assumes, so the backend
must match them or `types.d.ts` and `main.ts` must be adjusted.

| Command | Arguments | Result | When the UI calls it |
|---|---|---|---|
| `auth_status` | none | `AuthStatus` | At start, every 2 s while a device code is waiting, after every action |
| `auth_login` | none | `LoginPrompt` | Player presses "Logga in med Microsoft" |
| `auth_logout` | none | `null` | "Logga ut", and "Avbryt" in the device code dialog (cancels a waiting login) |
| `pack_status` | none | `PackStatus` | At start when signed in, every 500 ms while updating, every 2 s while the game runs |
| `pack_update` | none | `null` | Automatically, once, when `update_available` is true and nothing is running |
| `game_play` | none | `null` | "Spela" |
| `game_stop` | none | `null` | "Stoppa" |
| `settings_get` | none | `LauncherSettings` | When the settings panel opens |
| `settings_set` | `{ ram_mb: number }` | `null` | When the RAM slider is released |
| `open_game_folder` | none | `null` | "Öppna spelmappen" |
| `news_get` | none | `NewsItem[]` (newest first) | At start |
| `open_url` | `{ url: string }` | `null` | "Öppna sidan" in the login dialog, "Läs mer" on a news item, the three footer links |

```ts
AuthStatus       { signed_in: boolean; player: Player | null; login: LoginPrompt | null }
Player           { name: string; head_data_url: string | null }      // "data:image/png;base64,..." or null
LoginPrompt      { user_code: string; verification_uri: string; expires_in_s: number }
PackStatus       { installed_version: string | null; latest_version: string; update_available: boolean;
                   updating: boolean; progress: number /* 0 to 100 */; game_running: boolean }
LauncherSettings { ram_mb: number; ram_min_mb: number; ram_max_mb: number; java: string /* "auto" */ }
NewsItem         { version: string; date: string /* YYYY-MM-DD */; title: string; changes: string[]; url: string | null }
```

Points that go beyond the bare command list:

- `open_url` is a twelfth command. The brief requires links to go through a backend allowlist, so the UI needs it.
  The allowlist must cover the Microsoft device login page, `https://lillypadnetwork.se` and any news or footer link.
- `game_running` is part of `PackStatus` because there is no separate game status command.
- `auth_status.login` lets the UI show the device code again after a reload. If the backend cannot provide it, return `null`;
  the dialog is then driven only by the result of `auth_login`.
- A rejected command should reject with a short string, or an object with a `message` string. The UI shows it as plain
  text under "Något gick fel" with a "Försök igen" button. Send text that is safe to show to a player.

## Security properties of the UI

- No password field and no text input at all. The only inputs are a range slider and buttons.
- The UI never receives a token. It shows only `player.name` and `player.head_data_url`.
- Nothing about the account is stored. `localStorage` holds one key, `lillypad.theme`.
- All backend and news text is written with `textContent`. `innerHTML`, `eval` and `console` are not used anywhere.
- The head image is used only if it is a PNG data URL; anything else falls back to the bundled default head.
- No `fetch`, `XMLHttpRequest` or `WebSocket`. No anchor tags. External pages open only through `open_url`.
- The device code dialog and the signed out screen both state that sign in happens on the official Microsoft page.

## Footer links

`LINKS` at the top of `ui/ts/main.ts` holds the three footer addresses. "Webbplats" points at `https://lillypadnetwork.se`.
"Regler" and "Discord" also point at the site root for now because the exact addresses were not provided.
Replace them, rebuild, and add them to the backend allowlist.

## Untested

- Nothing has been run inside Tauri, WebView2, WKWebView or WebKitGTK. Only headless Chrome with the mock backend.
- The real backend has never been connected. The result shapes above are this UI's assumptions.
- The meta tag policy was exercised only from a `file:` address in Chrome, not under the Tauri protocol.
- `navigator.clipboard` for "Kopiera" may need a permission or the Tauri clipboard plugin in the webview. If it fails
  the button does nothing and the code can still be read and typed.
- Slider and progress bar styling uses WebKit and Firefox pseudo elements; check the look in each target webview.
- `icon.ico` and `icon.icns` were written with Pillow and not opened on Windows or macOS.
- Whole window scaling was checked at 960 x 600 only.
- Swedish text was written without a native review pass.

## Design notes

- Colours start from the site's pale green `#f3f7ec`, with lily pad green and soft pond blue. Flat fills, no gradients.
- The pond scenes, logo and heads are original pixel art made for this launcher. The site's frog mascot is not used.
- The logo is a lily pad with one flower. The app icon puts it on a pond blue tile.
- The login code is set in Nunito, not the pixel face, so characters such as 7, 2 and Z cannot be misread.
