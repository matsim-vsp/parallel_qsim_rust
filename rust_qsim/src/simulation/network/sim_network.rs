use super::{
    link::{LocalLink, SimLink, SplitInLink, SplitOutLink},
    Link, Network, Node,
};
use crate::simulation::agents::{AgentEvent, EnvironmentalEventObserver, SimulationAgentLogic};
use crate::simulation::config;
use crate::simulation::controller::ThreadLocalComputationalEnvironment;
use crate::simulation::events::{EventsManager, LinkEnterEventBuilder, LinkLeaveEventBuilder};
use crate::simulation::id::serializable_type::StableTypeId;
use crate::simulation::id::Id;
use crate::simulation::network::link::LinkPosition::{QStart, Waiting};
use crate::simulation::vehicles::InternalVehicle;
use nohash_hasher::{IntMap, IntSet};
use rand::rngs::ThreadRng;
use rand::{rng, Rng};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub struct StorageUpdate {
    pub link_id: Id<Link>,
    pub from_part: u32,
    pub released: f32,
}

#[derive(Debug)]
struct ActiveCache<C: StableTypeId> {
    active: IntSet<Id<C>>,
}

impl<C: StableTypeId> Default for ActiveCache<C> {
    fn default() -> Self {
        ActiveCache {
            active: IntSet::default(),
        }
    }
}

impl<C: StableTypeId> From<IntSet<Id<C>>> for ActiveCache<C> {
    fn from(value: IntSet<Id<C>>) -> Self {
        ActiveCache { active: value }
    }
}

impl<C: StableTypeId + 'static> ActiveCache<C> {
    fn activate(&mut self, id: Id<C>) -> bool {
        self.active.insert(id)
    }

    fn deactivate(&mut self, id: &Id<C>) -> bool {
        self.active.remove(id)
    }

    fn len(&self) -> usize {
        self.active.len()
    }

    fn contains(&self, id: &Id<C>) -> bool {
        self.active.contains(id)
    }
}

impl<'a, C: StableTypeId + 'static> IntoIterator for &'a ActiveCache<C> {
    type Item = &'a Id<C>;
    type IntoIter = <&'a IntSet<Id<C>> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.active.iter()
    }
}

#[derive(Debug)]
pub struct SimNetworkPartition {
    pub nodes: IntMap<Id<Node>, SimNode>,
    // use int map as hash map variant with stable order
    pub links: IntMap<Id<Link>, SimLink>,
    rnd: ThreadRng,
    active_nodes: ActiveCache<Node>,
    active_links: ActiveCache<Link>,
    veh_counter: usize,
    partition: u32,
}

#[derive(Debug)]
#[allow(unused)]
pub struct SimNode {
    id: Id<Node>,
    in_links: Vec<Id<Link>>,
}

#[derive(Debug)]
pub struct SimNetworkPartitionBuilder {
    pub(crate) nodes: IntMap<Id<Node>, SimNode>,
    pub(crate) links: IntMap<Id<Link>, SimLink>,
    partition: u32,
}

impl From<SimNetworkPartitionBuilder> for SimNetworkPartition {
    fn from(value: SimNetworkPartitionBuilder) -> Self {
        SimNetworkPartition::new(value.nodes, value.links, value.partition)
    }
}

impl SimNetworkPartitionBuilder {
    pub fn from_network(
        global_network: &Network,
        partition: u32,
        config: &config::Simulation,
    ) -> Self {
        let nodes: Vec<&Node> = global_network
            .nodes()
            .iter()
            .filter(|n| n.partition == partition)
            .copied()
            .collect();

        let link_ids: Vec<_> = nodes
            .iter()
            .flat_map(|n| n.in_links.iter().chain(n.out_links.iter()))
            .collect(); // collect here to get each link id only once

        let sim_links: IntMap<_, _> = link_ids
            .iter()
            .map(|link_id| global_network.get_link(link_id))
            .map(|link| {
                (
                    link.id.clone(),
                    Self::create_sim_link(
                        link,
                        partition,
                        global_network.effective_cell_size(),
                        &config,
                        global_network,
                    ),
                )
            })
            .collect();

        let sim_nodes: IntMap<_, SimNode> = nodes
            .iter()
            .map(|n| (n.id.clone(), Self::create_sim_node(n)))
            .collect();

        Self {
            nodes: sim_nodes,
            links: sim_links,
            partition,
        }
    }

    pub fn build(self) -> SimNetworkPartition {
        SimNetworkPartition::new(self.nodes, self.links, self.partition)
    }

    fn create_sim_node(node: &Node) -> SimNode {
        let in_links: Vec<_> = node.in_links.to_vec();

        SimNode {
            id: node.id.clone(),
            in_links,
        }
    }

    fn create_sim_link(
        link: &Link,
        partition: u32,
        effective_cell_size: f32,
        config: &config::Simulation,
        global_network: &Network,
    ) -> SimLink {
        let from_part = global_network.get_node(&link.from).partition; //all_nodes.get(link.from.internal()).unwrap().partition;
        let to_part = global_network.get_node(&link.to).partition; //all_nodes.get(link.to.internal()).unwrap().partition;

        if from_part == to_part {
            SimLink::Local(LocalLink::from_link(link, effective_cell_size, config))
        } else if to_part == partition {
            let local_link = LocalLink::from_link(link, effective_cell_size, config);
            SimLink::In(SplitInLink::new(from_part, local_link))
        } else {
            SimLink::Out(SplitOutLink::new(
                link,
                effective_cell_size,
                config.sample_size,
                to_part,
            ))
        }
    }
}

