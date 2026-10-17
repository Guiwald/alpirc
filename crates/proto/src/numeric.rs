//! Numériques IRC utiles (RFC 1459, RFC 2812, IRCv3, extensions courantes).

pub const RPL_WELCOME: &str = "001";
pub const RPL_YOURHOST: &str = "002";
pub const RPL_CREATED: &str = "003";
pub const RPL_MYINFO: &str = "004";
pub const RPL_ISUPPORT: &str = "005";
pub const RPL_BOUNCE: &str = "010";
pub const RPL_UMODEIS: &str = "221";
pub const RPL_LUSERCLIENT: &str = "251";
pub const RPL_AWAY: &str = "301";
pub const RPL_USERHOST: &str = "302";
pub const RPL_ISON: &str = "303";
pub const RPL_UNAWAY: &str = "305";
pub const RPL_NOWAWAY: &str = "306";
pub const RPL_WHOISREGNICK: &str = "307";
pub const RPL_WHOISUSER: &str = "311";
pub const RPL_WHOISSERVER: &str = "312";
pub const RPL_WHOISOPERATOR: &str = "313";
pub const RPL_WHOWASUSER: &str = "314";
pub const RPL_ENDOFWHO: &str = "315";
pub const RPL_WHOISIDLE: &str = "317";
pub const RPL_ENDOFWHOIS: &str = "318";
pub const RPL_WHOISCHANNELS: &str = "319";
pub const RPL_WHOISSPECIAL: &str = "320";
pub const RPL_LISTSTART: &str = "321";
pub const RPL_LIST: &str = "322";
pub const RPL_LISTEND: &str = "323";
pub const RPL_CHANNELMODEIS: &str = "324";
pub const RPL_CREATIONTIME: &str = "329";
pub const RPL_WHOISACCOUNT: &str = "330";
pub const RPL_NOTOPIC: &str = "331";
pub const RPL_TOPIC: &str = "332";
pub const RPL_TOPICWHOTIME: &str = "333";
pub const RPL_WHOISACTUALLY: &str = "338";
pub const RPL_INVITING: &str = "341";
pub const RPL_INVITELIST: &str = "346";
pub const RPL_ENDOFINVITELIST: &str = "347";
pub const RPL_EXCEPTLIST: &str = "348";
pub const RPL_ENDOFEXCEPTLIST: &str = "349";
pub const RPL_VERSION: &str = "351";
pub const RPL_WHOREPLY: &str = "352";
pub const RPL_NAMREPLY: &str = "353";
pub const RPL_WHOSPCRPL: &str = "354";
pub const RPL_ENDOFNAMES: &str = "366";
pub const RPL_BANLIST: &str = "367";
pub const RPL_ENDOFBANLIST: &str = "368";
pub const RPL_ENDOFWHOWAS: &str = "369";
pub const RPL_MOTD: &str = "372";
pub const RPL_MOTDSTART: &str = "375";
pub const RPL_ENDOFMOTD: &str = "376";
pub const RPL_WHOISHOST: &str = "378";
pub const RPL_WHOISMODES: &str = "379";
pub const RPL_YOUREOPER: &str = "381";
pub const RPL_TIME: &str = "391";
pub const RPL_HOSTHIDDEN: &str = "396";
pub const ERR_NOSUCHNICK: &str = "401";
pub const ERR_NOSUCHSERVER: &str = "402";
pub const ERR_NOSUCHCHANNEL: &str = "403";
pub const ERR_CANNOTSENDTOCHAN: &str = "404";
pub const ERR_TOOMANYCHANNELS: &str = "405";
pub const ERR_WASNOSUCHNICK: &str = "406";
pub const ERR_UNKNOWNCOMMAND: &str = "421";
pub const ERR_NOMOTD: &str = "422";
pub const ERR_ERRONEUSNICKNAME: &str = "432";
pub const ERR_NICKNAMEINUSE: &str = "433";
pub const ERR_NICKCOLLISION: &str = "436";
pub const ERR_UNAVAILRESOURCE: &str = "437";
pub const ERR_USERNOTINCHANNEL: &str = "441";
pub const ERR_NOTONCHANNEL: &str = "442";
pub const ERR_NOTREGISTERED: &str = "451";
pub const ERR_NEEDMOREPARAMS: &str = "461";
pub const ERR_PASSWDMISMATCH: &str = "464";
pub const ERR_YOUREBANNEDCREEP: &str = "465";
pub const ERR_CHANNELISFULL: &str = "471";
pub const ERR_UNKNOWNMODE: &str = "472";
pub const ERR_INVITEONLYCHAN: &str = "473";
pub const ERR_BANNEDFROMCHAN: &str = "474";
pub const ERR_BADCHANNELKEY: &str = "475";
pub const ERR_NEEDREGGEDNICK: &str = "477";
pub const ERR_CHANOPRIVSNEEDED: &str = "482";
pub const RPL_WHOISSECURE: &str = "671";
pub const RPL_MONONLINE: &str = "730";
pub const RPL_MONOFFLINE: &str = "731";
pub const RPL_MONLIST: &str = "732";
pub const RPL_ENDOFMONLIST: &str = "733";
pub const ERR_MONLISTFULL: &str = "734";
pub const RPL_LOGGEDIN: &str = "900";
pub const RPL_LOGGEDOUT: &str = "901";
pub const ERR_NICKLOCKED: &str = "902";
pub const RPL_SASLSUCCESS: &str = "903";
pub const ERR_SASLFAIL: &str = "904";
pub const ERR_SASLTOOLONG: &str = "905";
pub const ERR_SASLABORTED: &str = "906";
pub const ERR_SASLALREADY: &str = "907";
pub const RPL_SASLMECHS: &str = "908";

/// Vrai pour une réponse numérique (3 chiffres).
pub fn is_numeric(cmd: &str) -> bool {
    cmd.len() == 3 && cmd.bytes().all(|b| b.is_ascii_digit())
}

/// Vrai pour un numérique d'erreur (400 à 599).
pub fn is_error(cmd: &str) -> bool {
    is_numeric(cmd) && matches!(cmd.as_bytes()[0], b'4' | b'5')
}
