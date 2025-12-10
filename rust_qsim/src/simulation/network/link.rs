use crate::simulation::agents::SimulationAgentLogic;
use crate::simulation::config;
use crate::simulation::controller::ThreadLocalComputationalEnvironment;
use crate::simulation::events::{
    VehicleEntersTrafficEventBuilder, VehicleLeavesTrafficEventBuilder,
};
use crate::simulation::id::Id;
use crate::simulation::network::flow_cap::Flowcap;
use crate::simulation::network::storage_cap::StorageCap;
use crate::simulation::network::stuck_timer::StuckTimer;
use crate::simulation::network::Link;
use crate::simulation::network::Node;
use crate::simulation::time_queue::Identifiable;
use crate::simulation::vehicles::InternalVehicle;
use std::collections::VecDeque;
use std::fmt::Debug;

pub enum LinkPosition {
    QStart,
    Waiting,
}

#[derive(Debug)]
pub enum SimLink {
    Local(LocalLink),
    In(SplitInLink),
    Out(SplitOutLink),
}

impl SimLink {
    pub fn id(&self) -> &Id<Link> {
        match self {
            SimLink::Local(ll) => &ll.id,
            SimLink::In(il) => &il.local_link.id,
            SimLink::Out(ol) => &ol.id,
        }
    }

    pub fn from(&self) -> &Id<Node> {
        match self {
            SimLink::Local(l) => l.from(),
            SimLink::In(l) => l.local_link.from(),
            SimLink::Out(_) => {
                panic!("There is no from_id of a split out link.")
            }
        }
    }

    pub fn to(&self) -> &Id<Node> {
        match self {
            SimLink::Local(l) => l.to(),
            SimLink::In(l) => l.local_link.to(),
            SimLink::Out(_) => {
                panic!("There is no from_id of a split out link.")
            }
        }
    }

    pub fn neighbor_part(&self) -> u32 {
        match self {
            SimLink::Local(_) => {
                panic!("local links don't have information about neighbor partitions")
            }
            SimLink::In(il) => il.from_part,
            SimLink::Out(ol) => ol.to_part,
        }
    }

    pub fn flow_cap(&self) -> f32 {
        match self {
            SimLink::Local(l) => l.flow_cap.capacity_per_time_step(),
            SimLink::In(il) => il.local_link.flow_cap.capacity_per_time_step(),
            SimLink::Out(_) => {
                panic!("no flow cap for out links")
            }
        }
    }

    pub fn offers_veh(&self, now: u32) -> Option<&InternalVehicle> {
        match self {
            SimLink::Local(ll) => ll.offers_veh(now),
            SimLink::In(il) => il.local_link.offers_veh(now),
            SimLink::Out(_) => {
                panic!("can't query out links to offer vehicles.")
            }
        }
    }

    pub fn is_veh_stuck(&self, now: u32) -> bool {
        match self {
            SimLink::Local(ll) => ll.stuck_timer.is_stuck(now),
            SimLink::In(il) => il.local_link.stuck_timer.is_stuck(now),
            SimLink::Out(_) => {
                panic!("Out links don't offer vehicles. ")
            }
        }
    }

    pub fn is_available(&self) -> bool {
        match self {
            SimLink::Local(ll) => ll.is_available(),
            SimLink::In(_) => {
                panic!("In Links can't accept vehicles")
            }
            SimLink::Out(ol) => ol.storage_cap.is_available(),
        }
    }

    pub(super) fn is_active(&self) -> bool {
        match self {
            SimLink::Local(ll) => ll.is_active(),
            SimLink::In(il) => il.local_link.is_active(),
            SimLink::Out(o) => {
                panic!(
                    "Trying to check whether out link {} is active. This is not possible.",
                    o.id
                )
            }
        }
    }

    #[cfg(test)]
    pub fn used_storage(&self) -> f32 {
        match self {
            SimLink::Local(ll) => ll.storage_cap.used(),
            SimLink::In(il) => il.local_link.storage_cap.used(),
            SimLink::Out(ol) => ol.storage_cap.used(),
        }
    }

