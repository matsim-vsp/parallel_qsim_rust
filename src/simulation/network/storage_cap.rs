#[derive(Debug, Clone)]
pub struct StorageCap {
    pub(crate) max: f32,
    // keeps track of storage capacity released by vehicles leaving the link during one time step
    // on release_storage_cap, the used_storage_cap is reduced to account for vehicles leaving the
    // link. This is necessary, because we want additional storage capacity to be available only in
    // the following time step, to keep the resulting traffic pattern independent from the order in
    // which nodes are processed in the qsim.
    pub released: f32,
    // keeps track of the storage capacity consumed by the vehicles in the q. This property gets
    // updated immediately once a vehicle is pushed onto the link.
    pub used: f32,
}

impl StorageCap {
    pub fn new(
        length: f32,
        perm_lanes: f32,
        flow_cap_s: f32,
        sample_size: f32,
        effective_cell_size: f32,
    ) -> Self {
        let cap = length * perm_lanes * sample_size / effective_cell_size;
        // storage capacity needs to be at least enough to handle the cap_per_time_step:
        let max_storage_cap = cap.max(flow_cap_s);

        // the original code contains more logic to increase storage capacity for links with a low
        // free speed. Omit this for now, as we don't want to create a feature complete qsim

        Self {
            max: max_storage_cap,
            released: 0.0,
            used: 0.0,
        }
    }

    pub fn consume(&mut self, value: f32) {
        self.used = self.max.min(self.used + value);
    }

    pub fn clear(&mut self) {
        self.used = 0.;
    }

    pub fn release(&mut self, value: f32) {
        self.released += value;
    }

    pub fn apply_released(&mut self) {
        self.used = 0f32.max(self.used - self.released);
        self.released = 0.0;
    }

    pub fn is_available(&self) -> bool {
        let available_cap = self.max - self.used;
        available_cap > 0.0
    }

    pub fn available(&self) -> f32 {
        self.max - self.used
    }
}
