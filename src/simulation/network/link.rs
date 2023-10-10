use std::collections::VecDeque;
use std::fmt::Debug;

use crate::simulation::id::Id;
use crate::simulation::messaging::messages::proto::Vehicle;
use crate::simulation::network::flow_cap::Flowcap;
use crate::simulation::network::global_network::Node;

use super::global_network::Link;

#[derive(Debug, Clone)]
pub enum SimLink {
    Local(LocalLink),
    In(SplitInLink),
    Out(SplitOutLink),
}

impl SimLink {
    pub fn offers_veh(&self, now: u32) -> Option<&Vehicle> {
        match self {
            SimLink::Local(ll) => ll.offers_veh(now),
            SimLink::In(il) => il.local_link.offers_veh(now),
            SimLink::Out(_) => {
                panic!("can't query out links to offer vehicles.")
            }
        }
    }

    pub fn accepts_veh(&self) -> bool {
        match self {
            SimLink::Local(ll) => ll.accepts_veh(),
            SimLink::In(_) => {
                panic!("In Links can't accept vehicles")
            }
            SimLink::Out(_) => {
                panic!("Accept veh. not yet implemented for our links")
            }
        }
    }

    pub fn pop_veh(&mut self) -> Vehicle {
        match self {
            SimLink::Local(ll) => ll.q.pop_front().unwrap().vehicle,
            SimLink::In(il) => il.local_link.q.pop_front().unwrap().vehicle,
            SimLink::Out(_) => {
                panic!("Can't pop vehicle from out link")
            }
        }
    }