    pub(super) fn push_veh(&mut self, vehicle: InternalVehicle, position: LinkPosition, now: u32) {
        match self {
            SimLink::Local(l) => l.push_veh(vehicle, now, position),
            SimLink::In(il) => il.local_link.push_veh(vehicle, now, position),
            SimLink::Out(ol) => ol.push_veh(vehicle, position),
        }
    }

    pub fn pop_veh(&mut self) -> Option<InternalVehicle> {
        match self {
            SimLink::Local(ll) => ll.pop_veh(),
            SimLink::In(il) => il.local_link.pop_veh(),
            SimLink::Out(_) => {
                panic!("Can't pop vehicle from out link")
            }
        }
    }
}

#[derive(Debug)]
pub struct LocalLink {
    pub id: Id<Link>,
    q: VecDeque<VehicleQEntry>,
    buffer: VecDeque<InternalVehicle>,
    waiting_list: VecDeque<InternalVehicle>,
    length: f64,
    free_speed: f32,
    storage_cap: StorageCap,
    flow_cap: Flowcap,
    stuck_timer: StuckTimer,
    pub from: Id<Node>,
    pub to: Id<Node>,
}

#[derive(Debug)]
struct VehicleQEntry {
    vehicle: InternalVehicle,
    earliest_exit_time: u32,
}

impl LocalLink {
    pub fn from_link(link: &Link, effective_cell_size: f32, config: &config::Simulation) -> Self {
        LocalLink::build(
            link.id.clone(),
            link.capacity,
            link.freespeed,
            link.permlanes,
            link.length,
            effective_cell_size,
            config,
            link.from.clone(),
            link.to.clone(),
        )
    }

