//! Capture de l'application sans écran : une connexion simulée à Undernet, #lyon, quelques messages, puis
//! dessin dans un PNG. `cargo run -p alpirc-shell --example screenshot -- /tmp/alpirc.png [échelle] [sombre]`

use std::path::Path;

use alpirc_net::{Net, NetEvent};
use alpirc_shell::{LayoutKind, Shell, ShellConfig, ShellPlatform};
use alpirc_ui::{Canvas, Cursor, Input, Platform, Rect, Size};

struct P {
    clip: Option<String>,
    now: i64,
}

impl Platform for P {
    fn clipboard_get(&mut self) -> Option<String> {
        self.clip.clone()
    }
    fn clipboard_set(&mut self, text: &str) {
        self.clip = Some(text.into());
    }
    fn now_ms(&self) -> i64 {
        self.now
    }
    fn set_cursor(&mut self, _c: Cursor) {}
    fn set_ime_area(&mut self, _rect: Rect) {}
    fn request_repaint(&mut self) {}
}

impl ShellPlatform for P {
    fn notify(&mut self, title: &str, body: &str) {
        println!("notification : {title} — {body}");
    }
    fn beep(&mut self) {}
    fn play_sound(&mut self, _path: &Path) {}
    fn flash(&mut self) {}
    fn open_url(&mut self, url: &str) {
        println!("ouvrir {url}");
    }
    fn set_title(&mut self, title: &str) {
        println!("titre : {title}");
    }
    fn exit(&mut self) {}
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = args.get(1).cloned().unwrap_or_else(|| "/tmp/alpirc.png".into());
    let scale: f32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1.0);
    let dark = args.get(3).is_some_and(|s| s == "dark");
    let dir = std::env::temp_dir().join(format!("alpirc-shot-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut p = P { clip: None, now: 1_792_260_000_000 };
    let net = Net::new(Box::new(|_e: NetEvent| {}));
    let cfg = ShellConfig {
        settings_path: dir.join("settings.toml"),
        data_dir: dir.clone(),
        layout: LayoutKind::Desktop,
        utc_offset: 7200,
        locale: Some("fr-FR".into()),
        now_ms: p.now,
    };
    let mut shell = Shell::new(cfg, net);
    if dark {
        shell.client.settings_mut().display.theme = "dark".into();
        shell.settings_changed(&mut p);
    }
    shell.set_viewport(Size::new(1024, 700));
    shell.start(&mut p);
    let status = shell.client.windows()[0].id;
    shell.client.connect(status, "Undernet", None, p.now);
    shell.tick(&mut p);
    let conn = shell.client.conns()[0];
    shell.client.on_connected(conn, p.now);
    let lines = [
        ":irc.undernet.org 001 Guiwald :Welcome to the UnderNet IRC Network, Guiwald",
        ":irc.undernet.org 005 Guiwald NETWORK=UnderNet PREFIX=(ov)@+ CHANMODES=b,k,l,imnpstrDd :are supported by this server",
        ":irc.undernet.org 375 Guiwald :- irc.undernet.org Message of the Day -",
        ":irc.undernet.org 372 Guiwald :- Bienvenue sur Undernet !",
        ":irc.undernet.org 376 Guiwald :End of /MOTD command.",
        ":Guiwald!~alpirc@lyon.wanadoo.fr JOIN #lyon",
        ":irc.undernet.org 332 Guiwald #lyon :Bienvenue sur #lyon ! \x02Soirée IRL\x02 vendredi place des Terreaux",
        ":irc.undernet.org 333 Guiwald #lyon Kaelis 1792250000",
        ":irc.undernet.org 353 Guiwald = #lyon :@Guiwald @Kaelis +Elisa Bibi Marmotte Pepito Zaza @Nemo Tomtom",
        ":irc.undernet.org 366 Guiwald #lyon :End of /NAMES list.",
        ":Kaelis!kae@lyon.fr PRIVMSG #lyon :salut Guiwald :)",
        ":Elisa!~elisa@ppp-42.lyon.wanadoo.fr PRIVMSG #lyon :coucou ! quelqu'un a des nouvelles de Tomtom ?",
        ":Nemo!nemo@grenoble.fr PRIVMSG #lyon :\x01ACTION boit un café ☕\x01",
        ":Bibi!bibi@chambe.fr PRIVMSG #lyon :\x02\x034,1 ROUGE SUR NOIR \x03\x02 comme au bon vieux temps",
        ":Pepito!pepito@82-64-12-7.subs.proxad.net JOIN #lyon",
        ":Pepito!pepito@82-64-12-7.subs.proxad.net PRIVMSG #lyon :quelqu'un connait un bon client IRC pour Mac ?",
        ":Nemo!nemo@grenoble.fr PRIVMSG #lyon :Guiwald en code un justement : https://github.com/Guiwald/alpirc",
        ":Nemo!nemo@grenoble.fr MODE #lyon +v Pepito",
        ":Zaza!zaza@lyon.fr PART #lyon :bonne nuit !",
    ];
    for l in lines {
        p.now += 1000;
        shell.on_net(NetEvent::Line { conn, bytes: l.as_bytes().to_vec() }, &mut p);
    }
    shell.client.input(shell.client.active().unwrap(), "lu tout le monde ! \x02AlpIRC\x02 tourne enfin", p.now);
    shell.tick(&mut p);
    shell.input(Input::Text("j'arrive dans 5 min".into()), &mut p);
    let vp = shell.viewport();
    let mut canvas = Canvas::new((vp.w as f32 * scale) as u32, (vp.h as f32 * scale) as u32, scale);
    shell.paint(&mut canvas);
    canvas.to_png(&out).unwrap();
    println!("{out}");
}
