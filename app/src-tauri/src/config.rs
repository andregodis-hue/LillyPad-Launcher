//! Fixed settings. Nothing here is read from the network.

/// Microsoft application (client) id. Every launcher needs its own, approved by Mojang.
/// Set at build time with LILLYPAD_MS_CLIENT_ID. Without it login reports "not configured".
pub const MS_CLIENT_ID: Option<&str> = option_env!("LILLYPAD_MS_CLIENT_ID");

/// Where the signed release manifest lives. The manifest names the mrpack and carries the news.
pub const RELEASE_URL: &str = "https://lillypadnetwork.se/pack/release.json";
pub const RELEASE_SIG_URL: &str = "https://lillypadnetwork.se/pack/release.json.minisig";

/// minisign public key that signs release.json (second line of keys/pack.pub). The private key never leaves us.
pub const PACK_PUBKEY: &str = include_str!("../pack.pub");

pub const SERVICE: &str = "lillypad-launcher";
pub const MIN_RAM_MB: u64 = 2048;

/// Hosts that downloads may come from. Exact match, https only.
pub const DOWNLOAD_HOSTS: &[&str] = &[
    "lillypadnetwork.se",
    "cdn.modrinth.com",
    "piston-data.mojang.com",
    "piston-meta.mojang.com",
    "launchermeta.mojang.com",
    "libraries.minecraft.net",
    "resources.download.minecraft.net",
    "maven.neoforged.net",
    "textures.minecraft.net",
];

/// Hosts used for the sign in chain only.
pub const AUTH_HOSTS: &[&str] = &[
    "login.microsoftonline.com",
    "user.auth.xboxlive.com",
    "xsts.auth.xboxlive.com",
    "api.minecraftservices.com",
    "sessionserver.mojang.com",
];

/// Pages the "open_url" command may open in the system browser: (host, optional path prefix).
pub const OPEN_URLS: &[(&str, &str)] = &[
    ("lillypadnetwork.se", "/"),
    ("www.microsoft.com", "/link"),
    ("login.live.com", "/oauth20_remoteconnect.srf"),
    ("microsoft.com", "/link"),
    ("discord.gg", "/"),
    ("discord.com", "/invite/"),
];