    pub fn new_with_defaults(id: Id<Link>, from: Id<Node>, to: Id<Node>) -> Self {
        LocalLink {
            id,
            q: VecDeque::new(),
            buffer: VecDeque::new(),
            waiting_list: VecDeque::new(),
            length: 1.0,
            free_speed: 1.0,
            storage_cap: StorageCap::build(0., 1., 1., 1.0, 7.5),
            flow_cap: Flowcap::new(3600., 1.0),
            stuck_timer: StuckTimer::new(u32::MAX),
            from,
            to,
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        id: Id<Link>,
        capacity_h: f32,
        free_speed: f32,
        perm_lanes: f32,
        length: f64,
        effective_cell_size: f32,
        config: &config::Simulation,
        from: Id<Node>,
        to: Id<Node>,
    ) -> Self {
        let storage_cap = StorageCap::build(
            length,
            perm_lanes,
            capacity_h,
            config.sample_size,
            effective_cell_size,
        );

        LocalLink {
            id,
            q: VecDeque::new(),
            buffer: VecDeque::new(),
            waiting_list: VecDeque::new(),
            length,
            free_speed,
            storage_cap,
            flow_cap: Flowcap::new(capacity_h, config.sample_size),
            stuck_timer: StuckTimer::new(config.stuck_threshold),
            from,
            to,
        }
    }

    pub fn push_veh(&mut self, vehicle: InternalVehicle, now: u32, position: LinkPosition) {
        match position {
            LinkPosition::QStart => self.push_veh_to_queue(vehicle, now),
            LinkPosition::Waiting => self.push_veh_to_waiting_list(vehicle),
        }
    }

    fn push_veh_to_queue(&mut self, vehicle: InternalVehicle, now: u32) {
        let speed = self.free_speed.min(vehicle.max_v);
        let duration = 1.max((self.length / speed as f64) as u32); // at least 1 second per link
        let earliest_exit_time = now + duration;

        // update state
        self.storage_cap.consume(vehicle.pce);
        self.q.push_back(VehicleQEntry {
            vehicle,
            earliest_exit_time,
        });
    }

    /// Push a vehicle into the waiting list.
    pub fn push_veh_to_waiting_list(&mut self, vehicle: InternalVehicle) {
        self.waiting_list.push_back(vehicle);
    }

    /// This method fills the buffer from two sources with priority:
    /// 1. Check if there are vehicles in the waiting list and move them to the buffer.
    /// 2. Check if there are vehicles in the queue that have reached their earliest exit time and move them to the buffer.
    ///
    /// Both is done only if the flow capacity allows this.
    ///
    /// Returns the vehicles that end their leg on the link
    pub fn do_sim_step(
        &mut self,
        now: u32,
        comp_env: &mut ThreadLocalComputationalEnvironment,
    ) -> Vec<InternalVehicle> {
        self.update_flow_cap(now);
        let mut ending_vehicles = self.add_waiting_to_buffer(comp_env, now);
        ending_vehicles.append(&mut self.add_queue_to_buffer(now));

        for v in &ending_vehicles {
            comp_env.events_publisher_borrow_mut().publish_event(
                &VehicleLeavesTrafficEventBuilder::default()
                    .vehicle(v.id.clone())
                    .link(self.id.clone())
                    .driver(v.driver().id().clone())
                    .time(now)
                    .mode(v.driver().curr_leg().mode.clone())
                    .build()
                    .unwrap(),
            );
        }

        ending_vehicles
    }

    fn add_queue_to_buffer(&mut self, now: u32) -> Vec<InternalVehicle> {
        let mut released_vehicles = vec![];

        loop {
            let option = self.q.front();

            // If queue is empty, break the loop.
            if option.is_none() {
                break;
            }

            let veh = option.unwrap();

            let arrive = veh
                .vehicle
                .driver
                .as_ref()
                .unwrap()
                .is_wanting_to_arrive_on_current_link();
            let capacity_left = self.has_flow_capacity_left(&veh.vehicle);
            let exit = veh.earliest_exit_time <= now;

            // If the earliest exit time has not passed, nothing to do
            if !exit {
                break;
            }

            // If the vehicle wants to arrive, remove it from the queue
            if arrive {
                let veh = self.q.pop_front().unwrap().vehicle;
                self.storage_cap.release(veh.pce);
                released_vehicles.push(veh);
                continue;
            }

            // If the vehicle wants to move to another link, put it into buffer
            if capacity_left {
                let veh = self.q.pop_front().unwrap().vehicle;
                self.storage_cap.release(veh.pce);
                self.buffer.push_back(veh);
            } else {
                break;
            }
        }

        released_vehicles
    }

    fn add_waiting_to_buffer(
        &mut self,
        comp_env: &mut ThreadLocalComputationalEnvironment,
        now: u32,
    ) -> Vec<InternalVehicle> {
        let mut released_vehicles = vec![];

        loop {
            let option = self.waiting_list.front();

            // If waiting list is empty, break the loop.
            if option.is_none() {
                break;
            }

            // If arrival on link, remove from waiting list and put into buffer
            if option
                .unwrap()
                .driver
                .as_ref()
                .unwrap()
                .is_wanting_to_arrive_on_current_link()
            {
                released_vehicles.push(self.pop_from_waiting(comp_env, now));
                continue;
            }

            // If not arriving on link, check if flow capacity allows to move vehicle to buffer
            if self.is_accepting_from_wait(option.unwrap()) {
                let vehicle = self.pop_from_waiting(comp_env, now);
                self.buffer.push_back(vehicle);
            } else {
                break;
            }
        }

        released_vehicles
    }

    fn pop_from_waiting(
        &mut self,
        comp_env: &mut ThreadLocalComputationalEnvironment,
        now: u32,
    ) -> InternalVehicle {
        let vehicle = self.waiting_list.pop_front().unwrap();
        comp_env.events_publisher_borrow_mut().publish_event(
            &VehicleEntersTrafficEventBuilder::default()
                .vehicle(vehicle.id.clone())
                .link(self.id.clone())
                .driver(vehicle.driver().id().clone())
                .time(now)
                .mode(vehicle.driver().curr_leg().mode.clone())
                .build()
                .unwrap(),
        );
        vehicle
    }

    fn is_accepting_from_wait(&self, veh: &InternalVehicle) -> bool {
        self.has_flow_capacity_left(veh)
    }

    fn has_flow_capacity_left(&self, _veh: &InternalVehicle) -> bool {
        let buffer_cap = self.buffer.iter().map(|v| v.pce).sum::<f32>();
        self.flow_cap.value() - buffer_cap > 0.0
    }

    /// This method returns the next/first vehicle from the buffer and removes it from the buffer.
    fn pop_veh(&mut self) -> Option<InternalVehicle> {
        if let Some(veh) = self.buffer.pop_front() {
            // self.storage_cap.release(veh.pce);
            self.flow_cap.consume(veh.pce);
            self.stuck_timer.reset();
            return Some(veh);
        }
        None
    }

    fn update_flow_cap(&mut self, now: u32) {
        // increase flow cap if new time step
        self.flow_cap.update_capacity(now);
    }

    /// This method returns the next vehicle allowed to leave the connection and checks
    /// whether flow capacity is available.
    fn offers_veh(&self, now: u32) -> Option<&InternalVehicle> {
        if let Some(entry) = self.buffer.front() {
            if self.flow_cap.has_capacity_left() {
                self.stuck_timer.start(now);
                return Some(entry);
            }
        }

        None
    }

    #[cfg(test)]
    pub(super) fn veh_count(&self) -> usize {
        self.q.len() + self.waiting_list.len() + self.buffer.len()
    }

    pub fn is_available(&self) -> bool {
        self.storage_cap.is_available()
    }

    /// A link is active, if either the queue, waiting_list or buffer is not empty.
    pub(super) fn is_active(&self) -> bool {
        !self.q.is_empty() || !self.waiting_list.is_empty() || !self.buffer.is_empty()
    }

    fn from(&self) -> &Id<Node> {
        &self.from
    }

    fn to(&self) -> &Id<Node> {
        &self.to
    }

    pub fn to_nodes_active(&self, now: u32) -> bool {
        // the node will only look at the vehicle at the at the top of the queue in the next timestep
        // therefore, peek whether vehicles are available for the next timestep.
        self.offers_veh(now + 1).is_some()
    }
}

#[derive(Debug)]
pub struct SplitOutLink {
    pub id: Id<Link>,
    pub to_part: u32,
    q: VecDeque<InternalVehicle>,
    storage_cap: StorageCap,
}

impl SplitOutLink {
    pub fn new(
        link: &Link,
        effective_cell_size: f32,
        sample_size: f32,
        to_part: u32,
    ) -> SplitOutLink {
        let storage_cap = StorageCap::build(
            link.length,
            link.permlanes,
            link.capacity,
            sample_size,
            effective_cell_size,
        );

        SplitOutLink {
            id: link.id.clone(),
            to_part,
            q: VecDeque::default(),
            storage_cap,
        }
    }

