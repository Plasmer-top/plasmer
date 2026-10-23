# CLAUDE.md

- Windows-only. `build.bat` makes the release exe (Docker cross-compile, icon embedded) at `target/docker/Plasmer.exe`.
- `cargo test` runs the egui_kittest UI tests in `src/gui/tests.rs` (headless; no input is ever sent, settings go to a temp dir).
- `APP_VERSION` in `main.rs` is bumped by hand for each release; the updater compares it against the remote API.
