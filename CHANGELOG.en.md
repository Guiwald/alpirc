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
- Release: `cargo xtask` (versions, releases page, icons, `.app`/`.dmg`, `.apk` without Gradle, `.tar.gz`, `.deb`
  for the app and the relay, licenses), continuous integration on GitHub (Linux tests; macOS, Android and Linux
  builds) and the "Publication" workflow that builds a version's files and the signed APT repository.
