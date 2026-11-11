# Lillypad Launcher

English first, Swedish below.

Lillypad Launcher installs and starts the Lulumon Create modpack for the Lillypad Network server. The player signs in with Microsoft, the launcher installs the game, Java, NeoForge and the mods, keeps them up to date and starts the game. It is written in Rust with Tauri 2. The interface is plain HTML, CSS and TypeScript.

Licence: MIT. See LICENSE and THIRD_PARTY.md.

## What it does
* Signs in with the Microsoft device code flow. The player types a code on microsoft.com/link. The launcher never sees a password. The refresh token is kept in the Windows Credential Manager.
* Downloads a signed release file (`release.json`, minisign) from lillypadnetwork.se, then installs the modpack and the game files it names. Every file is checked against a SHA1, SHA256 or SHA512 hash before it is used.
* Starts Minecraft with NeoForge, with the player's own session.
* Updates itself through the Tauri updater, which only accepts updates signed with the project's update key.

## Code signing policy
Free code signing provided by [SignPath.io](https://signpath.io), certificate by [SignPath Foundation](https://signpath.org). Roles, rules and the privacy statement: see CODE_SIGNING_POLICY.md. Only builds made by the GitHub Actions workflow in this repository from a release tag are submitted for signing, and every submission is approved by hand.

## Privacy
The launcher sends no analytics and no telemetry. It only contacts the hosts listed in SECURITY.md, and only because the player asked for something (sign in, install, update, play). See CODE_SIGNING_POLICY.md.

## Build
You need Rust 1.99.1 (or newer), `cargo install tauri-cli --version "^2" --locked`, and on Windows the Microsoft C++ build tools and WebView2.

```
cd app/src-tauri
cargo test --locked
cargo audit
cargo deny check advisories bans licenses sources
cargo tauri build            # installer in target/release/bundle/nsis
```

Set `LILLYPAD_MS_CLIENT_ID` at build time to your own Microsoft application (client) id. Every launcher needs its own id, approved by Mojang (https://aka.ms/AppRegistration). Without it the login button says that sign in is not set up.

### Test build
`cargo tauri build --features testmode` makes a clearly marked test build. Play then works with an offline player name and no Microsoft login, and the window shows a red banner. The server runs in online mode, so such a build cannot join it. The release workflow never enables this feature, and the feature is off by default.

### Releases
The workflow `.github/workflows/release.yml` builds on a tag `v*`, uploads the unsigned installer, submits it to SignPath (when the repository variables are set), then signs the updater signature and writes `latest.json`. Required repository settings are listed in CODE_SIGNING_POLICY.md.

## Layout
* `app/src-tauri/src`: `lib.rs` (the twelve commands), `auth.rs`, `pack.rs`, `game.rs`, `net.rs` (https only, host allowlist, hash checks), `security.rs`, `settings.rs`, `head.rs`.
* `app/ui`: the interface (see docs/UI.md). `app/ui/ts` is the source, `app/ui/js` is the compiled output that the app loads.
* `tools/release.py`: builds and signs `release.json` for a new modpack version (maintainers only, needs the private pack key, which is never in this repository).
* `signpath/`: draft artifact configuration for SignPath.

## Interface credit
The interface and artwork were designed and written by Fable (an AI model by Anthropic) as original work for this project. Fonts: Nunito and Pixelify Sans, SIL Open Font License 1.1.

---

# Lillypad Launcher (svenska)

Lillypad Launcher installerar och startar modpacket Lulumon Create för Lillypad Networks server. Spelaren loggar in med Microsoft, launchern installerar spelet, Java, NeoForge och modparna, håller dem uppdaterade och startar spelet. Den är skriven i Rust med Tauri 2. Gränssnittet är vanlig HTML, CSS och TypeScript.

Licens: MIT. Se LICENSE och THIRD_PARTY.md.

## Vad den gör
* Loggar in med Microsofts enhetskod. Spelaren skriver en kod på microsoft.com/link. Launchern ser aldrig något lösenord. Inloggningstoken sparas i Windows Credential Manager.
* Hämtar en signerad releasefil (`release.json`, minisign) från lillypadnetwork.se och installerar modpacket och spelfilerna den pekar ut. Varje fil kontrolleras mot en hash innan den används.
* Startar Minecraft med NeoForge och spelarens egen session.
* Uppdaterar sig själv med Taurys uppdatering, som bara godtar uppdateringar signerade med projektets nyckel.

## Kodsigneringspolicy
Gratis kodsignering från SignPath.io, certifikat från SignPath Foundation. Roller, regler och integritetstext finns i CODE_SIGNING_POLICY.md.

## Integritet
Launchern skickar ingen statistik och ingen telemetri. Den kontaktar bara de adresser som står i SECURITY.md, och bara när spelaren ber om något (logga in, installera, uppdatera, spela).

## Bygga
Se avsnittet Build ovan. Testbygget (`--features testmode`) har en röd banner, spelar med offlinenamn och kan inte gå med på den publika servern. Det är avstängt i alla releasebyggen.
