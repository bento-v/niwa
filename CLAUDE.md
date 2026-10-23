# Niwa

Windows focus app: a timer closes distracting apps and games while a plant grows. The full spec is `SPEC.md`; the visual reference is `niwa-ui.html`.

## Stack

- Rust workspace (stable, MSVC toolchain) with the `windows` crate for Win32 calls.
- Tauri 2 with Svelte 5 and Vite for the card UI.
- SQLite via `rusqlite` for history and settings.
- A separate SYSTEM service binary (`niwa-service`) using `windows-service` for Strict mode.

## Layout

```
crates/
  niwa-blocker/   game detection (Steam, Epic, Riot), block-list matching, process watching and closing, removing an old website block from the hosts file
  niwa-core/      rules and storage: species, growth stages, unlocks, streak, SQLite store
  niwa-cli/       milestone 1 prototype: a command-line session that uses niwa-blocker
  niwa-service/   the Strict service: session engine, named pipe, install and uninstall
app/
  src/            Svelte card UI: App.svelte, screens/, lib/ (plants.ts holds all plant art)
  src-tauri/      the desktop app: commands (main.rs), Gentle session runner, Strict follower, tray, views
  scripts/        make-icons.mjs draws the app and tray icons from the plant art
SPEC.md           product spec; update it when a decision changes
niwa-ui.html      interactive mockup of the seven screens
```

## Commands

```
cargo test                          # all Rust unit tests
cargo run -p niwa-cli -- --help     # the command-line prototype
cd app && npm run check             # type-check the Svelte UI
cd app && npm run dev               # the UI alone in a browser, on a stand-in backend
cd app && npm run tauri dev         # the real app
cd app && npm run icons             # regenerate icons (needs Windows: the 庭 stamp uses Yu Gothic Bold)
cd app && npm run installer         # release build + signed NSIS installer + latest.json, to publish as GitHub release v{version} in target/release/bundle/nsis (never run the installer yourself: it installs the service)
cargo run --release -p niwa-blocker --example perf_probe   # time the blocker's poll on this PC (read-only)
node app/scripts/render-plants.mjs fern,ginkgo out.png   # every stage of some species as a PNG, for reviewing art
```

Useful while developing:

- `http://localhost:1420/#art` shows every plant drawing on one page.
- Browser options for the stand-in backend: `?scale=60` (session speed), `?hours=12`, `?empty`, `?strict`, `?allow` (allow-only list), `?noservice`, `?session` (start with a session running). Add `#mini` to see the mini timer.
- The app has two windows built from the same page: the card (`main`) and the mini timer (`mini`). `main.ts` picks what to draw from the window's label.
- `NIWA_TIME_SCALE=60` makes debug builds of the real app run sessions 60 times faster.
- `NIWA_DATA_DIR=<folder>` keeps a test run's database out of `%LOCALAPPDATA%\Niwa`.
- `NIWA_HEADLESS=1` runs a debug build of the app with no window, for automated tests.
- `niwa-service console` (debug builds) runs the Strict engine as an ordinary program, with `NIWA_PIPE` and `NIWA_SERVICE_DIR` to keep it apart from an installed service. This is how Strict is tested without admin rights.
- `niwa-service dev-abort` (debug builds) ends a Strict session at once.
- `NIWA_UPDATE_FEED=<url>` (debug builds) checks that feed for updates within seconds (plain http allowed); "installing" then stops after the download and signature check. Without it, debug builds never check. `?update` shows the offer in the browser.
- `NIWA_HOSTS=<file>` (debug builds) makes the service clean up that file instead of the real hosts file.
- A debug build with `NIWA_DATA_DIR` set skips the single-instance check, so a test copy runs beside the user's Niwa. Give it its own `WEBVIEW2_USER_DATA_FOLDER` too.

## Hard rules

