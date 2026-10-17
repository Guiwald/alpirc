//! File d'envoi protégée contre le flood : seau à jetons (une rafale de `burst` lignes, puis une ligne toutes
//! les `rate_ms`). Les lignes prioritaires (PONG, CAP, AUTHENTICATE, enregistrement) passent sans attendre.

use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub(crate) struct FloodQueue {
    queue: VecDeque<String>,
    ready: Vec<String>,
    tokens: f64,
    last: i64,
}

impl FloodQueue {
    pub fn new(burst: u32, now: i64) -> FloodQueue {
        FloodQueue { queue: VecDeque::new(), ready: Vec::new(), tokens: burst as f64, last: now }
    }

    pub fn reset(&mut self, burst: u32, now: i64) {
        self.queue.clear();
        self.ready.clear();
        self.tokens = burst as f64;
        self.last = now;
    }

    /// Ajoute une ligne ; `bypass` : envoyée au prochain [`FloodQueue::drain`] sans consommer de jeton.
    pub fn push(&mut self, line: String, bypass: bool) {
        if bypass {
            self.ready.push(line);
        } else {
            self.queue.push_back(line);
        }
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty() && self.ready.is_empty()
    }

    /// Lignes à envoyer maintenant. `enabled` faux : tout part.
    pub fn drain(&mut self, now: i64, enabled: bool, burst: u32, rate_ms: u64) -> Vec<String> {
        let mut out = std::mem::take(&mut self.ready);
        if !enabled {
            out.extend(self.queue.drain(..));
            self.tokens = burst as f64;
            self.last = now;
            return out;
        }
        let rate = rate_ms.max(1) as f64;
        let elapsed = (now - self.last).max(0) as f64;
        self.tokens = (self.tokens + elapsed / rate).min(burst.max(1) as f64);
        self.last = now;
        while self.tokens >= 1.0 {
            let Some(l) = self.queue.pop_front() else { break };
            out.push(l);
            self.tokens -= 1.0;
        }
        out
    }

    /// Quand la prochaine ligne en attente pourra partir.
    pub fn next_due(&self, rate_ms: u64) -> Option<i64> {
        if !self.ready.is_empty() {
            return Some(self.last);
        }
        if self.queue.is_empty() {
            return None;
        }
        let missing = (1.0 - self.tokens).max(0.0);
        Some(self.last + (missing * rate_ms as f64).ceil() as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket() {
        let mut q = FloodQueue::new(2, 0);
        for i in 0..4 {
            q.push(format!("L{i}"), false);
        }
        q.push("PONG".into(), true);
        assert_eq!(q.drain(0, true, 2, 1000), vec!["PONG", "L0", "L1"]);
        assert_eq!(q.next_due(1000), Some(1000));
        assert!(q.drain(500, true, 2, 1000).is_empty());
        assert_eq!(q.drain(1000, true, 2, 1000), vec!["L2"]);
        assert_eq!(q.drain(5000, true, 2, 1000), vec!["L3"]);
        assert!(q.is_empty());
    }
}