    pub fn apply_storage_cap_update(&mut self, released: f32) {
        self.storage_cap.consume(-released);
    }

    pub fn take_veh(&mut self) -> VecDeque<InternalVehicle> {
        std::mem::take(&mut self.q)
    }

    pub fn push_veh(&mut self, veh: InternalVehicle, position: LinkPosition) {
        match position {
            LinkPosition::QStart => {}
            LinkPosition::Waiting => {
                panic!(
                    "SplitOutLink {} cannot push vehicle {:?} into the buffer.",
                    self.id, veh
                )
            }
        }
        self.storage_cap.consume(veh.pce);
        self.q.push_back(veh);
    }
}

#[derive(Debug)]
pub struct SplitInLink {
    pub from_part: u32,
    pub local_link: LocalLink,
}

impl SplitInLink {
    pub(super) fn new(from_part: u32, local_link: LocalLink) -> Self {
        SplitInLink {
            from_part,
            local_link,
        }
    }

    pub(super) fn occupied_storage(&self) -> f32 {
        self.local_link.storage_cap.used()
    }
}

#[cfg(test)]
mod sim_link_tests {
    use crate::simulation::config;
    use crate::simulation::id::Id;
    use crate::simulation::network::link::LinkPosition::QStart;
    use crate::simulation::network::link::{LocalLink, SimLink};
    use crate::simulation::vehicles::InternalVehicle;
    use crate::test_utils;
    use crate::test_utils::create_agent_without_route;
    use assert_approx_eq::assert_approx_eq;
    use macros::integration_test;

