//! Journaux des fenêtres : `<dossier>/<réseau>/<#canal ou pseudo>.log`, avec les marques `Session Start:` et
//! `Session Close:` de mIRC. Toutes les écritures passent par [`LogSink`] : les tests peuvent en donner un autre.

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Destination des journaux.
pub trait LogSink: Send {
    /// Ajoute du texte à la fin d'un fichier (créé au besoin, dossiers compris).
    fn append(&mut self, path: &Path, text: &str) -> std::io::Result<()>;
}

/// Écriture directe sur le disque.
#[derive(Debug, Default)]
pub struct FileSink;

impl LogSink for FileSink {
    fn append(&mut self, path: &Path, text: &str) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
        f.write_all(text.as_bytes())
    }
}

/// Nom de fichier sûr : sans séparateurs ni caractères interdits, en minuscules (les canaux ne tiennent pas
/// compte de la casse).
pub fn sanitize(name: &str) -> String {
    let mut s: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect::<String>()
        .to_lowercase();
    while s.starts_with('.') {
        s.replace_range(0..1, "_");
    }
    if s.is_empty() {
        s.push('_');
    }
    s
}

/// Les journaux ouverts pendant cette session.
pub(crate) struct Logger {
    pub sink: Box<dyn LogSink>,
    open: HashSet<PathBuf>,
}

impl Logger {
    pub fn new(sink: Box<dyn LogSink>) -> Logger {
        Logger { sink, open: HashSet::new() }
    }

    pub fn path(folder: &Path, network: &str, window: &str) -> PathBuf {
        folder.join(sanitize(network)).join(format!("{}.log", sanitize(window)))
    }

    /// Écrit une ligne ; la première fois, la marque `Session Start: <date>`.
    pub fn write(&mut self, path: &Path, line: &str, date: &str) {
        let mut text = String::new();
        if !self.open.contains(path) {
            self.open.insert(path.to_path_buf());
            text.push_str(&format!("\nSession Start: {date}\n"));
        }
        text.push_str(line);
        text.push('\n');
        if let Err(e) = self.sink.append(path, &text) {
            log::warn!("journal {} : {e}", path.display());
        }
    }

    /// Ferme un journal ouvert : `Session Close: <date>`.
    pub fn close(&mut self, path: &Path, date: &str) {
        if self.open.remove(path) {
            let _ = self.sink.append(path, &format!("Session Close: {date}\n"));
        }
    }

    pub fn is_open(&self, path: &Path) -> bool {
        self.open.contains(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(sanitize("#Lyon"), "#lyon");
        assert_eq!(sanitize("../etc/passwd"), "_._etc_passwd");
        assert_eq!(sanitize("a:b|c"), "a_b_c");
        assert_eq!(Logger::path(Path::new("/l"), "Undernet", "#Lyon"), PathBuf::from("/l/undernet/#lyon.log"));
    }
}
