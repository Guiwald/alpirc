# Changelog (English)

All notable changes to AlpIRC, in English. The French [CHANGELOG.md](CHANGELOG.md) is the reference; this file
follows it section by section, with the same version headings, so that release notes can quote both. Format
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), [SemVer](https://semver.org/) numbering adapted to the
project (see `docs/versions.md`). Each entry starts with the part of the app it concerns ("Scripts: …").

## [Unreleased]

### Added
- Foundation: the Rust workspace (protocol, core, network, scripts, interface, relay, desktop and Android apps)
  and the 13 founding decisions (`docs/adr`), including the name, AlpIRC, and the scope of 1.0.
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