    #[integration_test]
    fn storage_cap_consumed() {
        let mut link = SimLink::Local(LocalLink::build(
            Id::create("0"),
            3600.,
            10.,
            3.,
            100.,
            7.5,
            &test_utils::config(),
            Id::create("0"),
            Id::create("0"),
        ));
        let agent = create_agent_without_route(1);
        let vehicle = InternalVehicle::new(1, 0, 10., 1.5, Some(agent));

        link.push_veh(vehicle, QStart, 0);

        // storage capacity should be consumed immediately. The expected value is max_storage_cap - pce of the vehicle
        assert_eq!(1.5, link.used_storage())
    }

    #[integration_test]
    fn storage_cap_released() {
        let mut link = SimLink::Local(LocalLink::build(
            Id::create("0"),
            3600.,
            10.,
            3.,
            10.,
            7.5,
            &test_utils::config(),
            Id::create("0"),
            Id::create("0"),
        ));
        let agent = create_agent_without_route(1);
        let vehicle = InternalVehicle::new(1, 0, 10., 1.5, Some(agent));

        link.push_veh(vehicle, QStart, 0);

        // After pushing, storage is 1.5
        assert_eq!(1.5, link.used_storage());

        let SimLink::Local(l) = &mut link else {
            unreachable!()
        };

        l.do_sim_step(1, &mut Default::default());
        let _vehicle = link.pop_veh().unwrap();

        // After popping, storage is 0.
        assert_eq!(0., link.used_storage());
    }

    #[integration_test]
    fn flow_cap_accumulates() {
        let mut link = SimLink::Local(LocalLink::build(
            Id::create("0"),
            360.,
            10.,
            3.,
            100.,
            7.5,
            &test_utils::config(),
            Id::create("0"),
            Id::create("0"),
        ));

        let agent1 = create_agent_without_route(1);
        let vehicle1 = InternalVehicle::new(1, 0, 10., 1.5, Some(agent1));
        let agent2 = create_agent_without_route(2);
        let vehicle2 = InternalVehicle::new(2, 0, 10., 1.5, Some(agent2));

        link.push_veh(vehicle1, QStart, 0);
        link.push_veh(vehicle2, QStart, 0);

        let SimLink::Local(l) = &mut link else {
            unreachable!()
        };

        l.do_sim_step(10, &mut Default::default());

        // this should reduce the flow capacity, so that no other vehicle can leave during this time step
        let popped1 = l.pop_veh().unwrap();
        assert_eq!("1", popped1.id.external());

        // as the flow cap is 0.1/s the next vehicle can leave the link 15s after the first
        for now in 11..24 {
            l.do_sim_step(now, &mut Default::default());
            assert!(l.offers_veh(now).is_none());
        }
        l.do_sim_step(25, &mut Default::default());

        if let Some(popped2) = link.offers_veh(25) {
            assert_eq!("2", popped2.id.external());
        } else {
            panic!("Expected vehicle2 to be available at t=30")
        }
    }

