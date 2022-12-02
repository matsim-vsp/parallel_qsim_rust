use crate::io::network::IOLink;
use crate::parallel_simulation::network::flowcap::Flowcap;
use crate::parallel_simulation::vehicles::Vehicle;
use std::collections::VecDeque;

#[derive(Debug)]
pub enum Link {
    LocalLink(LocalLink),
    SplitInLink(SplitInLink),
    SplitOutLink(SplitOutLink),
}

#[derive(Debug)]
pub struct LocalLink {
    pub id: usize,
    q: VecDeque<Vehicle>,
    length: f32,
    freespeed: f32,
    flowcap: Flowcap,
}

impl LocalLink {
    pub fn from_io_link(id: usize, link: &IOLink) -> LocalLink {
        LocalLink::new(id, link.capacity, link.freespeed, link.length)
    }

    pub fn new(id: usize, capacity_h: f32, freespeed: f32, length: f32) -> LocalLink {
        LocalLink {
            id,
            q: VecDeque::new(),
            flowcap: Flowcap::new(capacity_h / 3600.),
            freespeed,
            length,
        }
    }

    pub fn push_vehicle(&mut self, mut vehicle: Vehicle, now: u32) {
        let exit_time = now + (self.length / self.freespeed) as u32;
        vehicle.exit_time = exit_time;
        self.q.push_back(vehicle);
    }

    pub fn pop_front(&mut self, now: u32) -> Vec<Vehicle> {
        self.flowcap.update_capacity(now);
        let mut popped_veh = Vec::new();

        while let Some(vehicle) = self.q.front() {
            if vehicle.exit_time > now || !self.flowcap.has_capacity() {
                break;
            }

            let vehicle = self.q.pop_front().unwrap();
            self.flowcap.consume_capacity(1.0);
            popped_veh.push(vehicle);
        }

        popped_veh
    }

    pub fn id(&self) -> usize {
        self.id
    }
}

#[derive(Debug)]
pub struct SplitOutLink {
    id: usize,
    to_thread_id: usize,
}

impl SplitOutLink {
    pub fn new(id: usize, to_thread_id: usize) -> SplitOutLink {
        SplitOutLink { id, to_thread_id }
    }

    pub fn neighbor_partition_id(&self) -> usize {
        self.to_thread_id
    }
    pub fn id(&self) -> usize {
        self.id
    }
}

#[derive(Debug)]
pub struct SplitInLink {
    from_thread_id: usize,
    local_link: LocalLink,
}

impl SplitInLink {
    pub fn new(from_thread_id: usize, local_link: LocalLink) -> SplitInLink {
        SplitInLink {
            from_thread_id,
            local_link,
        }
    }

    pub fn neighbor_partition_id(&self) -> usize {
        self.from_thread_id
    }

    pub fn local_link_mut(&mut self) -> &mut LocalLink {
        &mut self.local_link
    }
}

#[cfg(test)]
mod tests {
    use crate::parallel_simulation::network::link::LocalLink;
    use crate::parallel_simulation::vehicles::Vehicle;

    #[test]
    fn local_link_push_single_veh() {
        let veh_id = 42;
        let mut link = LocalLink::new(1, 1., 1., 10.);
        let vehicle = Vehicle::new(veh_id, 1, vec![]);

        link.push_vehicle(vehicle, 0);

        // this should put the vehicle into the queue and update the exit time correctly
        let pushed_vehicle = link.q.front().unwrap();
        assert_eq!(veh_id, pushed_vehicle.id);
        assert_eq!(10, pushed_vehicle.exit_time);
    }

    #[test]
    fn local_link_push_multiple_veh() {
        let id1 = 42;
        let id2 = 43;
        let mut link = LocalLink::new(1, 1., 1., 11.8);
        let vehicle1 = Vehicle::new(id1, id1, vec![]);
        let vehicle2 = Vehicle::new(id2, id2, vec![]);

        link.push_vehicle(vehicle1, 0);
        link.push_vehicle(vehicle2, 0);

        // make sure that vehicles are added ad the end of the queue
        assert_eq!(2, link.q.len());

        let popped_vehicle1 = link.q.pop_front().unwrap();
        assert_eq!(id1, popped_vehicle1.id);
        assert_eq!(11, popped_vehicle1.exit_time);

        let popped_vehicle2 = link.q.pop_front().unwrap();
        assert_eq!(id2, popped_vehicle2.id);
        assert_eq!(11, popped_vehicle2.exit_time);
    }

    #[test]
    fn local_link_pop_with_exit_time() {
        let mut link = LocalLink::new(1, 1000000., 10., 100.);

        let mut n: u32 = 0;

        while n < 10 {
            link.push_vehicle(Vehicle::new(n as usize, n as usize, vec![]), n);
            n += 1;
        }

        let pop1 = link.pop_front(12);
        assert_eq!(3, pop1.len());
        let pop2 = link.pop_front(12);
        assert_eq!(0, pop2.len());
        let pop3 = link.pop_front(20);
        assert_eq!(7, pop3.len());
    }

    #[test]
    fn local_link_pop_with_capacity() {
        // link has capacity of 2 per second
        let mut link = LocalLink::new(1, 7200., 10., 100.);

        let mut n: u32 = 0;

        while n < 10 {
            link.push_vehicle(Vehicle::new(n as usize, n as usize, vec![]), n);
            n += 1;
        }

        n = 0;
        while n < 5 {
            let popped = link.pop_front(20 + n);
            assert_eq!(2, popped.len());
            assert_eq!(10 + n * 2, popped.get(0).unwrap().exit_time);
            assert_eq!(11 + n * 2, popped.get(1).unwrap().exit_time);
            n += 1;
        }
    }
}
