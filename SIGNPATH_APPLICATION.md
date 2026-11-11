# SignPath Foundation application (draft text and steps)

Everything in angle brackets must be filled in by a person. Source of the rules: https://signpath.org/terms (read on 2026-11-11). The application form itself is at https://signpath.org/apply. The form fields can change, so treat the text below as ready to paste, not as a promise about the form.

## Be aware before applying (honest check against the terms)
* "Released": the project must already be released in the form that will be signed. So the repository must be public and there must be a public release (an unsigned installer on GitHub Releases or on lillypadnetwork.se/launcher) before they approve.
* "Maintained" and "reputation": SignPath Foundation checks project reputation and control. A brand new project with few users may be declined or asked to come back later. Having real players, stars, a download page and a few weeks of commits helps.
* "Documented": the functionality must be described on the download page. Use the README text below.
* Every release needs manual approval, all team members need multi factor authentication on GitHub and SignPath, and the code signing policy must be linked from the home page and download page (CODE_SIGNING_POLICY.md does that, copy its text to the website).
* The installer only is signed in version 1. The program file inside it is not signed separately, so Windows Defender and SmartScreen may still show the installed exe as unknown until reputation builds. Signing both would need a two stage build, later.
* The owner is under 18. A parent or guardian should be the formal contact or at least agree, since SignPath asks for accepting terms. This is the owner's family's decision, not something to guess.
* The Microsoft login needs a Microsoft application id approved by Mojang (separate process, https://aka.ms/AppRegistration, not part of this repository). Signing does not replace it.

## Application text (paste)

**Project name:** Lillypad Launcher

**Repository:** https://github.com/<organisation or user>/lillypad-launcher

**Home page / download page:** https://lillypadnetwork.se/launcher/

**Short description (one sentence):** Lillypad Launcher is a small open source launcher that installs and starts the Lulumon Create modpack (Minecraft with NeoForge) for the Lillypad Network community server.

**Long description:** Lillypad Network is a Swedish Minecraft community server run by a teenager and volunteers. Lillypad Launcher is the desktop program players use to join our Create modpack server. It signs the player in with the official Microsoft device code flow (the program never sees a password), downloads the Minecraft files, Java and NeoForge from Mojang and NeoForge, downloads the mods listed in a signed release file from Modrinth, checks every file against a hash, and starts the game. It updates itself through the Tauri updater with a signed manifest. It is written in Rust with Tauri 2 and a plain HTML interface. It has no analytics or telemetry.

**Open source licence:** MIT (OSI approved). No dual licensing, no proprietary component. The licence was chosen because no GPL or LGPL code is copied into the project (all Rust dependencies are MIT, Apache 2.0, BSD, ISC, Zlib, Unicode or MPL 2.0, checked with cargo deny in CI), and a permissive licence matches the Tauri ecosystem and lets anyone reuse the code. Fonts are SIL OFL 1.1 (compatible, bundled with their licence text).

**Why a signature is needed:** Unsigned installers show a SmartScreen warning that scares players, many of them children. We want players to be able to verify who published the program.

**Build and origin:** The installer is built by GitHub Actions (`.github/workflows/release.yml`) from a release tag, with pinned tool versions and pinned action commits, and cargo audit and cargo deny gates. SignPath receives the artifact from that workflow run.

**Code signing policy URL:** https://lillypadnetwork.se/launcher/ (section "Code signing policy", copy of CODE_SIGNING_POLICY.md) and https://github.com/<organisation or user>/lillypad-launcher/blob/main/CODE_SIGNING_POLICY.md

**Required sentence on the page:** Free code signing provided by SignPath.io, certificate by SignPath Foundation.

**Team roles:**
* Authors: <GitHub handle of the owner>
* Reviewers: <GitHub handle of reviewer>
* Approvers: <GitHub handle of the owner>, <GitHub handle of a second trusted person>
All use multi factor authentication.

**Privacy statement:** This program does not collect or send usage data. It only contacts Microsoft login services, Mojang, Modrinth, NeoForge and lillypadnetwork.se when the user signs in, installs, updates or plays. The Microsoft refresh token is stored in the Windows Credential Manager on the user's computer. Full text in CODE_SIGNING_POLICY.md.

**Artifact configuration:** `signpath/artifact-configuration.xml` (draft, validate in the SignPath UI). Signed file: NSIS installer. Product name "Lillypad Launcher".

**Contact:** <name and email of the responsible adult or maintainer>

## Exact steps for the owner or a parent
Nothing below can be done by the agent. It never creates accounts or uses credentials.

1. **GitHub account with two factor authentication.** Create or use a GitHub account, turn on two factor authentication (Settings, Password and authentication). Do the same for every approver and reviewer.
2. **Create an organisation or repository.** Preferably a GitHub organisation (for example Lillypad Network, free plan) so roles can be shown as teams. Create a public repository `lillypad-launcher`. Do not add a README or licence in the web form.
3. **Check the folder for secrets yourself once more**, then push. In the folder `public_repo`:
   ```
   git init
   git add .
   git status        # look through the list, there must be no keys, .env or logs
   git commit -m "First public version"
   git branch -M main
   git remote add origin https://github.com/<organisation>/lillypad-launcher.git
   git push -u origin main
   ```
4. **Protect the repository.** Settings, Branches: protect `main`, require pull requests with one review. Settings, Tags: protect `v*`. Settings, Code security: turn on private vulnerability reporting.
5. **Repository variables and secrets.** Settings, Secrets and variables, Actions.
   * Variable `LILLYPAD_MS_CLIENT_ID` once the Microsoft app exists.
   * Secret `TAURI_SIGNING_PRIVATE_KEY`: the content of the updater private key file (kept on the VPS in launcher/keys, ask the agent to show it to you on the VPS, never paste it in chat or issues).
   * Later, after SignPath approves: variables `SIGNPATH_ORGANIZATION_ID`, `SIGNPATH_PROJECT_SLUG` and secret `SIGNPATH_API_TOKEN`.
6. **First public release, unsigned.** Add a tag and let the workflow build: `git tag v0.1.0 && git push origin v0.1.0`. Download the installer artifact, publish it on GitHub Releases and on lillypadnetwork.se/launcher/ with the download page text. (SignPath requires that a release exists first.)
7. **Add the Code signing policy section** to the download page and the repository home page (copy from CODE_SIGNING_POLICY.md, fill in real roles).
8. **Apply.** Open https://signpath.org/apply, accept the terms, paste the text above, submit. Wait for the reply by email.
9. **After approval.** In SignPath: create the project with slug (same as the variable), upload `signpath/artifact-configuration.xml`, create the signing policy `release-signing` with manual approval, connect GitHub as trusted build system (SignPath shows the steps for the SignPath GitHub app), create an API token for the CI user and save it as the secret. Set the three SignPath values in the repository.
10. **Sign a release.** Push a new tag. The workflow builds, SignPath asks the approver to approve, the signed installer comes out as the artifact `release-files` together with `latest.json`. Upload the files to lillypadnetwork.se/launcher/ (the signed installer and `latest.json` together). The launcher updates itself from that location.
