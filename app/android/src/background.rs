//! « Se déconnecter après N minutes en arrière-plan » (ADR 0006) : le compte à rebours, sans horloge ni système,
//! pour être testé seul. La boucle de l'appli lui dit quand AlpIRC passe derrière ou revient devant, et lui demande
//! à chaque tour si le délai est écoulé.

/// Le compte à rebours de la déconnexion en arrière-plan.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BackgroundTimer {
    /// Délai en minutes ; 0 : jamais.
    minutes: u32,
    /// Instant (ms) du passage en arrière-plan, tant qu'AlpIRC y est.
    since: Option<i64>,
    /// La déconnexion a eu lieu pendant ce séjour en arrière-plan.
    fired: bool,
}

impl BackgroundTimer {
    pub fn new(minutes: u32) -> BackgroundTimer {
        BackgroundTimer { minutes, ..BackgroundTimer::default() }
    }

    pub fn minutes(&self) -> u32 {
        self.minutes
    }

    /// Nouveau délai (menu ⋮) ; un compte à rebours en cours le prend en compte.
    pub fn set_minutes(&mut self, minutes: u32) {
        self.minutes = minutes;
    }

    /// AlpIRC passe en arrière-plan à `now` (ms) : le compte à rebours part (il n'est pas relancé si AlpIRC y
    /// était déjà).
    pub fn background(&mut self, now: i64) {
        if self.since.is_none() {
            self.since = Some(now);
            self.fired = false;
        }
    }

    /// AlpIRC revient au premier plan : le compte à rebours est annulé. Renvoie vrai si la déconnexion avait eu
    /// lieu entre-temps (l'appli se reconnecte alors).
    pub fn foreground(&mut self) -> bool {
        self.since = None;
        std::mem::take(&mut self.fired)
    }

    /// Instant (ms) de la déconnexion prévue, s'il y en a une.
    pub fn deadline(&self) -> Option<i64> {
        match (self.minutes, self.since, self.fired) {
            (m, Some(t0), false) if m > 0 => Some(t0 + m as i64 * 60_000),
            _ => None,
        }
    }

    /// Vrai une seule fois, quand le délai est écoulé : il faut alors se déconnecter.
    pub fn poll(&mut self, now: i64) -> bool {
        match self.deadline() {
            Some(d) if now >= d => {
                self.fired = true;
                true
            }
            _ => false,
        }
    }

    /// La déconnexion a eu lieu pendant ce séjour en arrière-plan.
    pub fn fired(&self) -> bool {
        self.fired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: i64 = 60_000;

    #[test]
    fn jamais_par_defaut() {
        let mut t = BackgroundTimer::new(0);
        t.background(0);
        assert_eq!(t.deadline(), None);
        assert!(!t.poll(10_000 * MIN));
        assert!(!t.foreground());
    }

    #[test]
    fn deconnexion_apres_le_delai_une_seule_fois() {
        let mut t = BackgroundTimer::new(15);
        t.background(1_000);
        assert_eq!(t.deadline(), Some(1_000 + 15 * MIN));
        assert!(!t.poll(1_000 + 15 * MIN - 1), "pas avant");
        assert!(t.poll(1_000 + 15 * MIN));
        assert!(t.fired());
        assert!(!t.poll(1_000 + 20 * MIN), "une seule fois");
        assert_eq!(t.deadline(), None);
        // Retour au premier plan : la déconnexion avait eu lieu (reconnexion), puis tout est remis à zéro.
        assert!(t.foreground());
        assert!(!t.foreground());
        assert!(!t.fired());
    }

    #[test]
    fn retour_avant_le_delai_annule() {
        let mut t = BackgroundTimer::new(5);
        t.background(0);
        assert!(!t.poll(4 * MIN));
        assert!(!t.foreground(), "pas de déconnexion : rien à reprendre");
        assert!(!t.poll(6 * MIN), "plus de compte à rebours au premier plan");
        // Nouveau séjour en arrière-plan : le délai repart de zéro.
        t.background(10 * MIN);
        assert!(!t.poll(14 * MIN));
        assert!(t.poll(15 * MIN));
    }

    #[test]
    fn arriere_plan_repete_ne_relance_pas() {
        let mut t = BackgroundTimer::new(5);
        t.background(0);
        // Android peut signaler deux fois le passage derrière (onPause, puis une autre activité) : le délai court
        // depuis le premier.
        t.background(3 * MIN);
        assert!(t.poll(5 * MIN));
    }

    #[test]
    fn changement_du_delai_en_cours() {
        let mut t = BackgroundTimer::new(60);
        t.background(0);
        t.set_minutes(5);
        assert_eq!(t.minutes(), 5);
        assert!(t.poll(5 * MIN));
        let mut t = BackgroundTimer::new(5);
        t.background(0);
        t.set_minutes(0);
        assert!(!t.poll(60 * MIN), "désactivé en cours de route");
    }
}
