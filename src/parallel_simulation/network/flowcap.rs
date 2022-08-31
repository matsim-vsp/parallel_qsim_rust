#[derive(Debug)]
pub struct Flowcap {
    last_update_time: u32,
    accumulated_capacity: f32,
    capacity_s: f32,
}

impl Flowcap {
    pub fn new(capacity_s: f32) -> Flowcap {
        Flowcap {
            last_update_time: 0,
            accumulated_capacity: capacity_s,
            capacity_s,
        }
    }

    /**
    Updates the accumulated capacity if the time has advanced.
     */
    pub fn update_capacity(&mut self, now: u32) {
        if self.last_update_time < now {
            let time_steps: f32 = (now - self.last_update_time) as f32;
            let acc_flow_cap = time_steps * self.capacity_s + self.accumulated_capacity;
            self.accumulated_capacity = f32::min(acc_flow_cap, self.capacity_s);
            self.last_update_time = now;
        }
    }

    pub fn has_capacity(&self) -> bool {
        self.accumulated_capacity > 0.0
    }

    pub fn consume_capacity(&mut self, by: f32) {
        self.accumulated_capacity -= by;
    }
}

#[cfg(test)]
mod tests {
    use super::Flowcap;

    #[test]
    fn flowcap_consume_capacity() {
        let mut flowcap = Flowcap::new(10.0);
        assert!(flowcap.has_capacity());

        flowcap.consume_capacity(20.0);
        assert!(!flowcap.has_capacity());
    }

    #[test]
    fn flowcap_max_capacity_s() {
        let mut flowcap = Flowcap::new(10.0);

        flowcap.update_capacity(20);

        assert_eq!(10.0, flowcap.accumulated_capacity);
        assert_eq!(20, flowcap.last_update_time);
    }

    #[test]
    fn flowcap_acc_capacity() {
        let mut flowcap = Flowcap::new(0.25);
        assert!(flowcap.has_capacity());

        // accumulated_capacity should be at -0.75 after this.
        flowcap.consume_capacity(1.0);
        assert!(!flowcap.has_capacity());

        // accumulated_capacity should be at -0.5
        flowcap.update_capacity(1);
        assert!(!flowcap.has_capacity());

        // accumulated_capacity should be at 0.0
        flowcap.update_capacity(3);
        assert!(!flowcap.has_capacity());

        // accumulated capacity should be at 0.5
        flowcap.update_capacity(5);
        assert!(flowcap.has_capacity());
    }
}