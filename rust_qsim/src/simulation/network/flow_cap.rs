#[derive(Debug, Clone)]
pub struct Flowcap {
    last_update_time: u32,
    value: f32,
    capacity_per_time_step: f32,
}

impl Flowcap {
    pub(super) fn new(capacity_h: f32, sample_size: f32) -> Flowcap {
        let capacity_s = capacity_h * sample_size / 3600.;
        Flowcap {
            last_update_time: 0,
            value: capacity_s,
            capacity_per_time_step: capacity_s,
        }
    }

    /// Updates the accumulated capacity if the time has advanced.
    pub(super) fn update_capacity(&mut self, now: u32) {
        if self.last_update_time < now {
            let time_steps: f32 = (now - self.last_update_time) as f32;
            let acc_flow_cap = time_steps * self.capacity_per_time_step + self.value;
            self.value = f32::min(acc_flow_cap, self.capacity_per_time_step);
            self.last_update_time = now;
        }
    }

    pub(super) fn has_capacity_left(&self) -> bool {
        self.value > 1e-10
    }

    pub(super) fn value(&self) -> f32 {
        self.value
    }

    pub(super) fn consume(&mut self, by: f32) {
        self.value -= by;
    }

    pub(super) fn capacity_per_time_step(&self) -> f32 {
        self.capacity_per_time_step
    }
}

#[cfg(test)]
mod tests {
    use assert_approx_eq::assert_approx_eq;

    use crate::simulation::network::flow_cap::Flowcap;

    #[test]
    fn init() {
        let cap = Flowcap::new(5432., 0.31415);
        assert_approx_eq!(0.47401747, cap.capacity_per_time_step, 0.0001);
    }

    #[test]
    fn flowcap_consume_capacity() {
        let mut flowcap = Flowcap::new(36000., 1.);
        assert!(flowcap.has_capacity_left());

        flowcap.consume(20.0);
        assert!(!flowcap.has_capacity_left());
    }

    #[test]
    fn flowcap_max_capacity_s() {
        let mut flowcap = Flowcap::new(36000., 1.);

        flowcap.update_capacity(20);

        assert_eq!(10.0, flowcap.value);
        assert_eq!(20, flowcap.last_update_time);
    }

    #[test]
    fn flowcap_acc_capacity() {
        let mut flowcap = Flowcap::new(900., 1.);
        assert!(flowcap.has_capacity_left());

        // accumulated_capacity should be at -0.75 after this.
        flowcap.consume(1.0);
        assert!(!flowcap.has_capacity_left());

        // accumulated_capacity should be at -0.5
        flowcap.update_capacity(1);
        assert!(!flowcap.has_capacity_left());

        // accumulated_capacity should be at 0.0
        flowcap.update_capacity(3);
        assert!(!flowcap.has_capacity_left());

        // accumulated capacity should be at 0.5
        flowcap.update_capacity(5);
        assert!(flowcap.has_capacity_left());
    }
}
