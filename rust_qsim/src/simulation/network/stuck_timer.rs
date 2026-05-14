use crate::simulation::time::Tick;
use std::cell::Cell;

#[derive(Debug, Clone)]
pub struct StuckTimer {
    timer_started: Cell<Option<Tick>>,
    stuck_threshold: Tick,
}

impl StuckTimer {
    pub fn new(stuck_threshold: Tick) -> Self {
        StuckTimer {
            timer_started: Cell::new(None),
            stuck_threshold,
        }
    }

    pub fn start(&self, now: impl Into<Tick>) {
        let now = now.into();
        if self.timer_started.get().is_none() {
            self.timer_started.replace(Some(now));
        }
    }

    pub fn reset(&self) {
        self.timer_started.replace(None);
    }

    pub fn is_stuck(&self, now: impl Into<Tick>) -> bool {
        let now = now.into();
        if let Some(time) = self.timer_started.get() {
            now - time >= self.stuck_threshold
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::network::stuck_timer::StuckTimer;
    use crate::simulation::time::Tick;

    #[test]
    fn init() {
        let timer = StuckTimer::new(Tick::new(42));
        assert!(timer.timer_started.get().is_none());
        assert_eq!(Tick::new(42), timer.stuck_threshold);
    }

    #[test]
    fn start() {
        let timer = StuckTimer::new(Tick::new(42));

        timer.start(Tick::new(1));
        timer.start(Tick::new(2));

        assert!(timer.timer_started.get().is_some());
        assert_eq!(Tick::new(1), timer.timer_started.get().unwrap());
    }

    #[test]
    fn reset() {
        let timer = StuckTimer::new(Tick::new(42));

        timer.start(Tick::new(17));
        assert!(timer.timer_started.get().is_some());

        timer.reset();
        assert!(timer.timer_started.get().is_none());
    }

    #[test]
    fn is_stuck() {
        let timer = StuckTimer::new(Tick::new(42));

        timer.start(Tick::new(17));
        assert!(!timer.is_stuck(Tick::new(18)));
        assert!(timer.is_stuck(Tick::new(17 + 42)));

        timer.reset();
        assert!(!timer.is_stuck(Tick::new(17 + 42)));
    }
}
