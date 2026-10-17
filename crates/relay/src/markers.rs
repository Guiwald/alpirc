//! Marques de lecture (`draft/read-marker`) : pour chaque utilisateur, réseau et cible (canal ou privé), l'heure
//! du dernier message lu, partagée entre ses appareils. Gardées dans `<données>/users/<nom>/readmarkers.json`
//! (`{ "<netid>": { "<cible repliée>": "2026-10-17T10:00:00.000Z" } }`).

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use crate::ids::{format_time, parse_time};

type UserMarkers = BTreeMap<String, BTreeMap<String, String>>;

/// Résultat d'un `MARKREAD <cible> timestamp=…`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkResult {
    /// La marque avance : à annoncer à tous les appareils.
    Updated(i64),
    /// La marque connue est plus récente (ou égale) : à renvoyer à l'appareil.
    Unchanged(Option<i64>),
}

/// Les marques de lecture de tous les utilisateurs (lues à la demande).
#[derive(Debug)]
pub struct ReadMarkers {
    /// `<données>/users` ; vide : en mémoire seulement.
    root: PathBuf,
    users: HashMap<String, UserMarkers>,
}

impl ReadMarkers {
    pub fn new(root: PathBuf) -> ReadMarkers {
        ReadMarkers { root, users: HashMap::new() }
    }

    fn path(&self, user: &str) -> Option<PathBuf> {
        (!self.root.as_os_str().is_empty()).then(|| self.root.join(user).join("readmarkers.json"))
    }

    fn user_mut(&mut self, user: &str) -> &mut UserMarkers {
        if !self.users.contains_key(user) {
            let loaded = self
                .path(user)
                .and_then(|p| std::fs::read_to_string(p).ok())
                .and_then(|t| serde_json::from_str::<UserMarkers>(&t).ok())
                .unwrap_or_default();
            self.users.insert(user.to_string(), loaded);
        }
        self.users.get_mut(user).expect("présent")
    }

    /// Marque connue (ms).
    pub fn get(&mut self, user: &str, net: &str, target: &str) -> Option<i64> {
        self.user_mut(user).get(net).and_then(|m| m.get(target)).and_then(|t| parse_time(t))
    }

    /// Avance la marque si `ts` est plus récent (borné à `now`) ; l'écrit sur le disque.
    pub fn set(&mut self, user: &str, net: &str, target: &str, ts: i64, now: i64) -> MarkResult {
        let ts = ts.min(now);
        let current = self.get(user, net, target);
        if current.is_some_and(|c| ts <= c) {
            return MarkResult::Unchanged(current);
        }
        self.user_mut(user).entry(net.to_string()).or_default().insert(target.to_string(), format_time(ts));
        self.save(user);
        MarkResult::Updated(ts)
    }

    /// Oublie les marques d'un réseau supprimé.
    pub fn forget_network(&mut self, user: &str, net: &str) {
        if self.user_mut(user).remove(net).is_some() {
            self.save(user);
        }
    }

    fn save(&self, user: &str) {
        let (Some(path), Some(m)) = (self.path(user), self.users.get(user)) else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let Ok(text) = serde_json::to_string_pretty(m) else { return };
        let mut tmp = path.as_os_str().to_owned();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        if let Err(e) = crate::config::write_private(&tmp, &text).and_then(|_| std::fs::rename(&tmp, &path)) {
            log::error!("{} : {e}", path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_only_move_forward_and_persist() {
        let dir = std::env::temp_dir().join(format!("alpirc-relay-markers-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut m = ReadMarkers::new(dir.clone());
        let now = 2_000_000;
        assert_eq!(m.get("gui", "1", "#lyon"), None);
        assert_eq!(m.set("gui", "1", "#lyon", 1_000_000, now), MarkResult::Updated(1_000_000));
        assert_eq!(m.set("gui", "1", "#lyon", 999_000, now), MarkResult::Unchanged(Some(1_000_000)));
        assert_eq!(m.set("gui", "1", "#lyon", 1_000_000, now), MarkResult::Unchanged(Some(1_000_000)));
        // Borné à maintenant.
        assert_eq!(m.set("gui", "1", "#lyon", 9_000_000, now), MarkResult::Updated(now));
        assert_eq!(m.set("gui", "2", "#lyon", 5, now), MarkResult::Updated(5));
        assert_eq!(m.set("autre", "1", "#lyon", 7, now), MarkResult::Updated(7));
        // Relu depuis le disque.
        let mut m2 = ReadMarkers::new(dir.clone());
        assert_eq!(m2.get("gui", "1", "#lyon"), Some(now));
        assert_eq!(m2.get("gui", "2", "#lyon"), Some(5));
        assert_eq!(m2.get("autre", "1", "#lyon"), Some(7));
        assert_eq!(m2.get("gui", "1", "bob"), None);
        m2.forget_network("gui", "2");
        assert_eq!(ReadMarkers::new(dir.clone()).get("gui", "2", "#lyon"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