- Never inject into, hook, or read the memory of another process. Closing means a window close request, then `TerminateProcess`.
- Never touch anti-cheat processes or services (Vanguard, Easy Anti-Cheat, BattlEye). `niwa-blocker` has a protected list that is checked before every close; keep it.
- The Strict service runs as SYSTEM. It must only close programs owned by the account that started the session, in a login session other than 0, and must only ever launch the `niwa-app.exe` beside it. Keep both checks. The one exception (approved Oct 6, 2026) is an update the card hands over (`Request::InstallUpdate`, `niwa-service/src/update.rs`): only from the card in a login session, never during a session, and only an installer that passes `niwa_core::update::check_installer` (signed with Niwa's update key, for exactly the version asked, newer than the service). It is checked in memory, copied into the SYSTEM-only state folder, and that copy is run with `/S /UPDATE`, never `/R`. Keep every one of those checks. The console engine (`run_installers: false`) checks and saves but never runs; never run a real installer in a test. It also opens the card (that same `niwa-app.exe`, via `launch::ensure_card_running`) once per scheduled window when the card isn't running, for the account that sent the schedules (`Request::Schedules`).
- Installing or removing the service changes the user's system and needs admin rights: give the user the command to run, never run it for them.
- Never close a process that no block-list rule matched, and never match on file name alone outside the prototype's `--app` flag. The one exception is an allow-only list (approved Oct 3, 2026): it may close only the session user's own programs that show a taskbar window, and never Windows' own programs, Microsoft's own Store apps, Niwa's folder, anti-cheat or the protected list. Keep all of those checks (`AllowOnly` in `blocklist.rs`, the window and owner filters in `session.rs` and the service).
- Niwa doesn't block websites (removed Oct 6, 2026, at the user's request). The service only takes out a block an earlier version left in the hosts file (between `# BEGIN Niwa` and `# END Niwa`, `hosts.rs`), when it starts and when it's uninstalled; it never writes anything else there and never adds a block. Never test against the real hosts file: use `NIWA_HOSTS`.
- Never run a real allow-only session while testing: it would close the user's apps. Use unit tests, the Blocks screen's preview, or `cargo run -p niwa-blocker --example allow_only_preview` (read-only).
- Never launch anti-cheat games (VALORANT, Rainbow Six Siege, Marvel Rivals) from tests or scripts. Automated tests use Notepad and Geometry Dash; the user tests anti-cheat games by hand.
- All data stays on the PC: no network calls, accounts or analytics. Fonts are bundled, not fetched. The one exception (approved Oct 6, 2026) is the updater in `src-tauri/src/update.rs`: once a day, unless turned off in Settings, it reads the version file at the address in `plugins.updater.endpoints` and sends nothing about the user. It offers an update only outside a session and installs only an installer signed with Niwa's key (`~/.tauri/niwa-updater.key`, never in the repo) for the exact version announced (`requireSignedVersion`). Keep both checks, and add no other network call. The public key is also `niwa_core::update::PUBLIC_KEY`, which the service checks with; a test keeps it and the app's version in step with `tauri.conf.json`.

## Conventions

- The tray is only touched on the main thread (`tray::show_session` posts there). The session thread must never hold a lock while waiting on the main thread.
- The pipe protocol and the saved Strict state live in `niwa-core` (`ipc.rs`, `strict.rs`) so the app and the service share one definition.
- Views sent to the UI live in `src-tauri/src/views.rs` and are mirrored by hand in `app/src/lib/api.ts`; change both together.
- Plant art layers its leaves. Build canopies with `clumps()` in `plants.ts` (overlapping back, middle and front clumps, each outlined by its own leaves); never lay individual leaves flat over one shape. Use `leaf()` and `limb()` for single leaves and trunks, and keep the light coming from the upper left.
- Show and hide windows with `set_shown` in `main.rs`, never `show()`/`hide()` alone: WebView2 keeps drawing a hidden window's page every frame unless told.
- No endless CSS animations or transitions on SVG or on screens shown during a session: each one repaints at the monitor's refresh rate. Slow, tiny motion (the plant's sway) is stepped from script a few times a second.
- Motion follows Niwa's own Settings > Reduce motion, not Windows (the user keeps Windows animations off but wants Niwa's motion). Reduced means fades instead of movement, not stillness: CSS keyframes multiply their distances by `var(--travel)` (0 when reduced), and Svelte transitions take distances from `travel()` in `lib/motion.svelte.ts`. Never add a blanket `prefers-reduced-motion` kill switch.

## Working agreements

- Build one roadmap milestone at a time and stop for review at the end of each.
- Write tests for game detection and unlock math.
- UI not covered by the mockup is designed in its style and approved by the user before the milestone moves on.