impl SimNetworkPartition {
    fn new(
        nodes: IntMap<Id<Node>, SimNode>,
        links: IntMap<Id<Link>, SimLink>,
        partition: u32,
    ) -> Self {
        Self {
            nodes,
            links,
            rnd: rng(),
            active_links: ActiveCache::<Link>::default(),
            active_nodes: ActiveCache::<Node>::default(),
            veh_counter: 0,
            partition,
        }
    }

    pub fn partition(&self) -> u32 {
        self.partition
    }

    pub fn neighbors(&self) -> IntSet<u32> {
        let distinct_partitions: IntSet<u32> = self
            .links
            .values()
            .filter(|link| match link {
                SimLink::Local(_) => false,
                SimLink::In(_) => true,
                SimLink::Out(_) => true,
            })
            .map(|link| link.neighbor_part())
            .collect();
        distinct_partitions
    }

    pub fn active_nodes(&self) -> usize {
        self.active_nodes.len()
    }

    pub fn active_links(&self) -> usize {
        self.active_links.len()
    }

    pub fn veh_on_net(&self) -> usize {
        self.veh_counter
    }

    pub fn get_link_ids(&self) -> HashSet<Id<Link>> {
        self.links
            .iter()
            .filter(|(_, link)| match link {
                SimLink::Local(_) => true,
                SimLink::In(_) => true,
                SimLink::Out(_) => false,
            })
            .map(|(id, _)| id.clone())
            .collect::<HashSet<_>>()
    }

    pub fn get_node_ids(&self) -> HashSet<Id<Node>> {
        self.nodes.keys().cloned().collect::<HashSet<_>>()
    }

    /// The event publisher is only used to publish link enter events. There are two different cases:
    /// 1. The vehicle is received from another partition. The event publisher should be Some(_) in order to publish the
    ///    link enter event.
    /// 2. The vehicle starts at this partition. Because its link enter is right after an activity,
    ///    the MATSim default is to not publish this link enter event. Therefore, the event publisher should be None.
    pub fn send_veh_en_route(
        &mut self,
        vehicle: InternalVehicle,
        events_publisher: Option<Rc<RefCell<EventsManager>>>,
        now: u32,
    ) {
        let link_id = vehicle.curr_link_id().unwrap_or_else(|| {
            panic!("Vehicle is expected to have a current link id if it is sent onto the network")
        });
        let link = self.links.get_mut(link_id).unwrap_or_else(|| {
            let agent_id = vehicle.id();
            let coming_from_other_partition = events_publisher.is_some();
            let where_is_it_from = if coming_from_other_partition {
                "Vehicle is already en route and comes from another partition."
            } else {
                "Vehicle was just sent en route. This is the first link."
            };
            panic!(
                "#{} Couldn't find link for id {:?}.for Agent {}. {} \n\n The vehicle: {:?}",
                self.partition,
                link_id,
                agent_id.external(),
                where_is_it_from,
                //self.global_network.get_link(&full_id),
                vehicle
            );
        });

        // If events_publisher is None, this is the start of the route and the vehicle goes
        // into the waiting list. `fill_buffer` prioritizes draining waiting_list into buffer.
        let is_route_begin = events_publisher.is_none();

        if let Some(publisher) = events_publisher {
            publisher.borrow_mut().publish_event(
                &LinkEnterEventBuilder::default()
                    .time(now)
                    .link(link.id().clone())
                    .vehicle(vehicle.id.clone())
                    .build()
                    .unwrap(),
            );
        }

        let pos = if is_route_begin { Waiting } else { QStart };
        link.push_veh(vehicle, pos, now);

        self.veh_counter += 1;

        self.active_links.activate(link.id().clone());
    }

    pub fn apply_storage_cap_updates(&mut self, storage_caps: Vec<StorageUpdate>) {
        for cap in storage_caps {
            if let SimLink::Out(link) = self.links.get_mut(&cap.link_id).unwrap() {
                link.apply_storage_cap_update(cap.released);
            } else {
                panic!("only expecting ids for split out links ")
            }
        }
    }

    pub fn move_links(
        &mut self,
        comp_env: &mut ThreadLocalComputationalEnvironment,
        now: u32,
    ) -> MoveAllLinksResult {
        let mut storage_cap_updates: Vec<_> = Vec::new();
        let mut vehicles_exit_partition: Vec<_> = Vec::new();
        let mut deactivate: IntSet<_> = IntSet::default();

        let mut vehicles_end_leg = vec![];
        for id in &self.active_links {
            let link = self.links.get_mut(id).unwrap();
            let mut res = match link {
                SimLink::Local(ll) => {
                    Self::move_local_link(ll, &mut self.active_nodes, now, comp_env)
                }
                SimLink::In(il) => Self::move_in_link(
                    il,
                    &mut self.active_nodes,
                    &mut storage_cap_updates,
                    now,
                    comp_env,
                ),
                SimLink::Out(ol) => Self::move_out_link(ol, &mut vehicles_exit_partition),
            };

            if !res.is_active {
                deactivate.insert(link.id().clone());
            }

            vehicles_end_leg.append(&mut res.vehicles_end_leg);
        }

        // bookkeeping. Empty links are no longer active.
        for id in deactivate {
            self.active_links.deactivate(&id);
        }
        // vehicles leaving this partition are no longer part of the veh count
        self.veh_counter -= vehicles_exit_partition.len();
        self.veh_counter -= vehicles_end_leg.len();

        MoveAllLinksResult {
            vehicles_exit_partition,
            vehicles_end_leg,
            storage_cap_updates,
        }
    }

