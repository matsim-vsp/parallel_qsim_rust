use crate::io::network::IOLink;
use crate::parallel_simulation::network::flowcap::Flowcap;
use crate::parallel_simulation::vehicles::Vehicle;
use std::collections::VecDeque;

#[derive(Debug)]
pub enum Link {
    LocalLink(LocalLink),
    SplitLink(SplitLink),
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
}

#[derive(Debug)]
pub struct SplitLink {
    id: usize,
    from_thread_id: usize,
    to_thread_id: usize,
}

impl SplitLink {
    pub fn new(id: usize, from_thread_id: usize, to_thread_id: usize) -> SplitLink {
        SplitLink {
            id,
            from_thread_id,
            to_thread_id,
        }
    }
    
    pub fn to_thread_id(&self) -> usize {
        self.to_thread_id
    }
}
