# Niwa

A focus timer for Windows that grows a garden. Pick how long you want to focus, plant a seed, and Niwa closes the games and apps on your block list until the timer ends. Stay focused and the plant grows; give up early and it withers.

- **Gentle** mode closes distractions while Niwa is open.
- **Strict** mode keeps blocking even if Niwa is closed or the PC restarts.
- Block Steam, Epic and Riot games in one switch, any app you choose, or everything except an allow list.
- Schedules start focus time on their own.
- Eleven plants to unlock as your focus hours add up.

Free, with no account and no ads.

## Download

Get **`Niwa_x.y.z_x64-setup.exe`** from the [latest release](https://github.com/bento-v/niwa/releases/latest) and run it. Windows 10 or 11, 64-bit.

Niwa isn't code-signed, so Windows shows **"Windows protected your PC"** the first time. Click **More info**, then **Run anyway**. Windows then asks for admin rights once: the installer sets up the small background service that Strict mode uses. Updates after that install without asking.

## Privacy

Everything stays on your PC. Niwa has no accounts or analytics and sends nothing about you. The one time it goes online is a daily check of this repository's latest release for a new version, which you can turn off in Settings.

## Uninstall

Settings > Apps > Installed apps > Niwa > Uninstall. You can't uninstall during a Strict session.

## Building from source

Needs Rust (stable, MSVC) and Node.js on Windows.

```
cargo test
cd app
npm install
npm run tauri dev
```

`npm run installer` builds the installer; it needs Niwa's update signing key.
