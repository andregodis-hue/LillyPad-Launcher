# Code signing policy

Free code signing provided by [SignPath.io](https://signpath.io), certificate by [SignPath Foundation](https://signpath.org).

## What is signed
Only the Windows installer of Lillypad Launcher (`Lillypad Launcher_<version>_x64-setup.exe`), built by the GitHub Actions workflow `.github/workflows/release.yml` in this repository from a release tag, from the source code in this repository. Nothing built on a personal computer is submitted. Test builds (feature `testmode`) are never submitted.

## Team roles
Fill in before applying. Everyone listed uses multi factor authentication on GitHub and on SignPath.

* Authors (may change the source code without extra review): `<GitHub handle of the owner>`
* Reviewers (review every change from someone who is not an author): `<GitHub handle of reviewer>`
* Approvers (approve each signing request by hand in SignPath): `<GitHub handle of the owner>`, `<GitHub handle of a second trusted person>`

## Rules
* Every release is approved manually in SignPath before it is signed.
* The signed installer must come from the workflow run of the release tag. SignPath checks that origin.
* Product name is always "Lillypad Launcher". The product version is the same in every file of a release.
* Changes to `.github/` and `signpath/` get the same review as program code, because they decide what is signed.

## Privacy policy
This program does not collect, store or send usage data, analytics or telemetry. It does not transfer information to other networked systems unless the user asks for it by using a feature:
* Signing in: talks to Microsoft, Xbox Live and Minecraft services (login.microsoftonline.com, xboxlive.com, minecraftservices.com, sessionserver.mojang.com). The Microsoft refresh token is stored on the user's computer in the Windows Credential Manager. Microsoft's privacy statement applies to the login.
* Installing and updating: downloads from lillypadnetwork.se (release file, news), Modrinth (mods), Mojang (game files, Java), and maven.neoforged.net (NeoForge). Those servers see the user's IP address like any web server.
* Updating the launcher: asks lillypadnetwork.se for `latest.json`.
* Playing: the game connects to a Minecraft server only when the user chooses to join one.
The program only changes files in its own folder in the user's profile. The NSIS installer installs for the current user and can be removed from Windows Settings, Apps, Installed apps.

## Repository settings the project must have
* Variables: `SIGNPATH_ORGANIZATION_ID`, `SIGNPATH_PROJECT_SLUG`, `LILLYPAD_MS_CLIENT_ID` (public id, not a secret).
* Secrets: `SIGNPATH_API_TOKEN`, `TAURI_SIGNING_PRIVATE_KEY` (the updater key).
* Protected `main` branch, required review, protected tags `v*`, two factor authentication for all members.
