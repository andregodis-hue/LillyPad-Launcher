# Security

## Report a problem
Open a private security advisory on GitHub (Security tab, Report a vulnerability) or email the maintainers at the address on the Lillypad Network website. Please do not post details publicly before there is a fix. We answer within a few days, this is a small volunteer project.

## What is protected, and how
* **Web view surface.** Only twelve commands can be called from the interface: auth_status, auth_login, auth_logout, pack_status, pack_update, game_play, game_stop, settings_get, settings_set, open_game_folder, news_get, open_url. They are listed in `app/src-tauri/build.rs` and allowed one by one in `capabilities/default.json`. No file system, shell or http permission is granted to the page.
* **Content Security Policy.** `default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self' ipc: http://ipc.localhost; base-uri 'none'; form-action 'none'; object-src 'none'`. No remote script, no inline script, no eval. The prototype is frozen.
* **Network.** https only. Downloads and the login only talk to hosts on exact match allowlists in `app/src-tauri/src/config.rs`, also after redirects. Download hosts: lillypadnetwork.se, cdn.modrinth.com, piston-data.mojang.com, piston-meta.mojang.com, launchermeta.mojang.com, libraries.minecraft.net, resources.download.minecraft.net, maven.neoforged.net, textures.minecraft.net. Login hosts: login.microsoftonline.com, user.auth.xboxlive.com, xsts.auth.xboxlive.com, api.minecraftservices.com, sessionserver.mojang.com.
* **Integrity.** The release file is signed with minisign and the public key is built into the program. The modpack file is checked by SHA256, every mod by SHA512, Mojang files by SHA1, and the NeoForge installer by a SHA256 from the signed release file. A wrong hash deletes the file.
* **Paths.** Zip entries and pack paths are checked, so nothing can be written outside the game folder. Only mods, config, resourcepacks and shaderpacks are accepted from a pack.
* **Secrets.** The Microsoft refresh token is only in the OS credential store. The game token lives in memory and is removed from the game log. Errors shown to the player never contain tokens.
* **Open links.** `open_url` only opens https links on a short fixed list (lillypadnetwork.se, microsoft.com/link, discord.gg, discord.com/invite).
* **Self update.** The Tauri updater checks a minisign signature against the key in `tauri.conf.json`.
* **Dependencies.** CI runs cargo audit and cargo deny (advisories, licences, sources) on every release build. Two Linux only advisories from the GTK stack are ignored, they are not in the Windows build.

## Known limits (honest list)
* The NeoForge installer is run with the Java that the launcher installed. It downloads its own libraries from hosts that are not on the launcher's allowlist (it is a separate program with its own network code). The installer file itself is pinned by SHA256 from the signed release file, and the installer verifies each library against hashes in its own install profile. We do not control those downloads, so the residual risk is the trust in NeoForge and the libraries it names.
* Mods are third party code. They run with the player's rights once the game starts, like any Minecraft mod.
* The Microsoft login only works once the project owns an approved Microsoft application id.
* Windows installers are signed through SignPath Foundation only for official release tags. Test builds are not signed.