    fn move_local_link(
        link: &mut LocalLink,
        active_nodes: &mut ActiveCache<Node>,
        now: u32,
        comp_env: &mut ThreadLocalComputationalEnvironment,
    ) -> MoveSingleLinkResult {
        let vehicles_end_leg = link.do_sim_step(now, comp_env);
        if link.to_nodes_active(now) {
            active_nodes.activate(link.to.clone());
        }

        // indicate whether link is active. The link is active if it has vehicles on it.
        let is_active = link.is_active();

        MoveSingleLinkResult {
            vehicles_end_leg,
            is_active,
        }
    }

    fn move_in_link(
        link: &mut SplitInLink,
        active_nodes: &mut ActiveCache<Node>,
        storage_cap_updates: &mut Vec<StorageUpdate>,
        now: u32,
        events: &mut ThreadLocalComputationalEnvironment,
    ) -> MoveSingleLinkResult {
        // if anything has changed on the link, we want to report the updated storage capacity to the
        // upstream partition.
        let before = link.occupied_storage();
        let result = Self::move_local_link(&mut link.local_link, active_nodes, now, events);
        let diff = before - link.occupied_storage();

        assert!(
            diff.partial_cmp(&0.0).unwrap().is_ge(),
            "The occupied storage on link {:?} cannot increase when moving vehicles.",
            link.local_link.id
        );

        if diff > 0. {
            storage_cap_updates.push(StorageUpdate {
                link_id: link.local_link.id.clone(),
                from_part: link.from_part,
                released: diff,
            })
        }

        result
    }

    fn move_out_link(
        link: &mut SplitOutLink,
        vehicles: &mut Vec<InternalVehicle>,
    ) -> MoveSingleLinkResult {
        let out_q = link.take_veh();
        for veh in out_q {
            vehicles.push(veh);
        }
        MoveSingleLinkResult::default()
    }

    pub fn move_nodes(&mut self, comp_env: &mut ThreadLocalComputationalEnvironment, now: u32) {
        let mut deactivate = vec![];
        for n in &self.active_nodes {
            let node = self.nodes.get(n).unwrap();
            let active = Self::move_node_capacity_priority(
                node,
                &mut self.links,
                &mut self.active_links,
                comp_env,
                &mut self.rnd,
                now,
            );
            if !active {
                deactivate.push(n.clone());
            }
        }

        for n in deactivate {
            self.active_nodes.deactivate(&n);
        }
    }

    fn move_node_capacity_priority(
        node: &SimNode,
        links: &mut IntMap<Id<Link>, SimLink>,
        active_links: &mut ActiveCache<Link>,
        comp_env: &mut ThreadLocalComputationalEnvironment,
        rnd: &mut ThreadRng,
        now: u32,
    ) -> bool {
        let (active, mut avail_capacity) =
            Self::get_active_in_links(&node.in_links, active_links, links);
        let mut exhausted_links: Vec<Option<()>> = vec![None; active.len()];
        let mut sel_cap: f32 = 0.;

        while avail_capacity > 1e-10 {
            // draw random number between 0 and available capacity
            let rnd_num: f32 = rnd.random::<f32>() * avail_capacity;

            #[allow(clippy::needless_range_loop)]
            // go through all in links and fetch one, which is not exhausted yet.
            for i in 0..active.len() {
                // if the link is exhausted, try next link
                if exhausted_links[i].is_some() {
                    // reduce the available capacity a little bit. Sometimes we have rounding errors
                    // which will cause an infinite loop. Reducing the remaining capacity a little
                    // bit at least prevents infinite loops.
                    avail_capacity -= 1e-6;
                    continue;
                }

                // take the not exhausted link and check whether it could release a vehicle and if
                // that vehicle can move to the next link
                let link_id = active.get(i).unwrap();
                if Self::should_veh_move_out(link_id, links, now) {
                    // the vehicle can move. Increase the selected capacity by the link's capacity
                    // this way it becomes more and more likely that a link can release vehicles,
                    // links with more capacity are more likely to release vehicles first though.
                    let in_link = links.get_mut(link_id).unwrap();
                    sel_cap += in_link.flow_cap();

                    if sel_cap >= rnd_num {
                        let veh = in_link.pop_veh().expect("No vehicle on link");
                        Self::move_vehicle(veh, links, active_links, comp_env, now);
                    }
                } else {
                    // in case the vehicle on the link can't move, we add the link to the exhausted
                    // bookkeeping and reduce the available capacity, which makes it more likely for
                    // other links to be able to release vehicles.
                    exhausted_links[i] = Some(());
                    let link = links.get(link_id).unwrap();
                    avail_capacity -= link.flow_cap();
                }
            }
        }
        // check whether any link is offering next timestep. Otherwise the node can be de-activated
        Self::any_link_offers(&active, links, now + 1)
    }

    fn get_active_in_links(
        in_links: &Vec<Id<Link>>,
        active_links: &ActiveCache<Link>,
        links: &IntMap<Id<Link>, SimLink>,
    ) -> (Vec<Id<Link>>, f32) {
        let mut active = Vec::new();
        let mut acc_cap = 0.;

        for id in in_links {
            if active_links.contains(id) {
                active.push(id.clone());
                let link = links.get(id).unwrap();
                acc_cap += link.flow_cap();
            }
        }

        (active, acc_cap)
    }