    pub fn update_flow_cap(&mut self, now: u32) {
        match self {
            SimLink::Local(ll) => ll.update_flow_cap(now),
            SimLink::In(il) => il.local_link.update_flow_cap(now),
            SimLink::Out(_) => {
                panic!("can't update flow cap on out links.")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct LocalLink {
    pub id: Id<Link>,
    q: VecDeque<VehicleQEntry<Vehicle>>,
    length: f32,
    free_speed: f32,
    storage_cap: f32,
    used_storage_cap: f32,
    flow_cap: Flowcap,
    pub from: Id<Node>,
    pub to: Id<Node>,
}

#[derive(Debug, Clone)]
struct VehicleQEntry<V> {
    vehicle: V,
    earliest_exit_time: u32,
}

impl LocalLink {
    pub fn from_link(link: &Link, sample_size: f32, effective_cell_size: f32) -> Self {
        LocalLink::new(
            link.id.clone(),
            link.capacity,
            link.freespeed,
            link.permlanes,
            link.length,
            sample_size,
            effective_cell_size,
            link.from.clone(),
            link.to.clone(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: Id<Link>,
        capacity_h: f32,
        free_speed: f32,
        perm_lanes: f32,
        length: f32,
        sample_size: f32,
        effective_cell_size: f32,
        from: Id<Node>,
        to: Id<Node>,
    ) -> Self {
        let flow_cap_s = capacity_h * sample_size / 3600.;
        let storage_cap = Self::calculate_storage_cap(
            length,
            perm_lanes,
            flow_cap_s,
            sample_size,
            effective_cell_size,
        );

        LocalLink {
            id,
            q: VecDeque::new(),
            flow_cap: Flowcap::new(flow_cap_s),
            free_speed,
            length,
            from,
            to,
            storage_cap,
            used_storage_cap: 0.0,
        }
    }

    pub fn push_vehicle(&mut self, vehicle: Vehicle, now: u32) {
        let speed = self.free_speed.min(vehicle.max_v);
        let duration = (self.length / speed) as u32;
        let earliest_exit_time = now + duration;

        // update state
        self.consume_storage_cap(vehicle.pce);
        self.q.push_back(VehicleQEntry {
            vehicle,
            earliest_exit_time,
        });
    }

    pub fn pop_front(&mut self, now: u32) -> Vec<Vehicle> {
        self.flow_cap.update_capacity(now);

        let mut popped_veh = Vec::new();

        while let Some(entry) = self.q.front() {
            if entry.earliest_exit_time > now || !self.flow_cap.has_capacity() {
                break;
            }

            // pop vehicle from queue, consume flow capacity, and release blocked storage capacity
            let vehicle = self.q.pop_front().unwrap().vehicle;
            self.flow_cap.consume_capacity(1.0);
            self.release_storage_cap(vehicle.pce);

            popped_veh.push(vehicle);
        }

        popped_veh
    }

    pub fn update_flow_cap(&mut self, now: u32) {
        // increase flow cap if new time step
        self.flow_cap.update_capacity(now);
    }

    pub fn offers_veh(&self, now: u32) -> Option<&Vehicle> {
        // check if we have flow cap left for current time step, otherwise abort
        if !self.flow_cap.has_capacity() {
            return None;
        }

        // peek if fist vehicle in queue can leave
        if let Some(entry) = self.q.front() {
            if entry.earliest_exit_time <= now {
                return Some(&entry.vehicle);
            }
        }

        None
    }

    pub fn available_storage_capacity(&self) -> f32 {
        self.storage_cap - self.used_storage_cap
    }

    pub fn accepts_veh(&self) -> bool {
        self.available_storage_capacity() > 0.0
    }

    fn consume_storage_cap(&mut self, cap: f32) {
        self.used_storage_cap = self.storage_cap.min(self.used_storage_cap + cap);
    }

    fn release_storage_cap(&mut self, cap: f32) {
        self.used_storage_cap = 0f32.max(self.used_storage_cap - cap);
    }

    fn calculate_storage_cap(
        length: f32,
        perm_lanes: f32,
        flow_cap_s: f32,
        sample_size: f32,
        effective_cell_size: f32,
    ) -> f32 {
        let cap = length * perm_lanes * sample_size / effective_cell_size;
        // storage capacity needs to be at least enough to handle the cap_per_time_step:
        cap.max(flow_cap_s)

        // the original code contains more logic to increase storage capacity for links with a low
        // free speed. Omit this for now, as we don't want to create a feature complete qsim
    }
}

#[derive(Debug, Clone)]
pub struct SplitOutLink {
    pub(crate) id: Id<Link>,
    to_part: usize,
}

impl SplitOutLink {
    pub fn new(id: Id<Link>, to_part: usize) -> SplitOutLink {
        SplitOutLink { id, to_part }
    }

    pub fn neighbor_partition_id(&self) -> usize {
        self.to_part
    }
}

#[derive(Debug, Clone)]
pub struct SplitInLink {
    from_part: usize,
    local_link: LocalLink,
}

impl SplitInLink {
    pub fn new(from_part: usize, local_link: LocalLink) -> Self {
        SplitInLink {
            from_part,
            local_link,
        }
    }

    pub fn neighbor_partition_id(&self) -> usize {
        self.from_part
    }

    pub fn local_link_mut(&mut self) -> &mut LocalLink {
        &mut self.local_link
    }

    pub fn local_link(&self) -> &LocalLink {
        &self.local_link
    }
}

#[cfg(test)]
mod tests {
    use assert_approx_eq::assert_approx_eq;

    use crate::simulation::id::IdImpl;
    use crate::simulation::messaging::messages::proto::{Activity, Route};
    use crate::simulation::messaging::messages::proto::{Agent, Leg, Plan, Vehicle};
    use crate::simulation::network::link::LocalLink;

    #[test]
    fn local_link_push_single_veh() {
        let veh_id = 42;
        let mut link = LocalLink::new(
            IdImpl::new_internal(1),
            1.,
            1.,
            1.,
            10.,
            1.,
            7.5,
            IdImpl::new_internal(0),
            IdImpl::new_internal(0),
        );
        let agent = create_agent(1, vec![]);
        let vehicle = Vehicle::new(veh_id, 0, 10., 1., Some(agent));

        // this should put the vehicle into the queue and update the exit time correctly
        link.push_vehicle(vehicle, 0);

        assert_eq!(0.33333337, link.available_storage_capacity());
        let pushed_vehicle = link.q.front().unwrap();
        assert_eq!(veh_id, pushed_vehicle.vehicle.id);
        assert_eq!(10, pushed_vehicle.earliest_exit_time);
    }

    #[test]
    fn local_link_push_multiple_veh() {
        let id1 = 42;
        let id2 = 43;
        let mut link = LocalLink::new(
            IdImpl::new_internal(1),
            1.,
            1.,
            1.,
            11.8,
            1.,
            7.5,
            IdImpl::new_internal(0),
            IdImpl::new_internal(0),
        );

        let agent1 = create_agent(1, vec![]);
        let vehicle1 = Vehicle::new(id1, 0, 10., 1., Some(agent1));
        let agent2 = create_agent(1, vec![]);
        let vehicle2 = Vehicle::new(id2, 0, 10., 1., Some(agent2));

        link.push_vehicle(vehicle1, 0);
        assert_approx_eq!(0.57, link.available_storage_capacity(), 0.01);
        assert!(link.accepts_veh());

        link.push_vehicle(vehicle2, 0);
        assert_approx_eq!(0., link.available_storage_capacity());
        assert!(!link.accepts_veh());

        // make sure that vehicles are added ad the end of the queue
        assert_eq!(2, link.q.len());

        let popped_vehicle1 = link.q.pop_front().unwrap();
        assert_eq!(id1, popped_vehicle1.vehicle.id);
        assert_eq!(11, popped_vehicle1.earliest_exit_time);

        let popped_vehicle2 = link.q.pop_front().unwrap();
        assert_eq!(id2, popped_vehicle2.vehicle.id);
        assert_eq!(11, popped_vehicle2.earliest_exit_time);
    }

    #[test]
    fn local_link_pop_with_exit_time() {
        let mut link = LocalLink::new(
            IdImpl::new_internal(1),
            1000000.,
            10.,
            1.,
            100.,
            1.,
            7.5,
            IdImpl::new_internal(0),
            IdImpl::new_internal(0),
        );

        let mut n: u32 = 0;

        while n < 10 {
            let agent = create_agent(n as u64, vec![]);
            let vehicle = Vehicle::new(n as u64, 0, 10., 1., Some(agent));
            link.push_vehicle(vehicle, n);
            n += 1;
        }

        assert_approx_eq!(267.7, link.available_storage_capacity(), 0.1);
        let pop1 = link.pop_front(12);
        assert_eq!(3, pop1.len());
        assert_approx_eq!(270.7, link.available_storage_capacity(), 0.1);
        let pop2 = link.pop_front(12);
        assert_eq!(0, pop2.len());
        assert_approx_eq!(270.7, link.available_storage_capacity(), 0.1);
        let pop3 = link.pop_front(20);
        assert_eq!(7, pop3.len());
        assert_approx_eq!(277.7, link.available_storage_capacity(), 0.1);
    }

    #[test]
    fn local_link_pop_with_capacity() {
        // link has capacity of 2 per second
        let mut link = LocalLink::new(
            IdImpl::new_internal(1),
            7200.,
            10.,
            100.,
            1.,
            1.,
            7.5,
            IdImpl::new_internal(0),
            IdImpl::new_internal(0),
        );

        let mut n: u32 = 0;

        while n < 10 {
            let agent = create_agent(n as u64, vec![]);
            let vehicle = Vehicle::new(n as u64, 0, 10., 1., Some(agent));
            link.push_vehicle(vehicle, n);
            n += 1;
        }

        n = 0;
        while n < 5 {
            let popped = link.pop_front(20 + n);
            assert_eq!(2, popped.len());
            n += 1;
        }
    }

    #[test]
    fn local_link_pop_with_capacity_reduced() {
        // link has a capacity of 1 * 0.1 per second
        let mut link = LocalLink::new(
            IdImpl::new_internal(1),
            3600.,
            10.,
            1.,
            100.,
            0.1,
            7.5,
            IdImpl::new_internal(0),
            IdImpl::new_internal(0),
        );

        let agent1 = create_agent(1, vec![]);
        let vehicle1 = Vehicle::new(1, 0, 10., 1., Some(agent1));
        let agent2 = create_agent(2, vec![]);
        let vehicle2 = Vehicle::new(2, 0, 10., 1., Some(agent2));
        link.push_vehicle(vehicle1, 0);
        link.push_vehicle(vehicle2, 0);

        let popped = link.pop_front(10);
        assert_eq!(1, popped.len());

        // actually this shouldn't let vehicles at 19 seconds as well, but due to floating point arithmatic
        // the flowcap inside the link has a accumulated capacity slightly greater than 0 at 19 🤷‍♀️
        let popped_2 = link.pop_front(18);
        assert_eq!(0, popped_2.len());

        let popped_3 = link.pop_front(20);
        assert_eq!(1, popped_3.len());
    }

    #[test]
    fn init_storage_cap() {
        let link = LocalLink::new(
            IdImpl::new_internal(1),
            3600.,
            10.,
            2.,
            100.,
            0.1,
            5.,
            IdImpl::new_internal(0),
            IdImpl::new_internal(0),
        );

        assert_eq!(4., link.storage_cap);
    }

    #[test]
    fn init_storage_cap_high_cappa_link() {
        let link = LocalLink::new(
            IdImpl::new_internal(1),
            36000.,
            10.,
            2.,
            10.,
            0.1,
            5.,
            IdImpl::new_internal(0),
            IdImpl::new_internal(0),
        );

        // storage capacity would be 0.2, but must be increased to 1.0 to accommodate flow cap
        assert_eq!(1., link.storage_cap);
    }

    fn create_agent(id: u64, route: Vec<u64>) -> Agent {
        let route = Route {
            veh_id: id,
            distance: 0.0,
            route,
        };
        let leg = Leg::new(route, 0, None, None);
        let act = Activity::new(0., 0., 0, 1, None, None, None);
        let mut plan = Plan::new();
        plan.add_act(act);
        plan.add_leg(leg);
        let mut agent = Agent::new(id, plan);
        agent.advance_plan();

        agent
    }
}
