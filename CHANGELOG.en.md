# Changelog (English)

All notable changes to AlpIRC, in English. The French [CHANGELOG.md](CHANGELOG.md) is the reference; this file
follows it section by section, with the same version headings, so that release notes can quote both. Format
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), [SemVer](https://semver.org/) numbering adapted to the
project (see `docs/versions.md`). Each entry starts with the part of the app it concerns ("Scripts: …").

## [Unreleased]

### Added
- Interface: View › Theme also lists your own themes; Alt+letter picks a tab in dialogs; a window's full name
  shows in a tooltip when its switchbar button is shortened.
- Sounds: `/splay stop` stops the current sound (Mac, Linux, Android).
- Android: touching the notification of a mention or private message opens that conversation's window.

### Changed
- Languages: connection, certificate and DCC errors and script messages follow the interface language; on first
  run, the full name and away message are created in your language; on the Mac, the app menu says "Quit AlpIRC".
- Interface: every underlined shortcut is unique within a dialog or menu, in all 7 languages; Larger and Smaller
  Font follow the sizes of the Options dialog; dialogs stay inside the window when it shrinks; switchbar buttons
  take the width of their name.

### Fixed
- Android: the keyboard opens when you touch a field in a dialog, even after Back or when the notification
  permission request prevented it on first run (welcome dialog); in landscape, the interface no longer goes under
  the camera cutout; an open drop-down list follows its dialog when the keyboard closes; the Send and Tab buttons
  change language with the interface; "1 user" in the singular in the nick list.
- Interface: the channel list and the URL catcher update even when the number of rows doesn't change; the last
  line of About is no longer cut off.
- Release: the APT site archive is no longer attached to a version's files.

## [0.9.0] - 2026-10-18

> Rollout: first published version, install as is. Mac: open the `.dmg`, drag AlpIRC into Applications; on first
> launch, System Settings › Privacy & Security › "Open Anyway" (the app isn't signed by Apple, see the README).
> Android: install the `.apk` (allow installing from your browser or file manager). Linux:
> `sudo apt install ./alpirc_0.9.0_amd64.deb`, which also adds the APT repository for updates. Trial version:
> everything planned for 1.0 is there; 1.0 ships after the trials on a Mac and a phone.

### Added
- Foundation: the Rust workspace (protocol, core, network, scripts, interface, relay, desktop and Android apps)
  and the founding decisions (`docs/adr`), including the name, AlpIRC, and the scope of 1.0.
- Protocol: reading and writing IRC lines (RFC 1459, RFC 2812) with IRCv3 tags, `ISUPPORT`, modes, numerics,
  masks, CTCP, mIRC colour codes, UTF-8 with a Windows-1252 fallback, and SASL (PLAIN, EXTERNAL, SCRAM-SHA-256).
- Scripts: the mSL engine (`alpirc-script`), sandboxed: aliases, popups, remotes (`on TEXT`, `ctcp`, `raw`…),
  groups, `%variables`, `$identifiers`, `if`/`while`/`goto`, timers, hash tables, files limited to the scripts
  folder, user levels and custom dialogs; no `/run`, `$dll` or `$com`.
- Core: the client engine (`alpirc-core`): several networks at once, TLS with full verification, pinning or
  trusting a certificate, SASL, NickServ, Undernet's X account (login on connect with `+x`), STS, reconnection,
  IRCv3 (server-time, echo-message, account-notify, away-notify, chghost, extended-join, multi-prefix, MONITOR,
  labeled-response, batch…), soju-compatible relays and bouncers (bouncer-networks, chathistory, read-marker).
- Commands: mIRC's commands (`/join`, `/msg`, `/query`, `/me`, `/whois`, `/away`, `/ctcp`, `/mode`, `/kick`,
  `/ban`, `/ignore`, `/notify`, `/list`, `/server`, `/raw`, `/window`, `/closemsg`, `/timestamp`, `/splay`,
  `/sound`, `/help`…), history, Tab completion (nicks, channels, commands, aliases), Ctrl+K/B/U/I/R/O, the paste
  guard, `$?` and `$input` (input box).
- Interface: the mIRC-style window (`alpirc-ui`, `alpirc-shell`): MDI windows, switchbar on top, optional
  treebar, toolbar, menus (native on the Mac), script popups, Classic and Dark themes plus your own themes, 16 + 99
  colours, highlights, nick colours, font choice (Fixedsys Excelsior by default), original pixel-art icons.
- Dialogs: Options (every page, including Sounds and DCC), network list and editor, address book (Notify,
  Ignore, Control), channels folder, Channel Central, script editor (Alt+R), colours, channel list with filters,
  logs, certificate prompt, first-run welcome.
- Windows: Notify (live), URL catcher, channel list, conversation logs, auto-away.
- DCC: CHAT, SEND/GET with resume and passive DCC, transfers window, drag and drop of a file.
- Sounds: event sounds (highlight, private message, Notify, DCC, connection, kick, invite…), original built-in
  sounds, `/splay`, CTCP SOUND requests (refused by default), and system notifications.
- Languages: the interface in English, French, Spanish, German, Italian, Valencian and Irish; localised dates.
- Mac: `irc://` and `ircs://` links, notifications under AlpIRC's name, the Option key (`|`, `{`, `[`…),
  trackpad scrolling.
- Linux: a single AlpIRC window (an `irc://` link is handed to the one already running), sounds.
- Android: the phone layout (windows and nicks drawers, touch menus, gestures), native input with its history,
  the permanent connection (service with a notification), disconnecting after N minutes in the background (off by
  default), notifications, sounds, the file picker for DCC.
- Relay: `alpirc-relay`, AlpIRC's relay (ADR 0014): the same IRC account on the phone and the computer, history
  on disk, read markers, pinned TLS, argon2id passwords, a `.deb` package with systemd.
- Release: `cargo xtask` (versions, releases page, icons, `.app`/`.dmg`, `.apk` without Gradle, `.tar.gz`, `.deb`
  for the app and the relay, licenses), continuous integration on GitHub (Linux tests; macOS, Android and Linux
  builds) and the "Publication" workflow that builds a version's files and the signed APT repository.