    #[integration_test]
    fn calculates_exit_time() {
        let mut link = SimLink::Local(LocalLink::build(
            Id::create("0"),
            3600.,
            10.,
            3.,
            100.,
            7.5,
            &test_utils::config(),
            Id::create("0"),
            Id::create("0"),
        ));

        let agent1 = create_agent_without_route(1);
        let vehicle1 = InternalVehicle::new(1, 0, 10., 1.5, Some(agent1));

        link.push_veh(vehicle1, QStart, 0);

        // this is also implicitly tested above, but we'll do it here again, so that we have descriptive
        // test naming
        for now in 0..9 {
            let SimLink::Local(l) = &mut link else {
                unreachable!()
            };
            l.do_sim_step(now, &mut Default::default());
            assert!(link.offers_veh(now).is_none());
        }

        let SimLink::Local(l) = &mut link else {
            unreachable!()
        };
        l.do_sim_step(10, &mut Default::default());
        assert!(link.offers_veh(10).is_some())
    }

    #[integration_test]
    fn fifo_ordering() {
        let id1 = 42;
        let id2 = 43;
        let mut link = SimLink::Local(LocalLink::build(
            Id::create("1"),
            1.,
            1.,
            1.,
            15.0,
            10.0,
            &test_utils::config(),
            Id::create("0"),
            Id::create("0"),
        ));

        let agent1 = create_agent_without_route(1);
        let vehicle1 = InternalVehicle::new(id1, 0, 10., 1., Some(agent1));
        let agent2 = create_agent_without_route(1);
        let vehicle2 = InternalVehicle::new(id2, 0, 10., 1., Some(agent2));

        link.push_veh(vehicle1, QStart, 0);
        assert_approx_eq!(1., link.used_storage());
        assert!(link.is_available());

        link.push_veh(vehicle2, QStart, 0);
        assert_approx_eq!(2.0, link.used_storage());
        assert!(!link.is_available());

        let SimLink::Local(l) = &mut link else {
            unreachable!()
        };
        l.do_sim_step(15, &mut Default::default());

        // First vehicle pops after 15 s
        let popped_vehicle1 = l.pop_veh().unwrap();
        assert_eq!(id1.to_string(), popped_vehicle1.id.external());

        l.do_sim_step(3614, &mut Default::default());
        assert_eq!(None, l.pop_veh());

        // Second vehicle pops after 3615 s
        l.do_sim_step(3615, &mut Default::default());
        let popped_vehicle2 = link.pop_veh().unwrap();
        assert_eq!(id2.to_string(), popped_vehicle2.id.external());
    }

    #[integration_test]
    pub fn stuck_time() {
        let stuck_threshold = 10;
        let config = config::Simulation {
            start_time: 0,
            end_time: 0,
            sample_size: 1.0,
            stuck_threshold,
            main_modes: vec![],
        };
        let mut link = SimLink::Local(LocalLink::build(
            Id::create("stuck-link"),
            1.,
            1.,
            1.0,
            10.0,
            7.5,
            &config,
            Id::create("from-node"),
            Id::create("to-node"),
        ));

        let vehicle = InternalVehicle::new(1, 0, 10., 1., Some(create_agent_without_route(1)));
        link.push_veh(vehicle, QStart, 0);

        // earliest exit is at 10. Therefore this call should not trigger the stuck timer
        let SimLink::Local(l) = &mut link else {
            unreachable!()
        };
        l.do_sim_step(9, &mut Default::default());
        let offers = l.offers_veh(9);
        assert!(offers.is_none());
        assert!(!l.stuck_timer.is_stuck(9));

        // this should trigger the stuck timer
        let expected_timer_start = 10;
        l.do_sim_step(expected_timer_start, &mut Default::default());
        let offers = l.offers_veh(expected_timer_start);
        assert!(offers.is_some());
        assert!(!l
            .stuck_timer
            .is_stuck(expected_timer_start + stuck_threshold - 1));
        assert!(l
            .stuck_timer
            .is_stuck(expected_timer_start + stuck_threshold));
    }

