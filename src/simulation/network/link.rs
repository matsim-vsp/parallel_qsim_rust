use std::collections::VecDeque;
use std::fmt::Debug;

use log::warn;

use crate::simulation::io::network::IOLink;
use crate::simulation::io::vehicle_definitions::VehicleDefinitions;
use crate::simulation::messaging::messages::proto::Vehicle;
use crate::simulation::network::flow_cap::Flowcap;

#[derive(Debug, Clone)]
pub enum Link {
    LocalLink(LocalLink),
    SplitInLink(SplitInLink),
    SplitOutLink(SplitOutLink),
}

impl Link {
    pub fn from_to_id(&self) -> (usize, usize) {
        (self.from_id(), self.to_id())
    }

    pub fn from_id(&self) -> usize {
        match self {
            Link::LocalLink(l) => l.from(),
            Link::SplitInLink(l) => l.local_link().from(),
            Link::SplitOutLink(_) => {
                panic!("There is no from id of a split out link.")
            }
        }
    }

    pub fn to_id(&self) -> usize {
        match self {
            Link::LocalLink(l) => l.to(),
            Link::SplitInLink(l) => l.local_link().to(),
            Link::SplitOutLink(_) => {
                panic!("There is no to id of a split out link.")
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct LocalLink {
    id: usize,
    q: VecDeque<VehicleQEntry<Vehicle>>,
    length: f32,
    freespeed: f32,
    flowcap: Flowcap,
    modes: Vec<String>,
    from: usize,
    to: usize,
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
        LocalLink::new(
            id,
            link.capacity,
            link.freespeed,
            link.length,
            link.modes(),
            sample_size,
            from,
            to,
        )
    }

    pub fn new(
        id: usize,
        capacity_h: f32,
        freespeed: f32,
        length: f32,
        modes: Vec<String>,
        sample_size: f32,
        from: usize,
        to: usize,
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

    pub fn push_vehicle(
        &mut self,
        vehicle: Vehicle,
        now: u32,
        vehicle_definitions: Option<&VehicleDefinitions>,
    ) {
        let speed = self.get_speed_for_vehicle(&vehicle, vehicle_definitions);
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

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn from(&self) -> usize {
        self.from
    }

    pub fn to(&self) -> usize {
        self.to
    }

    fn get_speed_for_vehicle(
        &self,
        vehicle: &Vehicle,
        vehicle_definitions: Option<&VehicleDefinitions>,
    ) -> f32 {
        if vehicle_definitions.is_none() {
            return self.freespeed;
        }

        let vehicle_max_speed = vehicle_definitions
            .as_ref()
            .unwrap()
            .get_max_speed_for_mode(vehicle.mode());

        if vehicle_max_speed.is_none() {
            warn!(
                "There is no max speed given for vehicle type {:?}. Using freespeed of links.",
                vehicle.mode()
            );
            return self.freespeed;
        }

        self.freespeed.min(vehicle_max_speed.unwrap())
    }
}

#[derive(Debug, Clone)]
pub struct SplitOutLink {
    id: usize,
    to_part: usize,
}

impl SplitOutLink {
    pub fn new(id: usize, to_part: usize) -> SplitOutLink {
        SplitOutLink { id, to_part }
    }

    pub fn neighbor_partition_id(&self) -> usize {
        self.to_part
    }
    pub fn id(&self) -> usize {
        self.id
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
    use crate::simulation::io::vehicle_definitions::VehicleDefinitions;
    use crate::simulation::messaging::messages::proto::leg::Route;
    use crate::simulation::messaging::messages::proto::{Activity, NetworkRoute};
    use crate::simulation::messaging::messages::proto::{Agent, Leg, Plan, Vehicle, VehicleType};
    use crate::simulation::network::link::LocalLink;

    #[test]
    fn local_link_push_single_veh() {
        let veh_id = 42;
        let mut link = LocalLink::new(1, 1., 1., 10., vec![], 1., 0, 0);
        let agent = create_agent(1, vec![]);
        let vehicle = Vehicle::new(veh_id, VehicleType::Network, String::from("car"), agent);

        link.push_vehicle(vehicle, 0, None);

        // this should put the vehicle into the queue and update the exit time correctly
        let pushed_vehicle = link.q.front().unwrap();
        assert_eq!(veh_id, pushed_vehicle.vehicle.id);
        assert_eq!(10, pushed_vehicle.earliest_exit_time);
    }

    #[test]
    fn local_link_push_multiple_veh() {
        let id1 = 42;
        let id2 = 43;
        let mut link = LocalLink::new(1, 1., 1., 11.8, vec![], 1., 0, 0);

        let agent1 = create_agent(1, vec![]);
        let vehicle1 = Vehicle::new(id1, VehicleType::Network, String::from("car"), agent1);
        let agent2 = create_agent(1, vec![]);
        let vehicle2 = Vehicle::new(id2, VehicleType::Network, String::from("car"), agent2);

        link.push_vehicle(vehicle1, 0, None);
        link.push_vehicle(vehicle2, 0, None);

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
        let mut link = LocalLink::new(1, 1000000., 10., 100., vec![], 1., 0, 0);

        let mut n: u32 = 0;

        while n < 10 {
            let agent = create_agent(n as u64, vec![]);
            let vehicle = Vehicle::new(n as u64, VehicleType::Network, String::from("car"), agent);
            link.push_vehicle(vehicle, n, None);
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
        let mut link = LocalLink::new(1, 7200., 10., 100., vec![], 1., 0, 0);

        let mut n: u32 = 0;

        while n < 10 {
            let agent = create_agent(n as u64, vec![]);
            let vehicle = Vehicle::new(n as u64, VehicleType::Network, String::from("car"), agent);
            link.push_vehicle(vehicle, n, None);
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
        let mut link = LocalLink::new(1, 3600., 10., 100., vec![], 0.1, 0, 0);

        let agent1 = create_agent(1, vec![]);
        let vehicle1 = Vehicle::new(1, VehicleType::Network, String::from("car"), agent1);
        let agent2 = create_agent(2, vec![]);
        let vehicle2 = Vehicle::new(2, VehicleType::Network, String::from("car"), agent2);
        link.push_vehicle(vehicle1, 0, None);
        link.push_vehicle(vehicle2, 0, None);

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
    fn local_link_with_vehicle_definitions() {
        let veh_id_car = 42;
        let veh_id_buggy = 43;
        let veh_id_bike = 44;
        let mut link = LocalLink::new(1, 1., 10., 100., vec![], 1., 0, 0);

        let vehicle_definitions = create_three_vehicle_definitions();

        let agent1 = create_agent(1, vec![]);
        let car = Vehicle::new(
            veh_id_car,
            VehicleType::Network,
            String::from("car"),
            agent1,
        );
        let agent2 = create_agent(2, vec![]);
        let buggy = Vehicle::new(
            veh_id_buggy,
            VehicleType::Network,
            String::from("buggy"),
            agent2,
        );
        let agent3 = create_agent(3, vec![]);
        let bike = Vehicle::new(
            veh_id_bike,
            VehicleType::Network,
            String::from("bike"),
            agent3,
        );

        link.push_vehicle(car, 0, Some(&vehicle_definitions));
        link.push_vehicle(buggy, 0, Some(&vehicle_definitions));
        link.push_vehicle(bike, 0, Some(&vehicle_definitions));

        // this should put the vehicle into the queue and update the exit time correctly
        let pushed_vehicle_car = link.q.get(0).unwrap();
        assert_eq!(veh_id_car, pushed_vehicle_car.vehicle.id);
        assert_eq!(10, pushed_vehicle_car.earliest_exit_time);

        let pushed_vehicle_buggy = link.q.get(1).unwrap();
        assert_eq!(veh_id_buggy, pushed_vehicle_buggy.vehicle.id);
        assert_eq!(10, pushed_vehicle_buggy.earliest_exit_time);

        let pushed_vehicle_bike = link.q.get(2).unwrap();
        assert_eq!(veh_id_bike, pushed_vehicle_bike.vehicle.id);
        assert_eq!(20, pushed_vehicle_bike.earliest_exit_time);
    }

    #[test]
    fn local_link_pop_with_vehicle_definitions() {
        let veh_id_car = 42;
        let veh_id_buggy = 43;
        let veh_id_bike = 44;
        let mut link = LocalLink::new(1, 3600., 10., 100., vec![], 1., 0, 0);

        let vehicle_definitions = create_three_vehicle_definitions();

        let agent1 = create_agent(1, vec![]);
        let car = Vehicle::new(
            veh_id_car,
            VehicleType::Network,
            String::from("car"),
            agent1,
        );
        let agent2 = create_agent(1, vec![]);
        let buggy = Vehicle::new(
            veh_id_buggy,
            VehicleType::Network,
            String::from("buggy"),
            agent2,
        );
        let agent3 = create_agent(1, vec![]);
        let bike = Vehicle::new(
            veh_id_bike,
            VehicleType::Network,
            String::from("bike"),
            agent3,
        );

        link.push_vehicle(bike, 0, Some(&vehicle_definitions));
        link.push_vehicle(buggy, 0, Some(&vehicle_definitions));
        link.push_vehicle(car, 0, Some(&vehicle_definitions));

        let popped = link.pop_front(10);
        assert_eq!(0, popped.len());

        let popped_2 = link.pop_front(20);
        assert_eq!(1, popped_2.len());
        assert!(popped_2.first().unwrap().mode.eq("bike"));

        let popped_3 = link.pop_front(21);
        assert_eq!(1, popped_3.len());
        assert!(popped_3.first().unwrap().mode.eq("buggy"));

        let popped_4 = link.pop_front(22);
        assert_eq!(1, popped_4.len());
        assert!(popped_4.first().unwrap().mode.eq("car"));
    }

    fn create_three_vehicle_definitions() -> VehicleDefinitions {
        VehicleDefinitions::new()
            .add_vehicle_type("car".to_string(), Some(20.))
            .add_vehicle_type("buggy".to_string(), Some(10.))
            .add_vehicle_type("bike".to_string(), Some(5.))
    }

    fn create_agent(id: u64, route: Vec<u64>) -> Agent {
        let route = Route::NetworkRoute(NetworkRoute::new(id, route));
        let leg = Leg::new(route, "car", None, None);
        let act = Activity::new(0., 0., String::from("some-type"), 1, None, None, None);
        let mut plan = Plan::new();
        plan.add_act(act);
        plan.add_leg(leg);
        let mut agent = Agent::new(id, plan);
        agent.advance_plan();

        agent
    }
}
