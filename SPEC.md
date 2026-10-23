# Niwa — Project Spec v1

Oct 1, 2026 · @dreadbalony

## Overview

Niwa is a free Windows focus app: you set a timer, it closes distracting apps and games, and a plant grows; when the timer ends the plant joins your garden. Everything is free; people who want to support it can pay what they wish on the website (decided Oct 3, 2026).

- **For:** students and PC gamers who lose study or work time to games like Valorant, Fortnite, Rocket League and Steam titles.
- **Promise:** one click blocks every game on the PC, and a calm, cozy reason to stay focused.
- **v1 scope:** Windows only · app and game blocking (website blocking was added Oct 4, 2026 and removed Oct 6) · Gentle and Strict modes · local-only data · built with Claude Code.
- **Design reference:** the [Niwa UI mockups](https://claude.ai/artifact/7qTbrRSuEe79SAH57EBBJB) (seven screens).

## Core loop

A session runs start to finish with no pause; it ends only by finishing (the plant joins the garden) or giving up (the plant withers).

1. **Start.** Pick a duration (25, 50, 90 min or custom, from 10 min to 5 h), a seed and a block list. If a blocked app is already running, a 60-second save-and-quit countdown appears, then the app is closed.
2. **Grow.** The timer runs and the plant moves through 5 stages at 0, 25, 50, 75 and 100% of the session. The card and the tray both show time left and the current stage.
3. **Slip.** Launching a blocked app closes it at once and shows a gentle nudge; the plant keeps growing. Each slip is logged for stats.
4. **Finish.** A soft chime and a Windows notification play, and the grown plant is placed in the garden.
5. **Ending early.** In both modes, "Request to end" starts a wait while blocking carries on; it can be cancelled, and when it ends the plant withers and stays in the garden as a reminder. Gentle waits 5 minutes; Strict waits a third of the session (see Blocking). Changed Oct 5, 2026: Gentle used to end at once with Give up.

Rules:

- No pausing, in either mode.
- Withered sessions do not count toward unlock hours.
- A day counts toward the streak when at least one session finishes.
- Time with the PC locked counts toward a session. Time with the PC asleep does not: the timer freezes and carries on at wake.
- Slips affect stats only.

## Blocking

Niwa watches for new processes and closes anything on the active block list, using a normal process close only: no injection, no hooking, no memory reads.

**Closing.** Ask the app's windows to close, wait a couple of seconds, then terminate the process if it is still running.

**Detection.** Watch process starts (a Windows process-start event, with a 1-second poll as fallback). Both modes currently run on a half-second poll; the process-start event (which needs admin rights, so only the Strict service could use it) is not built yet. Match each process by more than its file name, so renaming an .exe does not get around it: games by install folder, publisher signature and product name; user-picked apps by full path, signature, product name and file hash.

| Block-list toggle | How games are found |
| --- | --- |
| All Steam games | Library folders listed in `libraryfolders.vdf`, then each `steamapps\common` folder |
| All Epic games | Install manifests in Epic's ProgramData folder, which give each install location |
| Riot games | Riot Client install paths plus the "Riot Games" signature |
| Battle.net · EA · Ubisoft | Each launcher's install records |
| Also close launchers (optional) | Steam, Epic Games Launcher, Riot Client, Battle.net and the others |
| Specific apps | An .exe the user picks (Discord, Spotify, etc.) |

- **Exceptions** list for allowed apps inside a blocked category (e.g. Wallpaper Engine).
- **Anti-cheat** services (Vanguard, Easy Anti-Cheat, BattlEye) are never touched; only the game process is closed.

**Ready-made categories (Oct 3, 2026).** Switches on the Blocks screen that block a group of known apps: Chat & social (Discord, Telegram, WhatsApp, Signal, Messenger, Instagram, TikTok), Music & video (Spotify, Netflix, Prime Video, Disney+, VLC) and Web browsers (Chrome, Edge, Firefox, Opera, Brave). Each row says how many are installed, from Windows' list of installed apps and the user's Store packages. A known app is matched by its version resource's product name together with its program's file name, or by the Store package it runs from; never by file name alone. The catalog lives in `niwa-blocker/src/catalog.rs`. Store package names for apps not installed on the development PC (WhatsApp, Messenger, Instagram, TikTok, Netflix, Prime Video, Disney+) are unverified; a wrong one simply never matches.

**Allow-only lists (Oct 3, 2026).** A list is either a block list or allow-only (the "Block list / Allow only" switch). Allow-only closes every app that isn't allowed, within these limits, which keep it safe:
- Only programs showing a taskbar window (the Alt-Tab test: visible, top-level, not owned, not a tool window) are closed. Tray apps, overlays, desktop widgets and background helpers are left alone. A program that later opens a window is closed then.
- Only the session user's own programs, in their login session.
- Never anything in the Windows folder, Microsoft's own Store apps (Terminal, Notepad, Calculator...; not its games: Xbox, Minecraft, Solitaire...), Niwa's own folder, anti-cheat, or the protected system processes. The Always allowed exceptions still apply.
- Switching to allow-only with an empty allow list fills it with the work apps installed here (browsers, VS Code, Office, Teams, Slack, Zoom, Notion, Obsidian, Claude, Acrobat), for the user to trim. Removed suggestions stay offered below the list.
- The Blocks screen shows what would close if a session started now ("Would close now: ..."), for both kinds of list; it only looks.
- Apps open at the start get the same minute to save and quit, and launches count as slips.
- Strict: the service runs in session 0 and can't see the user's windows, so the card reports the processes with taskbar windows on every poll (`Request::Windows`). The service accepts the report only from the `niwa-app.exe` beside it, signed in as the session's account, and closes no allow-only candidate without a report from the last 3 seconds. If the card is killed, allow-only pauses until the service relaunches it (about 5 seconds). Services report `PROTOCOL` 2; the card won't start an allow-only or category list in Strict on an older service, which would ignore them, and asks for the service to be restarted.
- Store apps that draw inside Windows' ApplicationFrameHost are found through the frame's child windows.

**Telling the user when something was closed (Oct 3, 2026).** Every app Niwa itself closes (asked to close, or ended; not one that quit on its own) sends `app-closed`, slip or not, in Gentle and Strict. Closes within a few seconds of each other gather into one message ("Discord and Spotify were closed"), which stays up 4.5 seconds after the last one. The mini timer's second line reads "Closed Discord" in clay, with one soft ring around the plant and a warmer border. On the card's Focus screen the note in the scene shows it; on other tabs a note rises from the bottom of the card and the Focus tab keeps a small clay dot until it's next visited. Nothing takes focus or comes to the front, so a game isn't interrupted. The Windows notification for slips while the card is hidden is unchanged.

**Modes.**

|  | Gentle | Strict |
| --- | --- | --- |
| Ending early | Request to end, then 5 minutes | Request to end, then a third of the session (at least 5 minutes) |
| Closing the card or app | Tucks the card into the tray; the session carries on | Blocking continues in the background service |
| PC restart | Session ends | Session resumes and keeps blocking |
| Admin rights | Not needed | Needed once, at install |

**Strict mode internals.**

- A Windows service running as SYSTEM does the blocking; the card and the service watch each other and restart the other if it is closed.
- Session state lives in a folder only the service can write, with a checksum so edited files are rejected.
- Time is measured with a monotonic clock, so changing the system clock does nothing.
- The uninstaller refuses to run while a Strict session is active.

**Cooldown exit.** Ask to end early, then wait while blocking continues; the request can be cancelled. When the wait ends, the session ends and the plant withers. If the session reaches its end during the wait, the plant grows. The wait (`Mode::exit_wait_secs` in niwa-core, mirrored by `exitWaitMinutes` in the card) is 5 minutes in Gentle and a third of the session in Strict, rounded up to whole minutes and never under 5: 25 min → 9, 50 → 17, 60 → 20, 90 → 30, 5 h → 100. (Until Oct 5, 2026 Strict waited a flat 15 minutes and Gentle could give up at once; Strict services now report PROTOCOL 5, and the card won't start a Strict session on an older one.)

**Accepted gaps.** Safe Mode and a second admin account can get around Strict mode; v1 does not try to stop them.

## Plants & garden

Session length decides which seeds you can plant; total focused hours unlock new species. All thresholds below are starting values to tune in testing.

| Species | Minimum session | Unlocked at |
| --- | --- | --- |
| Sprout | Any length | Start |
| Succulent | 30 min | Start |
| Camellia | 45 min | Start |
| Pine bonsai | 50 min | Start |
| Fern | 25 min | 2 h total |
| Bamboo | 60 min | 5 h total |
| Hydrangea | 45 min | 8 h total |
| Maple | 90 min | 10 h total |
| Sakura | 60 min | 15 h total |
| Wisteria | 90 min | 25 h total |
| Ginkgo | 120 min | 40 h total |

Five species were added on Oct 3, 2026: Jack-o'-lantern (a pumpkin vine whose grown stage was a carved, candlelit pumpkin; removed at the user's request on Oct 18, 2026), Fern (arching fronds and fiddleheads), Hydrangea (blue and lilac heads built as clumps of florets), Sakura (a cherry bonsai under pink blossom) and Ginkgo (a tiered tree that turns gold). They're drawn with the same kit and rules as the first seven. Its id (`jackolantern`) is no longer known: a plant or session saved with it shows as a Sprout, the same fallback as any unknown id. The seed picker lists species in this table's order, by hours and then session length; a tile widens for a long name.

**Seed row scrolling (Oct 3, 2026).** The row of seeds can be dragged with the mouse (`lib/dragscroll.ts`): it follows the pointer one to one, a fling carries its speed and coasts to where it would stop (Apple's projection, deceleration 0.998), and it settles on a critically damped spring with a seed lined up at the left edge. Past either end it stretches with growing resistance and springs back. A drag never selects the seed under the pointer; grabbing the row mid-glide catches it. The mouse wheel glides the row the same way. Touch and sideways touchpad swipes keep native scrolling. With Reduce motion on, the row jumps straight to where it settles.

- **Growth:** every species has 5 stages (seed, sprout, sapling, young, fully grown) plus a withered version. Art is flat SVG in the mockup's style: two-tone clay pot, layered leaves.
- **Garden:** a grid of plants, newest first; each tile shows species, session length and date. Withered plants appear greyed. The newest plant is outlined.
- **Locked seeds** show on the start screen with a lock and their requirement, so users can see what's next.

## Window & UI

Niwa is a floating card that minimizes into a mini timer. It opens at 380 × 620 and can be resized larger (up to about 960 × 1250) by dragging its edges or corners; it remembers its size. Larger, the room behind the plant widens with the plant kept in the middle, and the garden shows more plants across. (Resizing added, and the Always on top setting removed, on Oct 4, 2026: the card tucks into the mini timer when another window is clicked, so keeping it on top no longer made sense. The mini timer itself stays on top.)

- **Mini timer** (added Oct 2, 2026): minimising the card collapses it into a small always-on-top panel in the bottom-right corner of the screen, above the taskbar. During a session it shows the plant at its current stage, the time left, the plant's name and stage, and a Gentle or Strict tag; during a Strict cooldown the second line reads "Ending in 12:04". With no session it reads "Ready to focus · Click to plant a seed". Clicking it opens the card and the timer goes away. It never takes keyboard focus when it appears. It can be dragged anywhere (a press that moves more than a few pixels is a drag, not a click), and it reappears where it was left; if that spot is on a monitor that's no longer connected, it goes back to the corner. Clicking another program's window also tucks the card into the mini timer (added Oct 4, 2026). While the card is shown, Niwa checks which program is in front four times a second; once a Niwa window has been in front, another program coming to the front tucks the card away. Niwa's own windows (a file picker, the tray menu) don't count. A card that never had the keyboard (launched by the installer, or brought back when a session ends) stays until it has been used. (The first version reacted to the card losing focus, and tucked a card away before anyone saw it when Windows didn't give it the keyboard at launch; fixed Oct 6, 2026.)

- **Tray icon:** the 庭 stamp (see Logo below); hovering shows time left and stage ("18:20 left · Pine, stage 3 of 5"). Its menu opens the card, shows the mode, and quits when no session is running.
- **Screens:** Plant a seed, Focus (running), Fully grown, Withered, Garden, Stats, Blocks, as in the mockups.
- **Sound and notifications:** a soft chime when a plant finishes; Windows notifications for finishing, slips and the save-and-quit countdown. Sound can be muted in settings.
- **Startup:** optional start with Windows, launching straight to the tray.
- **Motion:** colors fade over 0.25 s, buttons press in slightly, selections slide. Niwa runs its motion whatever Windows' animation setting is (changed Oct 2, 2026); Settings > Reduce motion swaps movement for fades and stops the plant's sway.
- **Scene behind the plant:** pale wall with an ajiro (woven bamboo) pattern, wooden rail and posts, and the pot on a low wooden stand.

**Design tokens**

| Token | Value |
| --- | --- |
| Card background | #F6F0E4 |
| Panel / tab bar | #EADFCB |
| Text | #3E3226 |
| Muted text | #6B5844 |
| Matcha (primary) | #4E6A35 |
| Matcha tint (selected) | #DCE5C4 |
| Clay pot | #A8825E |
| Heading font | Fraunces, soft setting 100 |
| Body font | Figtree |

### Screens

Seven screens, all 380 × 620, sharing one header (logo, minimize to tray, close) and a four-tab bar: Focus, Garden, Stats, Blocks. The Focus tab shows Plant a seed when idle and the running session otherwise. The interactive reference, niwa-ui.html (every screen, with the working animations), lives in the repo next to this spec; match its spacing, colors and motion.

**1. Plant a seed (Focus tab, idle)**

&#91;image: Plant a seed screen\]

- Duration picker: 25, 50, 90 min or Custom; the highlight slides to the selection.
- Seed cards: available seeds plus locked ones with their requirement; the selected card lifts with a green border.
- Growth stages strip previews the 5 stages.
- Block list row opens the Blocks tab; the main button reads "Plant & start · \<duration>".

**2. Focus (session running)**

&#91;image: Focus screen during a session\]

- Mode label, large countdown, progress bar, and chips for what's being blocked.
- Plant scene: ajiro wall, low wooden stand, current stage; labels for species, stage and time to fully grown.
- Request to end (both modes; starts the wait). "Skip to bloom" is a mockup-only button.

**3. Fully grown**

&#91;image: Fully grown screen\]

- Full-size finished plant, minutes focused, slips, streak.
- Place in garden (primary) or Grow another.

**4. Withered**

&#91;image: Withered screen\]

- Greyed scene with the withered plant; calm copy with the time that was left.
- Plant a new seed (primary) or View garden.

**5. Garden**

&#91;image: Garden screen\]

- 3-column grid, newest first and outlined; each tile shows species, length and date.
- Withered plants greyed; footer shows the next unlock.

**6. Stats**

&#91;image: Stats screen\]

- This week's focus time with the change from last week, a bar per day (today highlighted).
- Tiles: streak, hours this month, completion rate. "Most tempting" lists the apps opened most during sessions.
- Shows this week. (Longer ranges and export were planned as paid extras; there are no paid features, see Data & monetization.)

**7. Blocks**

&#91;image: Block lists screen\]

- Named list with Switch list; Gentle/Strict switch with a sliding highlight.
- Game launcher toggles, Also close launchers, app chips with remove, Add app, and the exceptions line.

## Data & monetization

All data stays on the PC: no account, no server and no analytics in v1. The one time Niwa goes online is the daily update check (Oct 6, 2026, below), which sends nothing about the user.

**Storage.** A SQLite file in the user's local app-data folder for history and settings. Strict-mode session state sits in a separate, service-owned folder.

| Record | Fields |
| --- | --- |
| Session | id, start, end, planned minutes, mode, seed, outcome (grown or withered), slip count, block list |
| Slip | session id, time, app name |
| Plant | id, session id, species, created |
| Block list | id, name, category toggles, apps (path + signature), exceptions |
| Settings | always on top, start with Windows, sound, theme |

Total hours, unlocked species and streak are calculated from sessions, not stored.

**No paid features (Oct 3, 2026).** Niwa is free, with every feature for everyone. A pay-what-you-want option will be offered on the website, outside the app: nothing in the app checks or unlocks anything, and the "paid" flag built in milestone 4 was removed. The extras once planned as paid (full history, long-range charts, CSV export, scheduled sessions, uninstall lock, longer cooldown options) are not in v1.

## Tech stack & architecture

Use Tauri 2: a Rust backend with a web UI, so the mockups' HTML and CSS carry straight over and the app stays small (a few MB, low memory).

&#91;embedded content: Niwa architecture · app, core, two blockers\]

Gentle mode needs only the app; Strict mode adds the service, which keeps blocking even when the card is closed.

| Part | Tech |
| --- | --- |
| UI | Tauri 2 webview with Svelte or React, reusing the mockup's styles |
| Core | Rust: timer, session state, plant and unlock logic |
| Process watching | Rust with the `windows` crate (process events, signature checks) |
| Strict service | Separate Rust binary using the `windows-service` crate, talking to the app over a named pipe |
| Storage | SQLite via `rusqlite` |
| Installer | Tauri's NSIS or MSI bundler plus service registration; code-signed before public release |

**Working with Claude Code**

- Keep this spec in the repo as `SPEC.md`, plus a short `CLAUDE.md` with the stack, folder layout and hard rules (never inject into game processes).
- Build one roadmap milestone at a time and have Claude Code write tests for game detection and unlock math.
- Test blocking on Notepad and a free Steam game before trying Valorant.

## Roadmap

Build the riskiest part, blocking, first, then add the cozy layer around it.

1. **Blocker prototype.** A bare app that closes Notepad during a timer, then detects and closes a Steam game. Test with harmless apps before real games.
2. **Gentle MVP.** The card UI, start/focus/grown/withered screens, garden, tray, local storage.
3. **Strict mode.** Background service, cooldown exit, resume after restart, protected session state.
4. **Stats and polish.** Stats screen, chime and notifications, start with Windows, more species.
5. **Release prep.** Installer, code-signing certificate, simple website, privacy policy. (Payment became pay-what-you-want on the website; no paid features.)

**Not in v1:** Mac, accounts or sync, social features, website blocking (in v1 from Oct 4 to Oct 6, 2026; see below).

**Open questions**

- [x] Custom duration range: 10 min to 5 h.
- [x] Sleep and lock: locked time counts, asleep time does not.
- [x] Slips affect stats only; no badge.
- [x] Payment route: none in the app. Pay what you want on the website, later.
- [ ] Final unlock thresholds after playtesting.
- [x] Can a plain close end games protected by anti-cheat? Yes. ARC Raiders (Easy Anti-Cheat) closed normally in Gentle mode (Oct 1, 2026). In Gentle mode, terminating `VALORANT-Win64-Shipping.exe` was refused with "Access is denied" (Vanguard), yet the game was still kept from running. A second hand test on Oct 2, 2026 confirmed Gentle mode closes VALORANT. Which step does it — the window close request or closing the `VALORANT.exe` bootstrapper — wasn't pinned down. Strict mode against VALORANT is still untested.

**Build decisions (Oct 1, 2026)**

- UI framework: Svelte.
- Screens the mockups do not cover (7-species seed picker, settings, custom duration, countdown, slip nudge, Strict cooldown, Add app, Switch list, exceptions) are designed in the mockup's style during their milestone and approved before moving on.
- Plant art for every species, stage and withered state is drawn as SVG in the mockup's style and approved before use.
- Automated blocker tests use Notepad and Geometry Dash. Anti-cheat games (VALORANT, Rainbow Six Siege, Marvel Rivals) are only ever tested by hand.
- Audit the UI with the apple-design skill once the card exists (milestone 2).

**Milestone 2 as built (awaiting approval)**

Designs added for things the mockups do not cover:

- Seed picker: all seven seeds in one row that scrolls sideways, with the fifth peeking in. A seed that needs a longer session or more hours shows a lock and its requirement.
- Custom duration: choosing Custom opens a slider under the duration buttons, 10 min to 5 h in 5-minute steps, with minus and plus buttons.
- Save-and-quit countdown: a clay-tinted banner on the Focus screen replaces the Blocking chips while apps that were already open wait out their 60 seconds.
- Slip nudge: a note slides in over the plant scene for six seconds: "<app> was closed. Your <plant> is still growing."
- Giving up, and closing the app mid-session, ask for confirmation in a sheet that rises from the bottom of the card. "Skip to bloom" is gone.
- Blocks: each launcher row shows how many games were found. Add app opens a file picker. "Always allowed" opens a sheet to add an app or folder and remove entries. The list is locked while a session runs.
- Garden tiles show the date under the species and length; the grid scrolls, replacing "See all".
- Plant art: redrawn for all seven species and five stages to one method (one light source, split leaves, tapered trunks). One withered drawing is shared by every species.
- A finished or withered session brings the card back if it was in the tray, without taking keyboard focus.

Behaviour decided while building:

- Picked apps are matched by path, or by the product name and original file name in their version resource if they have been moved or renamed. Signature and hash matching are not built yet.
- Riot and Epic games are matched by install folder; the "Riot Games" signature check is not built yet.
- "Also close launchers" closes the launchers of the categories that are switched on.
- The default list blocks Steam, Epic and Riot games and allows Wallpaper Engine if it is installed.
- Several blocked processes from one launch within five seconds count as one slip.

Not in milestone 2: Strict mode, Battle.net · EA · Ubisoft detection, Switch list, the Stats screen (a stand-in shows totals), sound, Windows notifications, settings, start with Windows. Always on top is in the tray menu for now.

**Milestone 3 as built (awaiting approval)**

Strict sessions run in a separate program, `niwa-service.exe`, installed as the Windows service "NiwaStrict" running as SYSTEM.

- The card hands the session to the service over a named pipe and then follows it. Closing or killing the card does not stop the blocking, and the service brings the card back on screen. The card starts the service again if it has been killed, and Windows restarts it too.
- The service only ever closes programs that belong to the account that started the session and run in a normal login session. It never closes services or another account's programs.
- A service cannot see the user's windows, so the card asks a blocked program's windows to close and the service ends the program two seconds later.
- Session state is saved every five seconds to `%ProgramData%\Niwa\session.json`, in a folder only SYSTEM can write, sealed with a key only SYSTEM can read. A file that fails its seal is ignored.
- Time is counted on the awake clock. After a restart the session resumes from the last save; time with the PC off or the service down does not count.
- The service refuses to be stopped while a session runs, and `niwa-service uninstall` refuses too.
- Cooldown exit: "Request to end" opens a sheet, then starts a 15-minute wait shown on the Focus screen. Blocking continues, and "Cancel and keep growing" calls it off. If the session reaches its end during the wait, the plant grows.
- In Strict the close button tucks the card into the tray instead of asking to give up.
- The Blocks screen offers Strict only when the service is installed; otherwise it shows "needs setup".
- Development builds of the service accept `niwa-service dev-abort`, which ends a session at once. Release builds do not.

Not tested yet, because they need the service installed with admin rights: running as SYSTEM, the folder permissions, the service restarting the card, and resuming after a real reboot. The logic behind them passed a 26-check run with the service running as an ordinary program.

Hand test on Oct 2, 2026: the service installed and runs. The session tested was Gentle (the list was never switched to Strict), so no Strict session has run against the installed service yet.

The installer step that installs the service was built on Oct 3, 2026; see "Efficiency pass and installer" below.

**Milestone 4 as built (awaiting approval)**

- Stats screen, built to the mockup: this week's focus time with the change from last week (left out when there was nothing last week), a bar per day with today highlighted and days still to come as stubs, then tiles for streak, hours this month and completion rate (finished sessions as a share of all sessions ended this month). Focus time on the Stats screen (the day bars, the week total and its change, hours this month) includes the time a withered session ran before it ended, capped at its planned length (changed Oct 2, 2026); grown sessions count in full. Withered time still doesn't count toward unlocks, and "Most tempting": the three apps closed most often during sessions this week. Free shows this week only, as planned; nothing longer is built yet.
- Chime: two soft bell tones, made in the app rather than played from a file, when a plant is fully grown. It can be turned off.
- Windows notifications: when a plant is fully grown; when a blocked app is closed while the card is hidden (the card shows its own note when it's open); and once per session when apps that were already open are about to be closed.
- Settings, behind a gear button in the card's header: Start with Windows, and Sound (Always on top was removed on Oct 4, 2026). Theme isn't offered: only the light design exists.
- Start with Windows uses Tauri's autostart plugin. It is off until the user turns it on, and it starts Niwa to the mini timer.
- A "paid" flag was stored and reported to the UI, off by default. Removed on Oct 3, 2026: Niwa has no paid features.

Not in this milestone: longer stats ranges and CSV export (no longer planned for v1), more species (all seven were drawn in milestone 2; five more came on Oct 3), a dark theme.

**Polish before milestone 5 (Oct 2, 2026)**

- Text fits everywhere: every screen was checked with the longest real values (Wisteria, five-hour sessions, long app names). Garden tiles show the species on one line and the length and date on the next, with "New" as a small badge. Long titles break into even lines. App chips and the labels over the plant shorten with an ellipsis rather than run over. Long lengths read as hours ("5 hours of focus").
- Garden close-up: clicking a tile opens the plant in its scene, with whether it grew or withered, how long it lasted, how many slips, the date and time, and the mode. Arrow buttons (or the arrow keys) step to newer and older plants; Back or Escape returns to the grid.
- Motion, which Settings > Reduce motion turns into plain fades (it no longer follows Windows): Stats bars grow up one after another, its tiles settle in and the "Most tempting" bars fill; garden tiles settle in turn; the result screen's plant rises in, then its words; the growth strip refreshes when the seed changes; switches stretch slightly while pressed; the mini timer settles in when it appears.
- Motion pass (Oct 2, 2026): screens drift in from the side of the tab you moved toward; the growing plant sways very slowly about its soil line during a session; the session bar moves every second instead of stepping by whole percents (its one-second glide was dropped on Oct 3, 2026 for efficiency: the bar moves under half a pixel a second); Stats tiles count up as they settle; sheets rise on a long soft curve and leave quicker the same way; garden close-ups slide in the direction you stepped; the Withered card's colour fades in. Switches glide on the compositor and stretch from anywhere on their row. Bugs fixed: a switch on Blocks slides back if saving fails; keyboard focus moves into the garden close-up and back to its tile; arrow keys no longer step garden plants behind an open sheet; choosing a seed scrolls only the seed row.

**Logo (Oct 2, 2026):** the app icon is 庭 ("niwa", garden) as a hanko-style stamp in matcha ink (#4E6A35) on white: a carved rounded frame, the character in Yu Gothic Bold, tilted slightly, with wandering edges, sparse fine specks and soft lighter patches from uneven pressure. (Solid-ink versions with more or fewer specks were tried and set aside; the user chose this first version.) The tray icon is the same stamp, upright, bolder and without texture so it reads at 16 pixels; it no longer changes with the plant's stage (the stage is in its hover text). A small plain version replaces the sprout beside "niwa" in the card's header.

**Efficiency pass and installer (Oct 3, 2026)**

Measured on the user's PC (240 Hz monitor, about 370 processes) with a hidden test copy of the app:

- Hidden windows now tell WebView2 they're hidden (`set_shown` in `main.rs`). Before, a card tucked behind the mini timer went on drawing every frame for the whole session: about 43% of one CPU core during a session with both windows hidden. Now about 2%. A hidden page pauses its animations and slows its timers; the Blocks preview skips its look while hidden.
- The growing plant's sway is redrawn ten times a second from script instead of as a CSS animation, which on SVG repainted the whole scene every frame. It moves well under a pixel a second, so it looks the same. With the progress bar's glide also gone, the card on screen during a session dropped from about 25% of a core to about 4%.
- The process list comes straight from the kernel into a buffer the watcher reuses, about half the cost of the ToolHelp snapshot (about 4.5 ms against 8 ms per poll, twice a second). A process's identity now includes its start time, and its owner is looked up once when it is first seen instead of on every poll.
- The Blocks preview keeps its watcher and its scan of installed games (rescanned after a minute or when the list changes), so each look every five seconds only inspects programs started since the last.
- The garden draws 30 plants at a time and more as it scrolls. Opening a garden of 1,500 plants took 6 seconds; now about 150 ms at any size.
- Release builds use link-time optimisation and strip debug symbols.
- Second pass (Oct 17, 2026), measured with a test copy on a hidden desktop: WebView2 now runs both windows' pages in one page process (`--renderer-process-limit=1`) and does its drawing inside its main process instead of a separate GPU process (`--in-process-gpu`), set in `BROWSER_ARGS` (`main.rs`) and the card's `additionalBrowserArgs`. Private memory went from about 215 MB to 167 MB with the mini timer up, and 251 MB to 190 MB with the card up; eight processes became six. Both windows draw the same as before, transparency included. The trade-off: if the graphics driver crashes, Niwa's windows go with WebView2's main process instead of the GPU process restarting alone (sessions carry on: they run in Rust and the service). Telling a hidden page to trim its memory (WebView2's low memory target) was tried and made no measurable difference, so it isn't used.
- The growing plant's sway is redrawn four times a second instead of ten (each step still under a fifth of a pixel at the leaf tips), since every redraw repaints the scene.

Installer: `npm run installer` (in `app/`) builds the service in release, bundles it beside `niwa-app.exe` and makes an NSIS installer in `target/release/bundle/nsis/`.

- It installs for all users into Program Files and asks for admin rights once. A per-user install would let the user swap the program that the SYSTEM service runs.
- After copying files it registers and starts the NiwaStrict service (`niwa-service install`). If that fails, Niwa is still installed, Gentle works, and the installer says why.
- Updating or uninstalling first stops and removes the service, and both refuse while a Strict session is running (`niwa-service` exits with 3).
- "Delete the application data" in the uninstaller also removes `%LOCALAPPDATA%\Niwa`. `%ProgramData%\Niwa` (the sealed session and its key) stays.
- `niwa-service install` now updates an existing NiwaStrict registration (such as the development one) instead of failing, and `uninstall` waits for the service to stop and treats "not installed" as done.
- Tested without admin rights (Oct 3, 2026): every installer and uninstaller hook was run against a stand-in `niwa-service.exe` for each outcome (no service yet, success, session running, other failure), and the real `niwa-service install` and `uninstall` both refuse with exit 3 during a Strict session on a test pipe. Not yet tested, because it needs admin: a real install, update and uninstall on this PC.
- Not signed yet, by choice for now (Oct 3, 2026): Windows SmartScreen warns until there's a code-signing certificate, which is needed before a public release. WebView2 is downloaded by the installer only on a PC that doesn't have it (Windows 11 always does).

**Website blocking (Oct 4, 2026; removed Oct 6, 2026, see the end of this spec)**

The user chose the hosts file, worked by the service. On Oct 4, 2026 they also asked to drop the News category and let people type in their own sites.

- Two switches under "Websites" on the Blocks screen, for block lists and allow-only lists alike: Social media (Reddit, X, Facebook, Instagram, TikTok, Threads, Bluesky, Pinterest, Tumblr) and Video & streaming (YouTube, Twitch, Netflix, Prime Video, Disney+, Hulu, Max, Kick). Each site's host names are listed in full in `niwa-blocker/src/sites.rs` (the hosts file has no wildcards). YouTube Music is left alone; Prime Video inside amazon.com isn't blocked.
- Your own sites: chips under the switches, removed with ×, and "+ Add site", which opens a sheet with one address field. A pasted URL is cut down to its site (`https://www.example.com/page` becomes `example.com`), and the site is blocked with and without "www.". Only plain names of letters, digits, dots and hyphens are accepted: no IP addresses, no `localhost` or `.local`, no Microsoft sites (Defender treats a hosts file blocking Microsoft as tampering), at most 100. The check (`niwa-core/src/site.rs`) runs when typing, again when the list is saved, and again in the service before anything reaches the hosts file. Lists that still name the News category have it dropped when next saved.
- During a session the service writes the sites into the hosts file, pointed at 0.0.0.0 and ::, between "# BEGIN Niwa" and "# END Niwa" lines, then flushes Windows' DNS cache. Nothing else in the file is touched, and the file is swapped in whole (`ReplaceFileW`, which keeps its permissions), so a crash can't leave half a file.
- Strict: the session's sites stay blocked until it ends, across restarts, and the block is checked every 5 seconds and put back if removed.
- Gentle: the card asks the service (`Request::Sites`, PROTOCOL 4) and repeats it every 5 seconds; the service lets the sites through when the session ends, or 30 seconds after the card stops asking (it was killed). Only the card may ask, and only the card may start a Strict session that blocks websites.
- The service clears a leftover block when it starts, and `niwa-service uninstall` clears one too.
- Without the service (or with one older than PROTOCOL 4) the switches show "needs setup" and a session using them won't start.
- Chips on the Focus screen read "Social sites" and "Video sites", then the one site typed in ("example.com") or how many ("3 sites"); the list's summary says the same.
- Limits: the block covers every browser and every account on the PC; a VPN or a browser set to its own secure DNS without checking the hosts file could get around it. YouTube, once visited, shows its own "You're offline" page from the browser's cache instead of an error: that means it is blocked (found Oct 4, 2026 when the user reported YouTube still opening).
- Browsers restart at session start (Oct 4, 2026, the user's choice of three options): a browser keeps open connections and its own address cache, so a site that was already open kept working. When a session that blocks websites starts, Chrome, Edge and Brave windows the user has open get the same 60 seconds as apps open at the start (the Focus banner reads "Chrome restarts in 42 s so blocked sites stop loading. Your tabs come back.", plus one Windows notification), then restart. They are closed through Windows' Restart Manager, as installers do for updates, so they save every window and tab and exit normally, and are reopened with `--restore-last-session`. Tested on a hidden desktop with a throwaway Edge profile: two windows and four tabs came back, and the browser recorded a normal exit. Asking each window to close lost a window, and ending the process lost tabs and recorded a crash, so neither is used. A browser that won't close is left open. Firefox isn't restarted (it can't be reopened with its session from outside). Strict sessions restart browsers from the card, so only when the card started the session, not when it picks one back up.
- Tested on Oct 4, 2026 with a test copy of the service writing to a temporary hosts file (`NIWA_HOSTS`): Strict and Gentle start, renew, give up, the card being killed, a block removed by hand, and a stale block at startup; later, typed-in sites including a pasted URL and an attempt to slip a second line in, which was dropped. Not yet tested against the real hosts file: that needs the installed service.

**Blank card at launch, fixed Oct 5, 2026.** The user launched Niwa two minutes after the PC woke and got an empty card (header only). Cause: the app's state was registered only at the end of start-up, after the mini timer's window was made; making a WebView2 window keeps handling messages while it waits, so the card's page could load and ask for its overview first. That request failed and was never repeated. Reproduced by slowing start-up the same way; fixed by registering the state before any window is made, and the card and the mini timer now retry a failed first load (a quarter of a second, then longer, up to every 4 seconds). The same change makes the mini timer use its remembered spot when Niwa starts with Windows.

**Categories, start confirmation and ending early (Oct 5, 2026)**

- Blocks screen: the "Ready-made" heading is now "Categories", and games are one of them. A single "Games" switch ("Steam, Epic, Riot", with how many were found) turns all three launchers' games on or off together, with "Also close game launchers" under it (off unless Games is on). The separate Games section and the "Battle.net · EA · Ubisoft · soon" row are gone. Lists still store Steam, Epic and Riot apart, so older lists and the service are unchanged; a list with only some of them shows Games on. Chips and the list summary say "Games".
- "Plant & start" asks first: a sheet titled "Plant a pine for 50 minutes?" says what the list blocks, that Strict keeps blocking if Niwa is closed or the PC restarts, how long ending early takes, and that open browsers restart after a minute when the list blocks websites. "Not yet" or "Plant & start".
- Gentle sessions end early like Strict ones: "Request to end", a confirmation, then 5 minutes with "Cancel and keep growing". Closing the card during any session tucks it into the tray; the tray's Quit stays unavailable during a session. Quitting Niwa some other way (Windows shutting down) still ends a Gentle session.
- Tested Oct 5, 2026 at 60× speed through the real card commands: Gentle waited 5 minutes, cancelled and asked again, and withered; a 60-minute Strict session (on a test copy of the service) waited 20 minutes and withered.

**Stats day totals, launchers, less text (Oct 5, 2026)**

- Stats: each bar of the week is a button. Picking one replaces "This week" with that day's date ("Monday, Oct 5") and the week's total with the day's; picking it again goes back to the week. Days still to come can't be picked. A fourth tile, "in total", shows every hour Niwa has spent blocking across all finished sessions, counted like the week (grown in full, withered for the minutes it ran, never past its plan).
- Game launchers (Steam, the Epic Games Launcher, the Riot Client) are never closed, and the "Also close game launchers" switch is gone, along with the setting behind it. Allow-only lists leave them open too: Steam's own programs (steam.exe and its `bin` folder, where its window is drawn) and the Epic and Riot launcher folders. Their games are still closed.
- Removed as unneeded: the note under Websites ("Blocked in every browser on this PC…" and its "needs the Niwa service" variant; the rows still say "needs setup"), "Keeps blocking if Niwa is closed" from the Blocks note and the start confirmation, and the hint in the "Block a website" sheet. The Blocks note is now just how long ending early takes (with "Closes every app with a window that isn't allowed." first for allow-only lists).

**Start-menu launch and the auto-updater (Oct 6, 2026, update design awaiting approval)**

- The card tucks into the mini timer when another program comes to the front, but no longer when Windows' own shell does (taskbar, Start, Search, the tray overflow, Alt+Tab). Opening Niwa from Start could hand the front to Search or the taskbar just after the card took it, which tucked the card away at once; right-clicking the card's taskbar button tucked it away before "Close window" reached it. File Explorer and the desktop still count as leaving.
- Updates (the user approved this exception to "no network calls"): once a day, unless Settings > "Check for updates" is off, Niwa reads `latest.json` from the address in `plugins.updater.endpoints` (`tauri.conf.json`). Since Oct 22, 2026 (the user chose GitHub Releases over a website) the address is the latest release of the public `bento-v/niwa` repository: `https://github.com/bento-v/niwa/releases/latest/download/latest.json`. Each release holds its installer and `latest.json`, which names that release's own installer (`releases/download/v{version}/`), never the "latest" one, so a signature always matches its file. With no address, Niwa never checks. Debug builds never check unless `NIWA_UPDATE_FEED` is set.
- A newer version shows an "Update" pill in the card's header, only outside a session and off the result screen. It opens "Update to Niwa 0.3.0?": "Niwa closes while it updates and opens again after." with Later and Update. Update downloads the installer and checks it, then hands it to the Strict service, which installs it with no admin prompt (below). Without the service, or with one older than PROTOCOL 6, the card runs the installer itself in passive mode (a progress bar, no questions) and the sheet adds "Windows asks to allow the change.", since the installer replaces the Strict service. The installer still refuses during a Strict session.
- Updates without admin (Oct 6, 2026, the user's request after asking why Niwa prompts and other apps don't): only the first install asks for admin. After that the card sends `Request::InstallUpdate` with the downloaded file, its signature and version. The service accepts it only from the card in a login session and outside a session, reads the file into memory, runs `niwa_core::update::check_installer` (Niwa's key, exactly that version, newer than the service), copies it into `C:\ProgramData\Niwa\updates` (SYSTEM-only), and runs that copy with `/S /UPDATE`. The card then quits; the installer replaces the app and the service, and the new service reads `reopen-card.txt` (the requesting account, under 15 minutes old) and reopens the card on that user's screen. If the installer ends before replacing the service, the old service reopens the card itself. Tested Oct 6 with a console test service and a hidden debug card: the update was checked and saved byte for byte (the console engine never runs installers), the no-service route still worked, and requests from programs other than the card were refused. Not yet tested: a real silent install by the installed service, which needs the user to install this build first and then a newer one.
- Checks: the installer must be signed with Niwa's update key, and the signature must name the version `latest.json` announces (`requireSignedVersion`, so an old release can't be passed off as new). The private key is `~/.tauri/niwa-updater.key` on the build PC, never in the repo; losing it means installed copies can't take updates until they're reinstalled by hand. Tested Oct 6 with a hidden debug copy and a local feed: a signed file passed, a tampered one was refused ("The update didn't pass Niwa's safety check, so it wasn't installed.").
- Releasing: bump the version in `tauri.conf.json` (and `Cargo.toml`, `package.json`), run `npm run installer`, then upload `Niwa_<version>_x64-setup.exe` and `latest.json` from `target/release/bundle/nsis/` to the update folder, installer first. `NIWA_RELEASE_NOTES` fills in the notes.
- The updater adds about 1 MB to the installer (HTTPS support).

**Website blocking removed (Oct 6, 2026)**

- The user asked to remove website blocking entirely, after naming the browser restarts and the browser's own error page as what bothered them. A browser extension (instant blocking, a Niwa page in place of the error, no restarts) was offered as the rework and not chosen for now.
- Gone: the Websites section and the "Block a website" sheet on the Blocks screen, the browser-restart notice on the Focus screen and in the start confirmation, the browser restart (`browsers.rs`), the site catalog (`sites.rs`), `normalize_site` (`niwa-core/src/site.rs`), `Request::Sites` and the service's hosts-file block.
- Lists saved with websites still load: the old `siteCategories` and `sites` fields are ignored, so a list that blocked only websites now blocks nothing until something else is switched on. `PROTOCOL` stays 5: cards never ask for websites now, and a Strict plan without them means the same to any service.
- Cleanup: the service takes out any `# BEGIN Niwa` block an earlier version left in the hosts file when it starts, and `niwa-service uninstall` does too. Nothing else in the file is touched. This runs once the user installs a build with the change; until then the old service is still the one running.

**Installing over a running Niwa (Oct 6, 2026)**

- The installer closes a running Niwa by asking it (`niwa-app.exe --quit`, which quits as the tray's Quit does and, like it, not during a session), waits up to 5 seconds, and reopens it as the user once the files and the service are in (not for silent installs: the service reopens the card after its own updates). If Niwa is still open, Tauri's own "Niwa is running" check asks as before; that's also what happens when updating from a version without `--quit`.
- `--quit` with no Niwa running exits before anything starts, judged by the single-instance plugin's window (`app.niwa.desktop-sic`), not the process list, which can still list a process that has exited. The installer runs it as admin, and an admin WebView2 would leave files in the user's WebView2 folder.
- The card no longer tucks into the mini timer when Niwa's installer or uninstaller comes to the front (product name "Niwa" in the version resource): clicking the installer's Finish after it opened Niwa did that.
- Tested Oct 6 on a hidden desktop with the release build: `--quit` alone exited in 0.2 s without creating a WebView2 folder; a running Niwa quit 0.13 s after being asked. The installer hooks themselves need a real install to test.
- The card came back as a sliver (Oct 9, 2026): when Windows minimises the card (clicking its taskbar button while it's in front, Win+D), Niwa tucks it away, and a minimised window that's hidden and shown again returns at Windows' minimised size and spot, 160 by 28 pixels in a corner. So "it opens to the mini timer and the big card doesn't show". Found on the user's PC (the card hidden and minimised) and replayed on a hidden desktop. Now the card notes where it is and how big whenever it's moved or resized as a whole card, and `set_shown` restores and puts it back there before showing it (or at full size in the middle if it was never seen whole). Replayed after the fix: the card reopened at 416 by 666 where it was, and drew its page.
- The see-through card (Oct 9, 2026): the user's card kept opening as an empty, transparent window. Pages are told to stop drawing when hidden (`set_shown`); hide and show requests come from different threads (the watcher tucks the card from its own) and reach WebView2 later on the main thread, so a stale "stop drawing" could land after a show. `sync_page` now applies whatever the window is meant to be when it runs, tells WebView2 where its window is so it paints at once, and runs again 400 ms after a show and whenever a Niwa window gets focus. Not reproducible on a hidden desktop (the debug copy redrew every time); confirmed only by the user's report.
- Settling (Oct 9, 2026): for 2 seconds after the card appears, and while Niwa's installer has the front, another program taking the front isn't counted as leaving. After installing, Windows passed the front from the closing installer to the window behind it, which tucked the new card away. The installer no longer reopens Niwa itself; its finish page's "Run Niwa" does.

**Schedules (Oct 17, 2026, design awaiting approval)**

- Focus times that start a session by themselves (scheduled sessions were once a planned paid extra; now everyone has them). Each has its own days and a start and end time; there can be several, each switched on or off. A time that ends at or before its start ends the next day, and belongs to the day it starts.
- The Focus tab gains a "Schedule" row under the block list: "Next: Today 9:00 AM" (or "Tomorrow", or the weekday), "All off", or "No focus times". It opens a "Schedule" sheet listing the focus times ("9:00 AM – 12:00 PM", "Weekdays") with a switch each, and "+ Focus time" and Done along the bottom (as "+ App" and Done in Always allowed). A focus time opens "Focus time" (or "New focus time"): seven day buttons M–S, From and Until (the system's time fields, 12- or 24-hour as Windows shows), the length ("3 hours", "…, ending the next day"), and Delete (or Cancel) and Save. Styled to match the rest of the card (Oct 17): the list as the Settings sheet's, the day buttons as the duration buttons, From and Until as eyebrow labels, the time fields in Niwa's colours, Delete an ordinary button. Saving refuses no days, under 10 minutes, or over 12 hours.
- When a window comes round with no session running, Niwa starts one with the block list (Gentle or Strict as the list is set) for the rest of the window, up to 12 hours (sessions started by hand stay at 5). The seed is the one last planted if it's unlocked and fits, or else the rarest that fits. A notification comes a minute before ("Focus time at 9:00 AM"); if it can't start, another says why. The card comes up, and the Focus screen reads "Deep work · Gentle · Scheduled".
- Joining late: a window still on when Niwa next runs (or when a session started by hand ends) starts for the time left, if that's at least 10 minutes. Each window runs once (`handled_until`), so a session ended early stays ended. A window already going on when a focus time is saved or changed is skipped: saving never starts blocking, or locks in a Strict session, there and then.
- With the Strict service installed, the card shares its schedules (`Request::Schedules`, PROTOCOL 7); when a window comes round with Niwa closed, the service opens it (once per window) for the account that set them, and the card starts the session. Opening `niwa-app.exe` beside it is the one program the service may launch anyway.
- Tested Oct 17: rules and storage by unit tests (windows past midnight, days off, validation, the database upgrade); a hidden debug copy with a harmless list started a scheduled 17-minute session on its own two minutes after the focus time was saved. Not yet tested: the service opening a closed Niwa, and Strict scheduled sessions, which need this build installed.