    #[integration_test]
    pub fn stuck_time_reset() {
        let stuck_threshold = 10;
        let earliest_exit: u32 = 10;
        let config = config::Simulation {
            start_time: 0,
            end_time: 0,
            sample_size: 1.0,
            stuck_threshold,
            main_modes: vec![],
        };
        let mut link = SimLink::Local(LocalLink::build(
            Id::create("stuck-link"),
            36000.,
            1.,
            1.0,
            earliest_exit as f64,
            7.5,
            &config,
            Id::create("from-node"),
            Id::create("to-node"),
        ));

        let vehicle1 = InternalVehicle::new(
            1,
            0,
            earliest_exit as f32,
            1.,
            Some(create_agent_without_route(1)),
        );
        let vehicle2 = InternalVehicle::new(
            2,
            0,
            earliest_exit as f32,
            1.,
            Some(create_agent_without_route(2)),
        );
        link.push_veh(vehicle1, QStart, 0);
        link.push_veh(vehicle2, QStart, 0);

        let SimLink::Local(l) = &mut link else {
            unreachable!()
        };

        // trigger stuck timer
        l.do_sim_step(earliest_exit, &mut Default::default());
        assert!(l.offers_veh(earliest_exit).is_some());
        // check that stuck timer works as expected
        let now = earliest_exit + stuck_threshold;
        assert!(l.stuck_timer.is_stuck(now));
        // fetch the stuck vehicle, which should reset the timer, so that the next veh is not stuck
        let _ = l.pop_veh();
        assert!(!l.stuck_timer.is_stuck(now));
        // the next vehicle should be ready to leave the link as well.
        // This call should trigger the stuck timer again.
        assert!(l.offers_veh(now).is_some());
        let now = now + stuck_threshold;
        assert!(!l.stuck_timer.is_stuck(now - 1));
        assert!(l.stuck_timer.is_stuck(now));
    }
}

#[cfg(test)]
mod out_link_tests {
    use crate::simulation::id::Id;
    use crate::simulation::network::link::LinkPosition::QStart;
    use crate::simulation::network::link::{SimLink, SplitOutLink};
    use crate::simulation::network::storage_cap::StorageCap;
    use crate::simulation::vehicles::InternalVehicle;
    use crate::test_utils::create_agent_without_route;
    use macros::integration_test;

    #[integration_test]
    fn push_and_take() {
        let mut link = SimLink::Out(SplitOutLink {
            id: Id::new_internal(0),
            to_part: 1,
            q: Default::default(),
            storage_cap: StorageCap::build(100., 1., 1., 1., 1.),
        });
        let id1 = 42;
        let id2 = 43;
        let agent1 = create_agent_without_route(1);
        let vehicle1 = InternalVehicle::new(id1, 0, 10., 1., Some(agent1));
        let agent2 = create_agent_without_route(1);
        let vehicle2 = InternalVehicle::new(id2, 0, 10., 1., Some(agent2));

        link.push_veh(vehicle1, QStart, 0);
        link.push_veh(vehicle2, QStart, 0);

        // storage should be consumed
        assert_eq!(2., link.used_storage());

        if let SimLink::Out(ref mut ol) = link {
            let mut result = ol.take_veh();

            // make sure, that vehicles have correct order
            assert_eq!(2, result.len());
            let taken_1 = result.pop_front().unwrap();
            assert_eq!(id1.to_string(), taken_1.id.external());
            let taken_2 = result.pop_front().unwrap();
            assert_eq!(id2.to_string(), taken_2.id.external());

            // make sure storage capacity is not released
            assert_eq!(2., link.used_storage());
        } else {
            panic!("expected out link")
        }
    }

    #[integration_test]
    fn update_storage_caps() {
        // set up the link, so that we consume two units of storage.
        let mut cap = StorageCap::build(100., 1., 1., 1., 1.);
        cap.consume(2.);
        let mut out_link = SplitOutLink {
            id: Id::new_internal(0),
            to_part: 1,
            q: Default::default(),
            storage_cap: cap,
        };

        assert_eq!(2., out_link.storage_cap.used());
        out_link.apply_storage_cap_update(2.);

        assert_eq!(0., out_link.storage_cap.used());
    }
}