    fn any_link_offers(
        link_ids: &[Id<Link>],
        links: &IntMap<Id<Link>, SimLink>,
        time: u32,
    ) -> bool {
        link_ids
            .iter()
            .map(|id| links.get(id).unwrap())
            .any(|link| link.offers_veh(time).is_some())
    }

    fn should_veh_move_out(in_id: &Id<Link>, links: &IntMap<Id<Link>, SimLink>, now: u32) -> bool {
        let in_link = links.get(in_id).unwrap();
        if let Some(veh_ref) = in_link.offers_veh(now) {
            return if let Some(next_id) = veh_ref.peek_next_route_element() {
                // if the vehicle has a next link id, it should move out of the current link.
                // if the vehicle has reached its stuck threshold, we push it to the next link regardless of the available
                // storage capacity. Under normal conditions, we check whether the downstream link has storage capacity available
                let out_link = links.get(next_id).unwrap_or_else(|| {
                    panic!(
                        "Link id {:?} was not in local network. Vehicle's leg is: {:?}",
                        next_id,
                        veh_ref.driver().curr_leg()
                    )
                });
                in_link.is_veh_stuck(now) || out_link.is_available()
            } else {
                panic!("Vehicle {:?} is offered by link {:?} but has no next link. This should not happen. Leg ends are handled in move_links, not move_nodes.", veh_ref.id(), in_link.id())
            };
        }
        // if the link doesn't have a vehicle to offer, we don't have to do anything.
        false
    }

    /// Moves the vehicle from the current link to the next link.
    fn move_vehicle(
        mut vehicle: InternalVehicle,
        links: &mut IntMap<Id<Link>, SimLink>,
        active_links: &mut ActiveCache<Link>,
        comp_env: &mut ThreadLocalComputationalEnvironment,
        now: u32,
    ) {
        let old_link_id = vehicle.curr_link_id().unwrap().clone();

        comp_env.events_publisher_borrow_mut().publish_event(
            &LinkLeaveEventBuilder::default()
                .vehicle(vehicle.id.clone())
                .link(old_link_id.clone())
                .time(now)
                .build()
                .unwrap(),
        );
        vehicle.notify_event(&mut AgentEvent::LeftLink(), now);
        let new_link_id = vehicle.curr_link_id().unwrap().clone();
        let new_link = links.get_mut(&new_link_id).unwrap();

        // for out links, link enter event is published at receiving partition
        if let SimLink::Local(_) = new_link {
            comp_env.events_publisher_borrow_mut().publish_event(
                &LinkEnterEventBuilder::default()
                    .time(now)
                    .link(new_link.id().clone())
                    .vehicle(vehicle.id.clone())
                    .build()
                    .unwrap(),
            );
        }

        new_link.push_veh(vehicle, QStart, now);

        // activate new link
        active_links.activate(new_link_id.clone());

        // deactivate old link if it is not active anymore
        if !links.get(&old_link_id).unwrap().is_active() {
            active_links.deactivate(&old_link_id);
        }
    }
}

pub struct MoveAllLinksResult {
    pub vehicles_exit_partition: Vec<InternalVehicle>,
    pub vehicles_end_leg: Vec<InternalVehicle>,
    pub storage_cap_updates: Vec<StorageUpdate>,
}

#[derive(Default)]
struct MoveSingleLinkResult {
    vehicles_end_leg: Vec<InternalVehicle>,
    is_active: bool,
}

#[cfg(test)]
mod tests {
    use super::{SimNetworkPartition, SimNetworkPartitionBuilder};
    use crate::simulation::config::{MetisOptions, PartitionMethod};
    use crate::simulation::controller::ThreadLocalComputationalEnvironment;
    use crate::simulation::id::Id;
    use crate::simulation::io::proto::xml_events::XmlEventsWriter;
    use crate::simulation::network::link::LinkPosition::QStart;
    use crate::simulation::network::link::SimLink;
    use crate::simulation::network::link::SimLink::Local;
    use crate::simulation::network::{Link, Network, Node};
    use crate::simulation::vehicles::InternalVehicle;
    use crate::test_utils;
    use assert_approx_eq::assert_approx_eq;
    use macros::integration_test;

    #[integration_test]
    fn from_network() {
        let mut network = Network::new();
        let mut sim_nets = create_three_node_sim_network_with_partition(&mut network);
        let net1 = sim_nets.get_mut(0).unwrap();

        // we expect two nodes
        assert_eq!(2, net1.nodes.len());
        // we expect two links one local and one out link
        assert_eq!(2, net1.links.len());
        let local_link = net1.links.get(&Id::create("link1")).unwrap();
        assert!(matches!(local_link, SimLink::Local(_)));
        let out_link = net1.links.get(&Id::create("link2")).unwrap();
        assert!(matches!(out_link, SimLink::Out(_)));

        let net2 = sim_nets.get_mut(1).unwrap();
        // we expect one node
        assert_eq!(1, net2.nodes.len());
        // we expect one in link
        assert_eq!(1, net2.links.len());
        let in_link = net2.links.get(&Id::create("link2")).unwrap();
        assert!(matches!(in_link, SimLink::In(_)));
    }

