//! Hôte factice pour les tests : enregistre les commandes, fournit quelques identifiants, une heure fixe.

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use alpirc_script::{Ctx, Engine, Host, Msg, ScriptKind};

pub struct FakeHost {
    /// (commande, arguments, connexion)
    pub cmds: Vec<(String, String, Option<u32>)>,
    pub errors: Vec<String>,
    pub now: i64,
    pub me: String,
    /// (canal, pseudo) en minuscules → modes
    pub members: HashMap<(String, String), String>,
    pub idents: HashMap<String, String>,
    /// pseudo en minuscules → adresse complète
    pub addresses: HashMap<String, String>,
    pub offset: i32,
    /// Rédaction des messages du moteur (par défaut : l'anglais du moteur).
    pub render: Option<fn(&Msg) -> String>,
}

impl FakeHost {
    pub fn new() -> FakeHost {
        let mut members = HashMap::new();
        for (n, m) in [("guiwald", "o"), ("bob", "o"), ("carl", "v"), ("dave", ""), ("hal", "h")] {
            members.insert(("#test".to_string(), n.to_string()), m.to_string());
        }
        members.insert(("#other".to_string(), "guiwald".to_string()), String::new());
        let mut idents = HashMap::new();
        idents.insert("server".to_string(), "irc.test".to_string());
        let mut addresses = HashMap::new();
        addresses.insert("bob".to_string(), "Bob!bob@op.host".to_string());
        FakeHost {
            cmds: Vec::new(),
            errors: Vec::new(),
            now: 1_700_000_000_000,
            me: "Guiwald".into(),
            members,
            idents,
            addresses,
            offset: 0,
            render: None,
        }
    }

    pub fn clear(&mut self) {
        self.cmds.clear();
        self.errors.clear();
    }

    /// Arguments des `/echo`, sans le `-a ` de tête.
    pub fn echoes(&self) -> Vec<String> {
        self.cmds
            .iter()
            .filter(|(n, _, _)| n == "echo")
            .map(|(_, a, _)| a.strip_prefix("-a ").unwrap_or(a).to_string())
            .collect()
    }

    /// Les `/echo` sans les messages d'information du moteur (`* …`).
    pub fn says(&self) -> Vec<String> {
        self.echoes().into_iter().filter(|e| !e.starts_with("* ")).collect()
    }

    pub fn last(&self) -> (String, String) {
        let (n, a, _) = self.cmds.last().cloned().expect("aucune commande");
        (n, a)
    }

    pub fn names(&self) -> Vec<String> {
        self.cmds.iter().map(|(n, _, _)| n.clone()).collect()
    }

    pub fn find(&self, name: &str) -> Vec<String> {
        self.cmds.iter().filter(|(n, _, _)| n == name).map(|(_, a, _)| a.clone()).collect()
    }
}

impl Host for FakeHost {
    fn command(&mut self, ctx: &Ctx, name: &str, args: &str) -> Result<(), String> {
        if name == "fail" {
            return Err(format!("/{name}: insufficient parameters"));
        }
        self.cmds.push((name.to_string(), args.to_string(), ctx.conn));
        Ok(())
    }

    fn command_quiet(&mut self, ctx: &Ctx, name: &str, args: &str) -> Result<(), String> {
        self.cmds.push((format!("quiet:{name}"), args.to_string(), ctx.conn));
        Ok(())
    }

    fn identifier(&mut self, ctx: &Ctx, name: &str, args: &[String], _prop: Option<&str>) -> Option<String> {
        match name {
            "me" => Some(self.me.clone()),
            "?" => Some("answer".into()),
            "address" if !args.is_empty() => self.addresses.get(&args[0].to_lowercase()).cloned(),
            "scon" => match args.first().map(|s| s.as_str()) {
                Some("0") => Some("2".into()),
                Some("1") => Some("1".into()),
                Some("2") => Some("5".into()),
                _ => None,
            },
            "isignore" => Some(if args.first().is_some_and(|a| a.eq_ignore_ascii_case("troll")) { "$true".into() } else { "$false".into() }),
            "conncheck" => Some(ctx.conn.map(|c| c.to_string()).unwrap_or_default()),
            _ => self.idents.get(name).cloned(),
        }
    }

    fn member_modes(&mut self, _ctx: &Ctx, chan: &str, nick: &str) -> Option<String> {
        self.members.get(&(chan.to_lowercase(), nick.to_lowercase())).cloned()
    }

    fn is_channel(&mut self, _ctx: &Ctx, name: &str) -> bool {
        name.starts_with('#')
    }

    fn script_error(&mut self, _ctx: &Ctx, text: &str) {
        self.errors.push(text.to_string());
    }

    fn message_text(&self, msg: &Msg) -> String {
        match self.render {
            Some(f) => f(msg),
            None => msg.to_string(),
        }
    }

    fn now_ms(&self) -> i64 {
        self.now
    }

    fn utc_offset(&self, _unix_secs: i64) -> i32 {
        self.offset
    }
}

static N: AtomicUsize = AtomicUsize::new(0);

/// Dossier temporaire neuf pour un test.
pub fn tmpdir() -> PathBuf {
    let n = N.fetch_add(1, Ordering::SeqCst);
    let d = std::env::temp_dir().join(format!("alpirc-script-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Contexte d'une saisie dans `#test` sur la connexion 1.
pub fn ctx() -> Ctx {
    Ctx { conn: Some(1), window: Some("#test".into()), ..Ctx::default() }
}

pub fn engine() -> Engine {
    Engine::new(tmpdir())
}

/// Moteur avec un remote `test.mrc` chargé (et ses `on LOAD` exécutés).
pub fn setup(script: &str) -> (Engine, FakeHost) {
    let mut e = engine();
    let mut h = FakeHost::new();
    e.load_text("test.mrc", ScriptKind::Remote, script).unwrap();
    e.flush_pending(&mut h);
    (e, h)
}

pub fn ev(e: &mut Engine, h: &mut FakeHost, text: &str) -> String {
    e.evaluate(h, &ctx(), text)
}

pub fn run(e: &mut Engine, h: &mut FakeHost, line: &str) {
    if let Err(err) = e.run_line(h, &ctx(), line) {
        panic!("run_line({line:?}) : {err}");
    }
}

pub fn run_err(e: &mut Engine, h: &mut FakeHost, line: &str) -> String {
    match e.run_line(h, &ctx(), line) {
        Ok(()) => panic!("run_line({line:?}) devait échouer"),
        Err(err) => err.to_string(),
    }
}

/// Valeur d'une condition, par `$iif`.
pub fn cond(e: &mut Engine, h: &mut FakeHost, c: &str) -> bool {
    let v = ev(e, h, &format!("$iif({c},1,0)"));
    assert!(v == "1" || v == "0", "condition {c:?} → {v:?}");
    v == "1"
}
