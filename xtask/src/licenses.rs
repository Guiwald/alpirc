//! `cargo xtask licenses [paquet…]` : la licence de chaque crate tierce compilée dans AlpIRC (par défaut : appli de
//! bureau, appli Android, relais), en tableau Markdown sur la sortie standard. Lu dans `cargo tree` (le vrai
//! résolveur de fonctions de cargo) : dépendances normales et de compilation, toutes plateformes confondues ; les
//! dépendances de test sont exclues.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::process::Command;

use crate::{Res, workspace_root};

/// Les programmes livrés.
const ROOTS: [&str; 3] = ["alpirc", "alpirc-android", "alpirc-relay"];

pub fn licenses(packages: &[&str]) -> Res {
    let wanted: Vec<&str> = if packages.is_empty() { ROOTS.to_vec() } else { packages.to_vec() };
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut found = Vec::new();
    let mut all = BTreeSet::new();
    for p in &wanted {
        let out = Command::new(&cargo)
            .args(["tree", "-p", p, "-e", "normal,build", "--target", "all", "--prefix", "none", "--format", "{p}|{l}"])
            .current_dir(workspace_root())
            .output()?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            if err.contains("did not match any packages") {
                continue;
            }
            return Err(format!("cargo tree -p {p} a échoué :\n{err}").into());
        }
        found.push(*p);
        all.extend(parse_tree(&String::from_utf8_lossy(&out.stdout)));
    }
    if found.is_empty() {
        return Err(format!("aucun de ces paquets dans l'espace de travail : {}", wanted.join(", ")).into());
    }
    print!("{}", table(&all, &found, &crate::today()));
    Ok(())
}

/// Les crates tierces d'une sortie de `cargo tree --prefix none --format "{p}|{l}"` : (nom, version, licence).
/// Les crates du projet (avec un chemin) sont laissées de côté.
fn parse_tree(text: &str) -> BTreeSet<(String, String, String)> {
    let mut out = BTreeSet::new();
    for line in text.lines() {
        let line = line.trim().trim_end_matches("(*)").trim_end();
        let Some((pkg, license)) = line.split_once('|') else { continue };
        if pkg.contains(" (/") || pkg.contains(" (\\") || pkg.contains(":\\") {
            continue; // crate locale (chemin)
        }
        let mut it = pkg.split_whitespace();
        let (Some(name), Some(version)) = (it.next(), it.next()) else { continue };
        let license = license.trim();
        out.insert((name.to_string(), version.trim_start_matches('v').to_string(), if license.is_empty() { "?".to_string() } else { license.to_string() }));
    }
    out
}

fn table(rows: &BTreeSet<(String, String, String)>, roots: &[&str], date: &str) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "# Licences des crates d'AlpIRC\n");
    let _ = writeln!(
        s,
        "Généré par `cargo xtask licenses` le {date}, pour {} : dépendances normales et de compilation, toutes plateformes confondues.\n",
        roots.iter().map(|r| format!("`{r}`")).collect::<Vec<_>>().join(", ")
    );
    let _ = writeln!(s, "| Crate | Version | Licence |\n|---|---|---|");
    let mut count: BTreeMap<&str, usize> = BTreeMap::new();
    for (name, version, license) in rows {
        let _ = writeln!(s, "| {name} | {version} | {} |", license.replace('|', "\\|"));
        *count.entry(license.as_str()).or_default() += 1;
    }
    if rows.is_empty() {
        let _ = writeln!(s, "\nAucune crate tierce pour l'instant.");
    } else {
        let mut by: Vec<(&&str, &usize)> = count.iter().collect();
        by.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let summary = by.iter().map(|(l, n)| format!("{l} : {n}")).collect::<Vec<_>>().join(" ; ");
        let _ = writeln!(s, "\n{} crates — {summary}", rows.len());
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyse_et_tableau() {
        let tree = "alpirc v0.1.0 (/home/x/alpirc/app/desktop)|MIT\n\
                    alpirc-proto v0.1.0 (/home/x/alpirc/crates/proto)|MIT\n\
                    log v0.4.28|MIT OR Apache-2.0\n\
                    cc v1.2.0|MIT OR Apache-2.0\n\
                    log v0.4.28|MIT OR Apache-2.0 (*)\n\
                    serde_derive v1.0.0 (proc-macro)|MIT OR Apache-2.0\n\
                    weird v2.0.0|\n\
                    \n";
        let rows = parse_tree(tree);
        assert_eq!(rows.len(), 4, "{rows:?}");
        assert!(rows.contains(&("log".into(), "0.4.28".into(), "MIT OR Apache-2.0".into())));
        assert!(rows.contains(&("serde_derive".into(), "1.0.0".into(), "MIT OR Apache-2.0".into())));
        assert!(rows.contains(&("weird".into(), "2.0.0".into(), "?".into())));
        let t = table(&rows, &["alpirc"], "2026-10-17");
        assert!(t.contains("| cc | 1.2.0 | MIT OR Apache-2.0 |"));
        assert!(!t.contains("alpirc-proto"));
        assert!(t.contains("4 crates — MIT OR Apache-2.0 : 3 ; ? : 1"));
        assert!(t.contains("pour `alpirc` :"));
        assert!(table(&BTreeSet::new(), &["alpirc"], "2026-10-17").contains("Aucune crate tierce"));
    }
}