    #[integration_test]
    fn vehicle_travels_local() {
        let mut publisher = ThreadLocalComputationalEnvironment::default();
        let register = XmlEventsWriter::register("test_output/test.xml".into());
        register(&mut publisher.events_publisher_borrow_mut());

        let global_net = Network::from_file(
            "./assets/3-links/3-links-network.xml",
            1,
            &PartitionMethod::Metis(MetisOptions::default()),
        );
        let mut network =
            SimNetworkPartitionBuilder::from_network(&global_net, 0, &test_utils::config()).build();
        let agent = test_utils::create_agent(1, vec!["link1", "link2", "link3"]);
        let vehicle = InternalVehicle::new(1, 0, 10., 1., Some(agent));
        network.send_veh_en_route(vehicle, None, 0);

        for i in 0..113 {
            network.move_nodes(&mut publisher, i);
            let result = network.move_links(&mut Default::default(), i);

            // only in the timestep before the vehicle switches links, we should see one active node. Otherwise not.
            // leaves link1 at timestep 0 and enters link1; leaves link1 and enters link2 at timestep 101
            if i == 0 || i == 101 {
                assert_eq!(1, network.active_nodes(), "There was no active node at {i}");
                network.active_nodes.contains(&Id::create("node1"));
            } else {
                assert_eq!(0, network.active_nodes(), "There was an active node at {i}");
            }

            if i == 112 {
                assert!(!result.vehicles_end_leg.is_empty());
                let veh = result.vehicles_end_leg.first().unwrap();
                assert_eq!(&Id::create("link3"), veh.curr_link_id().unwrap());
            } else {
                // the vehicle should not leave the network until the 112th timestep
                assert_eq!(0, result.vehicles_end_leg.len());
                // we should always have one active link which has the vehicle
                assert_eq!(1, network.active_links());
                // we expect one vehicle
                assert_eq!(1, network.veh_on_net());
            }
        }

        // the network should be empty in the end
        assert_eq!(0, network.active_links());
        assert_eq!(0, network.active_nodes());
        assert_eq!(0, network.veh_on_net());
    }

    #[integration_test]
    fn vehicle_reaches_boundary() {
        let mut publisher = Default::default();
        let global_net = Network::from_file(
            "./assets/3-links/3-links-network.xml",
            2,
            &PartitionMethod::None,
        );
        let mut network =
            SimNetworkPartitionBuilder::from_network(&global_net, 0, &test_utils::config()).build();
        let agent = test_utils::create_agent(1, vec!["link1", "link2", "link3"]);
        let vehicle = InternalVehicle::new(1, 0, 10., 100., Some(agent));
        network.send_veh_en_route(vehicle, None, 0);

        for now in 0..20 {
            network.move_nodes(&mut publisher, now);

            let res = network.move_links(&mut publisher, now);
            assert_eq!(0, res.storage_cap_updates.len()); // we expect no out links here

            assert_eq!(0, res.vehicles_end_leg.len());

            // when the vehicle moves from link1 to link2, it will be placed on an out link.
            // the stored vehicles on out links should be collected during move links.
            if now == 1 {
                assert_eq!(1, res.vehicles_exit_partition.len());
            } else {
                assert!(
                    res.vehicles_exit_partition.is_empty(),
                    "There should be no vehicles on the out link at timestep {now}"
                );
            }
        }
    }

    #[integration_test]
    fn move_nodes_enter_exit_constraint() {
        let mut publisher = Default::default();
        let global_net = Network::from_file(
            "./assets/3-links/3-links-network.xml",
            1,
            &PartitionMethod::Metis(MetisOptions::default()),
        );
        let mut network =
            SimNetworkPartitionBuilder::from_network(&global_net, 0, &test_utils::config()).build();

        // place 100 vehicles on first link
        for i in 0..100 {
            let agent = test_utils::create_agent(i, vec!["link1"]);
            let vehicle = InternalVehicle::new(i, 0, 10., 1., Some(agent));
            network.send_veh_en_route(vehicle, None, 0);
        }

        // all vehicles only have to traverse link1. they enter and directly exit
        for now in 0..2 {
            network.move_nodes(&mut publisher, now);
            let res = network.move_links(&mut publisher, now);
            if now == 0 {
                assert_eq!(100, res.vehicles_end_leg.len());
            } else {
                assert_eq!(0, res.vehicles_end_leg.len());
            }
        }
    }

