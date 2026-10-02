# MOTIX build server (ADR-036)

While GitHub is set aside, the owner's server builds and publishes MOTIX updates.

- **Set up once:** double-click `Set up MOTIX build server.cmd`. It installs Rust and the Visual Studio C++ Build
  Tools (free) if missing, stores the update-signing key in `%LOCALAPPDATA%\MOTIX-build` (readable only by this
  Windows account), asks which folder to publish to, and adds the "MOTIX build server" scheduled task (every minute,
  no window, only while signed in).
- **Each update:** Claude writes the changed files into the `motix` folder, checks them, and then writes
  `motix\.motix-build-ready` (its text becomes the release notes). The task builds
  `0.2.0-server.<yyyyMMddHHmm>` with `cargo build --release`, then `xtask publish` writes the archive,
  `SHA256SUMS` (version-pinned) and its signature to the publish folder, signature last.
- **Results:** `MOTIX build status.txt` and `MOTIX build log.txt` next to the `motix` folder.
- **PCs:** shared-folder copies read the share's `updates` folder automatically; own copies use
  Help > Check for updates > *Get updates from a folder on your network*.
- **Remove:** delete the "MOTIX build server" task in Task Scheduler and the `%LOCALAPPDATA%\MOTIX-build` folder.
