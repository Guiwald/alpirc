# Changelog (English)

All notable changes to AlpIRC, in English. The French [CHANGELOG.md](CHANGELOG.md) is the reference; this file
follows it section by section, with the same version headings, so that release notes can quote both. Format
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), [SemVer](https://semver.org/) numbering adapted to the
project (see `docs/versions.md`). Each entry starts with the part of the app it concerns ("Scripts: …").

## [Unreleased]

## [0.16.0] - 2026-10-19

> Rollout: nothing to do, install over 0.15.0 (settings are kept).

### Added
- Channel Central: the topic offers the list of the channel's last topics (the last 10 seen), as in mIRC, to put
  an old one back.
- Commands: `/join -i` joins the last channel you were invited to, as in mIRC.

## [0.15.0] - 2026-10-19

> Rollout: nothing to do, install over 0.14.0 (settings are kept).

### Added
- Interface: mIRC's dialog keys: Alt+L channel list, Alt+N Notify, Alt+U URLs, Alt+S DCC send, Alt+C DCC chat,
  Alt+E server choice, Alt+A favorites, Alt+X maximize or restore the window, Alt+1 to 9 to go to a window (Linux;
  on the Mac, Option types characters).

### Fixed
- Interface: double-clicking in a channel's text opens Channel Central, as in mIRC; in Status it runs `/lusers`;
  in a private window, `/whois`.
- Interface: Alt+O opened the "Tools" menu instead of the Options when the language gives the letter O to a menu;
  shortcuts now come before the menu bar's underlined letters, as in mIRC.

## [0.14.0] - 2026-10-19

> Rollout: nothing to do, install over 0.13.0 (settings are kept).

### Added
- Interface: a right-click menu (long press on the phone) in the channel list (join, add to favorites, copy the
  name, list again) and in the URL catcher (open, copy, remove, clear all), as in mIRC.
- Interface: "Save Buffer..." in a window's menu (mIRC's Buffer › Save): all the window's text, without colors,
  into a file; on Android, into the received files folder.

## [0.13.0] - 2026-10-19

> Rollout: nothing to do, install over 0.12.0 (settings are kept).

### Added
- Missing mIRC commands: `/anick` (alternate nickname), `/mnick` (main nickname, without changing the current
  one), `/tnick` (nickname for this time), `/fullname` (full name), `/ajinvite on|off`, `/creq` and `/sreq` (what
  to do with DCC requests), `/remote on|off`, `/toolbar`, `/switchbar` and `/treebar on|off`, `/qmsg` and `/qme`
  (in every private window), `/clipboard`, `/savebuf` (a window's text into a file of the scripts folder).
- Commands: `/aop`, `/avoice` and `/protect`, as in mIRC (add, `-r` remove, `-l` list, `-w` any network, mask type,
  `on`/`off`); the Address Book has "Active lists" boxes in the Control tab.
- Commands: `/dns host` or `/dns nick` gives the IP address (it used to answer "not supported").
- Commands: `/debug @window` (or a file of the logs folder) shows the lines exchanged with the server, passwords
  hidden; `/debug off` stops it.

## [0.12.0] - 2026-10-19

> Rollout: nothing to do, install over 0.11.0 (settings are kept). On the Mac and Linux, the Channels Folder now
> opens on every connection, as in mIRC: uncheck "Pop up on connect" in the folder (Favorites › Channels Folder, Alt+J) if
> you don't want it.

### Added
- Interface: the menu bar's **Commands** menu, like mIRC's (it was empty): join or part a channel, query, send a
  notice, whois, CTCP, away, invite, ban, kick, ignore or unignore someone, your user modes, change nick, quit
  IRC.
- Interface: the Channels Folder opens on connect, as in mIRC (not on automatic reconnections). The "Pop up on
  connect" box, in the folder, stops it. On the phone, it is unchecked.
- Docs: `docs/ecarts-mirc.md`, the inventory of what still differs from mIRC 6.x, with the open questions.

### Changed
- Scripts: the default menus follow mIRC's: "Ignore" and the new "Unignore" at the top of the nick list's Control
  submenu; a CTCP submenu in a private window's menu. Menus you haven't modified are updated automatically.
- Android: the service notification counts each network once ("Connected to Undernet (2 connections)" instead of
  "Connected to 2 networks").

## [0.11.0] - 2026-10-19

> Rollout: nothing to do, install over 0.10.0 (settings are kept). Mac: open the `.dmg` and replace AlpIRC in
> Applications. Android: install the `.apk` over the old one. Linux: `sudo apt update && sudo apt upgrade`. If you
> had modified the default menus (`popups.mrc`, Scripts Editor › Popups), they are left alone and "Info" still runs
> `/whois` there: replace `whois $$1` with `uwho $$1` on the "Info" lines to get User Central.

### Added
- Interface: the **User Central** dialog, as in mIRC. "Info" in a nickname's menu (nick list, private window) or
  `/uwho nick` shows the person's whois (address, name, server, channels, account, idle time, sign-on time, away
  message) without writing anything in the windows, and what your Address Book keeps about them: real name,
  e-mail, website, notes. Update, Copy all and Query buttons.
- Address Book: an **Info** tab, the list of people and their notes (the same as in User Central).

### Changed
- Android: "Whois in active window" (Options › IRC) is checked by default, so a whois reply shows in the window
  you are in instead of Status. On a phone where AlpIRC is already installed, it is checked once by the update; if
  you uncheck it afterwards, it stays unchecked. Mac and Linux: no change (unchecked, as in mIRC).

### Fixed
- Scripts: right-click menus stayed in the language of the first run when you changed language (welcome dialog,
  Options). The ones you haven't modified now follow the interface language, and pick up the version's changes
  ("Info" opens User Central).

## [0.10.0] - 2026-10-18

> Rollout: nothing special to do, install over 0.9.0 (settings are kept). Mac: open the `.dmg` and replace AlpIRC
> in Applications. Android: install the `.apk` over the old one. Linux: `sudo apt update && sudo apt upgrade` (or
> the new `.deb`). This is the version to try on the Pixel 8: the defects found in the Android emulator are fixed.

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