    /// Test that vehicles are released from out links in case there is no stuck timer.
    #[integration_test]
    fn move_nodes_storage_cap_constraint() {
        let mut publisher = ThreadLocalComputationalEnvironment::default();
        let mut global_net = Network::from_file(
            "./assets/3-links/3-links-network.xml",
            1,
            &PartitionMethod::Metis(MetisOptions::default()),
        );
        global_net.set_effective_cell_size(10.);

        let id_1: Id<Link> = Id::get_from_ext("link1");
        let id_2: Id<Link> = Id::get_from_ext("link2");
        let id_3: Id<Link> = Id::get_from_ext("link3");
        let mut config = test_utils::config();
        config.stuck_threshold = u32::MAX;
        let mut network = SimNetworkPartitionBuilder::from_network(&global_net, 0, &config).build();

        // Place 10 vehicles on link1. They will be released every 10s because PCE is 10 and flow_cap is 1.
        // Since they are super slow, they will leave link2 after 1000s.
        // They will traverse link3 in 100 s, after that they will leave link3. Since link3 has a storage cap of 10 and
        // PCE of the vehicle is 10 only one vehicle per time can be present on link3.
        for i in 0..10 {
            let agent =
                test_utils::create_agent(i, vec![id_1.external(), id_2.external(), "link3"]);
            let vehicle = InternalVehicle::new(i, 0, 1., 10., Some(agent));
            network.send_veh_en_route(vehicle, None, 0);
        }

        for now in 0..2012 {
            network.move_nodes(&mut publisher, now);
            network.move_links(&mut publisher, now);

            let link1 = network.links.get(&id_1).unwrap();
            let link2 = network.links.get(&id_2).unwrap();
            let link3 = network.links.get(&id_3).unwrap();

            // at 10, 20, 30, ... link1 offers a vehicle
            if now < 91 && (0..91).step_by(10).collect::<Vec<u32>>().contains(&now) {
                assert!(
                    link1.offers_veh(now).is_some(),
                    "No vehicle offered at timestep {now}"
                );
            } else {
                assert!(
                    link1.offers_veh(now).is_none(),
                    "Vehicle offered at timestep {now}"
                );
            }

            // From 1002, no vehicle if offered by link1
            if (1002..1911).contains(&now) {
                // once the last vehicle has moved, link1 has nothing to offer.
                assert!(link1.offers_veh(now).is_none());

                // veh0 reaches buffer at 1001 and is released immediately.
                // veh1 reaches buffer at 1011 and is released at 1102; flow cap is refilled after 10
                // veh2 reaches q end at 1021 and buffer at 1112 (because of flow cap refill from before) and is released at 1203; flow cap is refilled after 10
                // ...
                if (1011..=1102).contains(&now)
                    || (1112..=1203).contains(&now)
                    || (1213..=1304).contains(&now)
                    || (1314..=1405).contains(&now)
                    || (1415..=1506).contains(&now)
                    || (1516..=1607).contains(&now)
                    || (1617..=1708).contains(&now)
                    || (1718..=1809).contains(&now)
                    || (1819..=1910).contains(&now)
                {
                    assert!(
                        link2.offers_veh(now).is_some(),
                        "No vehicle offered at timestep {now}"
                    );
                    if !(now == 1102
                        || now == 1203
                        || now == 1304
                        || now == 1405
                        || now == 1506
                        || now == 1607
                        || now == 1708
                        || now == 1809
                        || now == 1910)
                    {
                        assert!(
                            !link3.is_available(),
                            "Storage cap reached at timestep {now}"
                        );
                    }
                } else {
                    assert!(
                        link2.offers_veh(now).is_none(),
                        "Vehicle offered at timestep {now}"
                    );
                }
            }
        }
    }

    /// Tests that vehicles are released from out links when stuck timer is reached.
    #[integration_test]
    fn move_nodes_stuck_threshold() {
        let mut publisher = ThreadLocalComputationalEnvironment::default();
        XmlEventsWriter::register("test_output/test.xml".into())(
            &mut publisher.events_publisher_borrow_mut(),
        );
        let mut global_net = Network::from_file(
            "./assets/3-links/3-links-network.xml",
            1,
            &PartitionMethod::Metis(MetisOptions::default()),
        );
        global_net.set_effective_cell_size(10.);

        let id_1: Id<Link> = Id::get_from_ext("link1");
        let id_2: Id<Link> = Id::get_from_ext("link2");
        let id_3: Id<Link> = Id::get_from_ext("link3");
        let mut config = test_utils::config();
        config.stuck_threshold = 10;
        let mut network = SimNetworkPartitionBuilder::from_network(&global_net, 0, &config).build();

        // Place 10 vehicles on link1. They will be released every 10s because PCE is 10 and flow_cap is 1.
        // Since they are super slow, they will leave link2 after 1000s.
        // They will traverse link3 in 100 s, after that they will leave link3. Since link3 has a storage cap of 10 and
        // PCE of the vehicle is 10 only one vehicle per time can be present on link3.
        // But, since we enabled the stuck timer, they will be put onto link3 after being stuck for 10s.
        for i in 0..10 {
            let agent =
                test_utils::create_agent(i, vec![id_1.external(), id_2.external(), "link3"]);
            let vehicle = InternalVehicle::new(i, 0, 1., 10., Some(agent));
            network.send_veh_en_route(vehicle, None, 0);
        }

        for now in 0..3300 {
            network.move_nodes(&mut publisher, now);
            network.move_links(&mut publisher, now);

            let link1 = network.links.get(&id_1).unwrap();
            let link2 = network.links.get(&id_2).unwrap();
            let link3 = network.links.get(&id_3).unwrap();

            // at 10, 20, 30, ... link1 offers a vehicle
            if now < 91 && (0..91).step_by(10).collect::<Vec<u32>>().contains(&now) {
                assert!(
                    link1.offers_veh(now).is_some(),
                    "No vehicle offered at timestep {now}"
                );
            } else {
                assert!(
                    link1.offers_veh(now).is_none(),
                    "Vehicle offered at timestep {now}"
                );
            }

            // From 1002, no vehicle if offered by link1
            if (1002..1911).contains(&now) {
                // once the last vehicle has moved, link1 has nothing to offer.
                assert!(link1.offers_veh(now).is_none());

                // veh0 reaches buffer at 1001 and is released immediately.
                // veh1 reaches buffer at 1011 and is released at 1021; flow cap is refilled after 10
                // veh2 reaches q end at 1021 and buffer at 1031 (because of flow cap refill from before) and is released at 1041 (because of stuck timer); flow cap is refilled after 10
                // ...
                if (1011..=1021).contains(&now)
                    || (1031..=1041).contains(&now)
                    || (1051..=1061).contains(&now)
                    || (1071..=1081).contains(&now)
                    || (1091..=1101).contains(&now)
                    || (1111..=1121).contains(&now)
                    || (1131..=1141).contains(&now)
                    || (1151..=1161).contains(&now)
                    || (1171..=1181).contains(&now)
                {
                    assert!(
                        link2.offers_veh(now).is_some(),
                        "No vehicle offered at timestep {now}"
                    );
                    if !(now == 1021
                        || now == 1041
                        || now == 1061
                        || now == 1081
                        || now == 1101
                        || now == 1121
                        || now == 1141
                        || now == 1161
                        || now == 1181)
                    {
                        assert!(
                            !link3.is_available(),
                            "Storage cap reached at timestep {now}"
                        );
                    }
                } else {
                    assert!(
                        link2.offers_veh(now).is_none(),
                        "Vehicle offered at timestep {now}"
                    );
                }
            }
        }
        publisher.events_publisher_borrow_mut().finish();
    }

