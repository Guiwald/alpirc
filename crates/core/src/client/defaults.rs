//! Scripts écrits au premier démarrage dans `<données>/scripts` : popups à la manière de mIRC 6 et quelques alias
//! classiques. Ce sont des fichiers « remote » ordinaires : l'utilisateur peut les modifier.

/// `popups.mrc`.
pub const POPUPS: &str = r#"; Menus par défaut d'AlpIRC, à la manière de mIRC 6.
menu status {
  Lusers:lusers
  Motd:motd
  Time:time
  -
  Join channel:join $$?="Enter channel name:"
  -
  Quit:quit
}

menu channel {
  Channel Modes:channel
  Channels Folder:folder
  -
  Part:part #
}

menu query {
  Info:whois $$1
  Whois:whois $$1
  Query:query $$1
  -
  DCC
  .Send:dcc send $$1
  .Chat:dcc chat $$1
  -
  Ignore:ignore $$1 1 | close -m $$1
  Close:close -m $$1
}

menu nicklist {
  Info:whois $$1
  Whois:whois $$1
  Query:query $$1
  -
  Control
  .Op:op $$1-
  .Deop:deop $$1-
  .Voice:voice $$1-
  .Devoice:devoice $$1-
  .-
  .Kick:kick # $$1
  .Kick (why):kick # $$1 $$?="Reason:"
  .Ban:ban # $$1 2
  .Ban, Kick:ban -k # $$1 2
  .Ban, Kick (why):ban -k # $$1 2 $$?="Reason:"
  CTCP
  .Ping:ctcp $$1 ping
  .Time:ctcp $$1 time
  .Version:ctcp $$1 version
  DCC
  .Send:dcc send $$1
  .Chat:dcc chat $$1
  -
  Slap!:me slaps $$1 around a bit with a large trout
  -
  Ignore:ignore $$1 1
}
"#;

/// `aliases.mrc`.
pub const ALIASES: &str = r#"; Alias par défaut d'AlpIRC.
alias j join $1-
alias p part $1-
alias n names $1-
alias w whois $1-
alias k kick $1-
alias q query $1-
alias send dcc send $1-
alias chat dcc chat $1-
alias ping ctcp $1 ping
"#;

/// Fichiers par défaut, dans l'ordre de chargement.
pub const FILES: &[(&str, &str)] = &[("aliases.mrc", ALIASES), ("popups.mrc", POPUPS)];
