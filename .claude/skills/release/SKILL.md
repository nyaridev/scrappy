---
name: release
description: Cut a new Scrappy release. Bumps the version, drafts the release notes, runs the checks, commits "Release <version>" and tags it; pushing the tag (which builds the signed installers for Windows, macOS and Linux and the extension's zips into a draft GitHub release) and publishing the draft with its notes happen only when the user asks. Use when asked for a new version or release.
argument-hint: <version, e.g. 1.1.0>
---

# Release

Cut Scrappy $ARGUMENTS. With no version given, ask for one: minor for new or changed behaviour, patch for fixes.
Running copies update themselves from the latest published release (ADR-0037), built by
`.github/workflows/release.yml`.

1. **Version**, the same in all four places (`Cargo.lock` follows on the next build):
   - `package.json` (`version`),
   - `extension/package.json` (`version`: the extension's zips are named after it),
   - `Cargo.toml` (`[workspace.package] version`: the version the app reports and the updater compares),
   - `src-tauri/tauri.conf.json` (`version`).
2. **Release notes.** Every release gets them: the workflow makes the draft with an empty body. Draft them in the
   scratchpad from `git log v<previous>..HEAD`: one sentence on what the version brings, a bold-led `-` bullet per
   change a user notices (none for internal fixes), then which file to download on each system
   (`Scrappy_<version>_x64-setup.exe` on Windows, the `.dmg` on macOS, the `.AppImage` or `.deb` on Linux) and, when
   the extension changed, its `-chrome.zip` or `-firefox.zip`. Show them to the user.
3. **Checks.** `pnpm check`, then `build.bat` for the Windows installer. Ask the user to try what the release changes
   in the installed app.
4. **Commit and tag.** Commit only the release's files as `Release <version>`. Tag it:
   `git tag -a v<version> -m "Scrappy <version>"`. No AI attribution.
5. **Push, only when the user asks:** `git push origin main`, then `git push origin v<version>`. The tag starts the
   Release workflow; `gh run watch` follows it. It needs GitHub Actions on for the repo and the
   `TAURI_SIGNING_PRIVATE_KEY` secret.
6. **Publish, only when the user asks:** put the notes on the draft and publish it:
   `gh release edit v<version> -R nyaridev/scrappy --notes-file <notes> --draft=false --latest`.
   Check that the release has `latest.json`, a `.sig` beside each installer and the extension's zips: the updater
   reads `latest.json` and refuses an update without a valid signature. Never mark it a pre-release: the updater only
   sees the latest release.