    /// Tests that move_node produces outcome as expected with different link loadings.
    #[integration_test]
    fn move_nodes_transition_logic() {
        let mut net = Network::new();
        let node1 = Node {
            x: 0.0,
            y: 0.0,
            id: Id::create("node1"),
            in_links: vec![],
            out_links: vec![],
            partition: 0,
            cmp_weight: 1,
        };
        let node2 = Node {
            id: Id::create("node2"),
            ..node1.clone()
        };
        let node3 = Node {
            id: Id::create("node3"),
            ..node1.clone()
        };
        let node4 = Node {
            id: Id::create("node4"),
            ..node1.clone()
        };
        let node5 = Node {
            id: Id::create("node5"),
            ..node1.clone()
        };
        net.add_node(node1);
        net.add_node(node2);
        net.add_node(node3);
        net.add_node(node4);
        net.add_node(node5);

        net.add_link(Link {
            id: Id::create("link1"),
            from: Id::create("node1"),
            to: Id::create("node3"),
            length: 1.0,
            capacity: 3600.,
            freespeed: 100.,
            permlanes: 1.0,
            modes: Default::default(),
            partition: 0,
            attributes: Default::default(),
        });
        net.add_link(Link {
            id: Id::create("link2"),
            from: Id::create("node2"),
            to: Id::create("node3"),
            length: 1.0,
            capacity: 7200.,
            freespeed: 100.0,
            permlanes: 1.0,
            modes: Default::default(),
            partition: 0,
            attributes: Default::default(),
        });
        net.add_link(Link {
            id: Id::create("link3"),
            from: Id::create("node3"),
            to: Id::create("node4"),
            length: 75.,
            capacity: 3600.,
            freespeed: 100.0,
            permlanes: 1.0,
            modes: Default::default(),
            partition: 0,
            attributes: Default::default(),
        });
        net.add_link(Link {
            id: Id::create("link4"),
            from: Id::create("node4"),
            to: Id::create("node5"),
            length: 75.,
            capacity: 3600.,
            freespeed: 100.0,
            permlanes: 1.0,
            modes: Default::default(),
            partition: 0,
            attributes: Default::default(),
        });
        let mut sim_net =
            SimNetworkPartitionBuilder::from_network(&net, 0, &test_utils::config()).build();

        // Place 1000 vehicles on link1. Flow cap: 1 veh/s
        for i in 0..1000 {
            let agent = test_utils::create_agent(i, vec!["link1", "link3", "link4"]);
            let vehicle = InternalVehicle::new(i, 0, 100., 1., Some(agent));
            sim_net.send_veh_en_route(vehicle, None, 0);
        }

        // Place 1000 vehicles on link2. Flow cap: 2 veh/s
        for i in 1000..2000 {
            let agent = test_utils::create_agent(i, vec!["link2", "link3", "link4"]);
            let vehicle = InternalVehicle::new(i, 0, 100., 1., Some(agent));
            sim_net.send_veh_en_route(vehicle, None, 0);
        }

        let mut publisher = ThreadLocalComputationalEnvironment::default();
        XmlEventsWriter::register("test_output/test.xml".into())(
            &mut publisher.events_publisher_borrow_mut(),
        );

        for now in 0..1000 {
            sim_net.move_nodes(&mut publisher, now);
            sim_net.move_links(&mut publisher, now);
            if let Local(l) = sim_net.links.get(&Id::create("link1")).unwrap() {
                println!("Time {}, link1 veh_count: {}", now, l.veh_count());
            }
            if let Local(l) = sim_net.links.get(&Id::create("link2")).unwrap() {
                println!("Time {}, link2 veh_count: {}", now, l.veh_count());
            }
        }

        let link1 = if let Local(l) = sim_net.links.get(&Id::create("link1")).unwrap() {
            l.veh_count()
        } else {
            unreachable!()
        };
        let link2 = if let Local(l) = sim_net.links.get(&Id::create("link2")).unwrap() {
            l.veh_count()
        } else {
            unreachable!()
        };

        // Not 1000 but 993 because at the beginning link3 is not saturated.
        assert_eq!(link1 + link2, 993);

        // link1 has flow cap of 1 veh/s, link2 has flow cap of 2 veh/s.
        // Since all go from link1 and link2 to link3 (flow cap: 1 veh/s), there is only one vehicle per time step moved over the node.
        // This is why we expect link1 to have roughly twice the vehicles as link2.
        assert!(
            (link2 * 2).abs_diff(link1) <= 100,
            "values differ by more than 100"
        );
    }

