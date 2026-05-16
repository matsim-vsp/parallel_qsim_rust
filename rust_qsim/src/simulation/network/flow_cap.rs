use crate::simulation::time::SimTime;

#[derive(Debug, Clone)]
pub struct Flowcap {
    last_update_time: SimTime,
    value: f64,
    capacity_per_second: f64,
    max_available: f64,
}

impl Flowcap {
    pub(super) fn new(capacity_h: f64, sample_size: f64, max_available: f64) -> Flowcap {
        let capacity_per_second = capacity_h * sample_size / 3600.;
        Flowcap {
            last_update_time: SimTime::default(),
            value: max_available,
            capacity_per_second,
            max_available,
        }
    }

    pub(super) fn update_capacity(&mut self, now: SimTime) {
        if self.last_update_time < now {
            let elapsed = now.duration_since(self.last_update_time).as_secs_f64();
            let acc_flow_cap = elapsed * self.capacity_per_second + self.value;
            self.value = f64::min(acc_flow_cap, self.max_available);
            self.last_update_time = now;
        }
    }

    pub(super) fn has_capacity_left(&self) -> bool {
        self.value > 1e-10
    }

    pub(super) fn value(&self) -> f64 {
        self.value
    }

    pub(super) fn consume(&mut self, by: f64) {
        self.value -= by;
    }

    #[cfg(test)]
    pub(super) fn max_available(&self) -> f64 {
        self.max_available
    }

    pub(super) fn capacity_per_tick(&self) -> f64 {
        self.max_available // FIXME should be .capacity_per_tick?
    }
}

#[cfg(test)]
mod tests {
    use assert_approx_eq::assert_approx_eq;

    use crate::simulation::network::flow_cap::Flowcap;
    use crate::simulation::time::{SimClock, SimTime};
    use std::time::Duration;

    #[test]
    fn init() {
        let clock = SimClock::new(10);
        let cap = Flowcap::new(5432., 0.31415, 0.47401747 / 10.0);
        assert_approx_eq!(0.47401747 / 10.0, cap.max_available(), 0.0001);
        assert_eq!(Duration::from_millis(100), clock.tick_length());
    }

    #[test]
    fn flowcap_consume_capacity() {
        let mut flowcap = Flowcap::new(36000., 1., 1.0);
        assert!(flowcap.has_capacity_left());

        flowcap.consume(20.0);
        assert!(!flowcap.has_capacity_left());
    }

    #[test]
    fn flowcap_max_capacity_s() {
        let mut flowcap = Flowcap::new(36000., 1., 1.0);

        flowcap.update_capacity(SimTime::from_secs(20));

        assert_eq!(1.0, flowcap.value);
    }

    #[test]
    fn flowcap_acc_capacity() {
        let mut flowcap = Flowcap::new(900., 1., 0.25);
        assert!(flowcap.has_capacity_left());

        flowcap.consume(1.0);
        assert!(!flowcap.has_capacity_left());

        flowcap.update_capacity(SimTime::from_secs(1));
        assert!(!flowcap.has_capacity_left());

        flowcap.update_capacity(SimTime::from_secs(3));
        assert!(!flowcap.has_capacity_left());

        flowcap.update_capacity(SimTime::from_secs(5));
        assert!(flowcap.has_capacity_left());
    }
}
