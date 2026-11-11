// Command contract between the UI and the Rust backend. See README.md.
// The UI never receives tokens, passwords or any other account secret.

interface Player {
  /** Minecraft player name. Shown as plain text. */
  name: string;
  /** PNG of the player head as a data URL ("data:image/png;base64,..."), or null. */
  head_data_url: string | null;
}

interface LoginPrompt {
  /** Short code the player types on the Microsoft page. */
  user_code: string;
  /** The official Microsoft page for the device code flow. Opened only through open_url. */
  verification_uri: string;
  /** Seconds until the code stops working. */
  expires_in_s: number;
}

interface AuthStatus {
  signed_in: boolean;
  player: Player | null;
  /** Set while a device code login is waiting for the player, otherwise null. */
  login: LoginPrompt | null;
  /** True only in test builds. */
  test_mode?: boolean;
}

interface PackStatus {
  installed_version: string | null;
  latest_version: string;
  update_available: boolean;
  /** True while pack_update is running. */
  updating: boolean;
  /** 0 to 100, only meaningful while updating. */
  progress: number;
  /** True while the game process is alive. */
  game_running: boolean;
}

interface LauncherSettings {
  ram_mb: number;
  ram_min_mb: number;
  ram_max_mb: number;
  /** Always "auto" for now. */
  java: string;
}

interface NewsItem {
  version: string;
  /** ISO date, for example 2026-11-08. */
  date: string;
  title: string;
  changes: string[];
  /** Optional page with more details. Opened only through open_url. */
  url: string | null;
}

interface CommandMap {
  auth_status: { args: undefined; result: AuthStatus };
  auth_login: { args: undefined; result: LoginPrompt };
  auth_logout: { args: undefined; result: null };
  pack_status: { args: undefined; result: PackStatus };
  pack_update: { args: undefined; result: null };
  game_play: { args: undefined; result: null };
  game_stop: { args: undefined; result: null };
  settings_get: { args: undefined; result: LauncherSettings };
  settings_set: { args: { ram_mb: number }; result: null };
  open_game_folder: { args: undefined; result: null };
  news_get: { args: undefined; result: NewsItem[] };
  open_url: { args: { url: string }; result: null };
}

type MockInvoke = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

interface Window {
  __TAURI__?: { core?: { invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown> } };
  __LILLYPAD_MOCK__?: MockInvoke;
}