    #[integration_test]
    fn storage_cap_over_boundaries() {
        // use programmed network here, to avoid instabilities with metis algorithm for small
        // network graphs
        let mut network = Network::new();
        let mut sim_nets = create_three_node_sim_network_with_partition(&mut network);
        let net2 = sim_nets.get_mut(1).unwrap();
        let mut publisher = Default::default();

        let split_link_id: Id<Link> = Id::get_from_ext("link2");
        let agent = test_utils::create_agent(1, vec![split_link_id.external()]);
        let vehicle = InternalVehicle::new(1, 0, 10., 100., Some(agent));

        // Network is empty, so no storage cap updates should be collected
        let res = net2.move_links(&mut Default::default(), 0);
        assert!(res.storage_cap_updates.is_empty());

        // NOTE: We are using push_veh and manually set active and veh_counter in order to not use
        // send_veh_en_route. This is because send_veh_en_route would put the vehicle into the waiting list and not into the queue.
        // But for this test, we need it to be inserted into the queue directly.
        net2.links
            .get_mut(&Id::create("link2"))
            .unwrap()
            .push_veh(vehicle, QStart, 0);
        net2.active_links.activate(Id::create("link2"));
        net2.veh_counter = 1;

        // now, in the next time step, nothing has changed on the link. It should therefore not
        // report any storage capacities
        net2.move_nodes(&mut publisher, 0);
        let res = net2.move_links(&mut Default::default(), 0);
        assert!(res.storage_cap_updates.is_empty());

        // After 10 steps, the vehicle can leave. As this is the end of the route, it is directly removed from the link, no move_nodes is required.
        let res = net2.move_links(&mut Default::default(), 10);

        assert_eq!(1, res.storage_cap_updates.len());
        let storage_cap = res.storage_cap_updates.first().unwrap();
        assert_eq!(split_link_id, storage_cap.link_id);
        assert_approx_eq!(100., storage_cap.released, 0.00001);
    }

    #[integration_test]
    fn neighbors() {
        let mut net = Network::new();
        let node = Node::new(Id::create("node-1"), -0., 0., 0, 1);
        let node_1_1 = Node::new(Id::create("node-1-1"), -0., 0., 1, 1);
        let node_1_2 = Node::new(Id::create("node-1-2"), -0., 0., 1, 1);

        let node_2_1 = Node::new(Id::create("node-2-1"), -0., 0., 2, 1);
        let node_3_1 = Node::new(Id::create("node-3-1"), -0., 0., 3, 1);
        let node_4_1 = Node::new(Id::create("not-a-neighbor"), 0., 0., 4, 1);

        // create in links from partitions 1 and 2. 2 incoming links from partition 1, one incoming from
        // partition 2
        let in_link_1_1 = Link::new_with_default(Id::create("in-link-1-1"), &node_1_1, &node);
        let in_link_1_2 = Link::new_with_default(Id::create("in-link-1-2"), &node_1_2, &node);
        let in_link_2_1 = Link::new_with_default(Id::create("in-link-2-1"), &node_2_1, &node);

        // create out links to partitions 1 and 3
        let out_link_1_1 = Link::new_with_default(Id::create("out-link-1-1"), &node, &node_1_1);
        let out_link_1_2 = Link::new_with_default(Id::create("out-link-1-2"), &node, &node_1_2);
        let out_link_3_1 = Link::new_with_default(Id::create("out-link-3-1"), &node, &node_3_1);

        net.add_node(node);
        net.add_node(node_1_1);
        net.add_node(node_1_2);
        net.add_node(node_2_1);
        net.add_node(node_3_1);
        net.add_node(node_4_1);
        net.add_link(in_link_1_1);
        net.add_link(in_link_1_2);
        net.add_link(in_link_2_1);
        net.add_link(out_link_1_1);
        net.add_link(out_link_1_2);
        net.add_link(out_link_3_1);

        let sim_net =
            SimNetworkPartitionBuilder::from_network(&net, 0, &test_utils::config()).build();

        let neighbors = sim_net.neighbors();
        assert_eq!(3, neighbors.len());
        assert!(neighbors.contains(&1));
        assert!(neighbors.contains(&2));
        assert!(neighbors.contains(&3));
        assert!(!neighbors.contains(&4));
    }

    fn create_three_node_sim_network_with_partition(
        network: &mut Network,
    ) -> Vec<SimNetworkPartition> {
        let node1 = Node::new(Id::create("node1"), -100., 0., 0, 1);
        let node2 = Node::new(Id::create("node2"), 0., 0., 0, 1);
        let mut node3 = Node::new(Id::create("node3"), 100., 0., 0, 1);
        node3.partition = 1;
        let mut link1 = Link::new_with_default(Id::create("link1"), &node1, &node2);
        link1.capacity = 3600.;
        link1.freespeed = 10.;
        let mut link2 = Link::new_with_default(Id::create("link2"), &node2, &node3);
        link2.capacity = 3600.;
        link2.freespeed = 10.;
        link2.partition = 1;

        network.add_node(node1);
        network.add_node(node2);
        network.add_node(node3);
        network.add_link(link1);
        network.add_link(link2);

        vec![
            SimNetworkPartitionBuilder::from_network(network, 0, &test_utils::config()).into(),
            SimNetworkPartitionBuilder::from_network(network, 1, &test_utils::config()).into(),
        ]
    }
}
