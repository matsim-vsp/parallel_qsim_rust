use std::collections::VecDeque;
use std::fmt::Debug;

use crate::simulation::id::{Id, IdImpl};
use crate::simulation::io::network::IOLink;
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
    pub fn from(&self) -> &Id<Node> {
        match self {
            SimLink::Local(l) => &l.from,
            SimLink::In(l) => &l.local_link.from,
            SimLink::Out(_) => {
                panic!("There is no from id of a split out link.")
            }
        }
    }

    pub fn to(&self) -> &Id<Node> {
        match self {
            SimLink::Local(l) => &l.to,
            SimLink::In(l) => &l.local_link.to,
            SimLink::Out(_) => {
                panic!("There is no to id of a split out link.")
            }
        }
    }

    pub fn contains_mode(&self, mode: &String) -> bool {
        match self {
            SimLink::Local(l) => l.modes.contains(mode),
            SimLink::In(l) => l.local_link.modes.contains(mode),
            SimLink::Out(_) => {
                panic!("There is not enough information for SplitOutLinks to evaluate.")
            }
        }
    }

    pub fn freespeed(&self) -> f32 {
        match self {
            SimLink::Local(l) => l.freespeed,
            SimLink::In(l) => l.local_link.freespeed,
            SimLink::Out(_) => {
                panic!("There is no freespeed of a split out link.")
            }
        }
    }

    pub fn length(&self) -> f32 {
        match self {
            SimLink::Local(l) => l.length,
            SimLink::In(l) => l.local_link.length,
            SimLink::Out(_) => {
                panic!("There is no length of a split out link.")
            }
        }
    }

    pub fn id(&self) -> Id<Link> {
        match self {
            SimLink::Local(l) => l.id.clone(),
            SimLink::In(l) => l.local_link.id.clone(),
            SimLink::Out(l) => l.id.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LocalLink {
    pub id: Id<Link>,
    q: VecDeque<VehicleQEntry<Vehicle>>,
    length: f32,
    freespeed: f32,
    flowcap: Flowcap,
    modes: Vec<String>,
    pub from: Id<Node>,
    pub to: Id<Node>,
}

#[derive(Debug, Clone)]
struct VehicleQEntry<V> {
    vehicle: V,
    earliest_exit_time: u32,
}

impl LocalLink {
    pub fn from_io_link(
        id: usize,
        link: &IOLink,
        sample_size: f32,
        from: usize,
        to: usize,
    ) -> Self {
        // TODO: remove this method or change parameters to Id<T>
        let wrapped_id = IdImpl::new_internal(id);
        let from_id = IdImpl::new_internal(from);
        let to_id = IdImpl::new_internal(to);
        LocalLink::new(
            wrapped_id,
            link.capacity,
            link.freespeed,
            link.length,
            link.modes(),
            sample_size,
            from_id,
            to_id,
        )
    }

    pub fn from_link(link: &Link, sample_size: f32) -> Self {
        //TODO This should take the modes as set of ids, as well as ids, instead of the internal representation.
        let modes = link
            .modes
            .iter()
            .map(|mode_id| mode_id.external.clone())
            .collect();
        LocalLink::new(
            link.id.clone(),
            link.capacity,
            link.freespeed,
            link.length,
            modes,
            sample_size,
            link.from.clone(),
            link.to.clone(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: Id<Link>,
        capacity_h: f32,
        freespeed: f32,
        length: f32,
        modes: Vec<String>,
        sample_size: f32,
        from: Id<Node>,
        to: Id<Node>,
    ) -> Self {
        LocalLink {
            id,
            q: VecDeque::new(),
            flowcap: Flowcap::new(capacity_h * sample_size / 3600.),
            freespeed,
            modes,
            length,
            from,
            to,
        }
    }

    pub fn push_vehicle(&mut self, vehicle: Vehicle, now: u32) {
        let speed = self.freespeed.min(vehicle.max_v);
        let duration = (self.length / speed) as u32;
        let earliest_exit_time = now + duration;
        self.q.push_back(VehicleQEntry {
            vehicle,
            earliest_exit_time,
        });
    }

    pub fn pop_front(&mut self, now: u32) -> Vec<Vehicle> {
        self.flowcap.update_capacity(now);

        let mut popped_veh = Vec::new();

        while let Some(entry) = self.q.front() {
            if entry.earliest_exit_time > now || !self.flowcap.has_capacity() {
                break;
            }

            let vehicle = self.q.pop_front().unwrap().vehicle;
            self.flowcap.consume_capacity(1.0);
            popped_veh.push(vehicle);
        }

        popped_veh
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
    use crate::simulation::id::IdImpl;
    use crate::simulation::messaging::messages::proto::leg::Route;
    use crate::simulation::messaging::messages::proto::{Activity, NetworkRoute};
    use crate::simulation::messaging::messages::proto::{Agent, Leg, Plan, Vehicle};
    use crate::simulation::network::link::LocalLink;

    #[test]
    fn local_link_push_single_veh() {
        let veh_id = 42;
        let mut link = LocalLink::new(
            IdImpl::new_internal(1),
            1.,
            1.,
            10.,
            vec![],
            1.,
            IdImpl::new_internal(0),
            IdImpl::new_internal(0),
        );
        let agent = create_agent(1, vec![]);
        let vehicle = Vehicle::new(veh_id, 0, 10., 1., Some(agent));

        link.push_vehicle(vehicle, 0);

        // this should put the vehicle into the queue and update the exit time correctly
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
            11.8,
            vec![],
            1.,
            IdImpl::new_internal(0),
            IdImpl::new_internal(0),
        );

        let agent1 = create_agent(1, vec![]);
        let vehicle1 = Vehicle::new(id1, 0, 10., 1., Some(agent1));
        let agent2 = create_agent(1, vec![]);
        let vehicle2 = Vehicle::new(id2, 0, 10., 1., Some(agent2));

        link.push_vehicle(vehicle1, 0);
        link.push_vehicle(vehicle2, 0);

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
            100.,
            vec![],
            1.,
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
        let mut link = LocalLink::new(
            IdImpl::new_internal(1),
            7200.,
            10.,
            100.,
            vec![],
            1.,
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
            100.,
            vec![],
            0.1,
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

    fn create_agent(id: u64, route: Vec<u64>) -> Agent {
        let route = Route::NetworkRoute(NetworkRoute::new(id, route));
        let leg = Leg::new(route, "car", None, None);
        let act = Activity::new(0., 0., 0, 1, None, None, None);
        let mut plan = Plan::new();
        plan.add_act(act);
        plan.add_leg(leg);
        let mut agent = Agent::new(id, plan);
        agent.advance_plan();

        agent
    }
}
