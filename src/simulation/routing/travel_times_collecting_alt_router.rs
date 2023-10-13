use std::collections::{BTreeMap, HashMap, HashSet};

use mpi::topology::SystemCommunicator;
use mpi::Rank;

use crate::simulation::id::Id;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::messaging::messages::proto::TravelTimesMessage;
use crate::simulation::network::global_network::Network;
use crate::simulation::routing::alt_router::AltRouter;
use crate::simulation::routing::graph::ForwardBackwardGraph;
use crate::simulation::routing::network_converter::NetworkConverter;
use crate::simulation::routing::router::{CustomQueryResult, Router};
use crate::simulation::routing::travel_times_message_broker::TravelTimesMessageBroker;
use crate::simulation::vehicles::vehicle_type::VehicleType;

pub struct TravelTimesCollectingAltRouter {
    router_by_mode: BTreeMap<u64, AltRouter>,
    traffic_message_broker: TravelTimesMessageBroker,
    link_ids_of_process: HashSet<u64>,
}

impl Router for TravelTimesCollectingAltRouter {
    fn query_links(&mut self, from_link: u64, to_link: u64, mode: u64) -> CustomQueryResult {
        //TODO
        self.get_router_by_mode(mode)
            .expect(&*format!(
                "There is no router for mode {:?}. Check the vehicle definitions.",
                mode
            ))
            .query_links(from_link, to_link)
    }

    fn next_time_step(&mut self, now: u32, events: &mut EventsPublisher) {
        todo!()
    }
}

impl TravelTimesCollectingAltRouter {
    pub fn new(
        forward_backward_graph_by_mode: HashMap<u64, ForwardBackwardGraph>,
        communicator: SystemCommunicator,
        rank: Rank,
        link_ids_of_process: HashSet<u64>,
    ) -> Self {
        let router_by_mode = forward_backward_graph_by_mode
            .iter()
            .map(|(&m, g)| (m, AltRouter::new(g.clone())))
            .collect::<BTreeMap<_, _>>();

        TravelTimesCollectingAltRouter {
            router_by_mode,
            traffic_message_broker: TravelTimesMessageBroker::new(communicator, rank),
            link_ids_of_process,
        }
    }

    fn handle_traffic_info_messages(
        &mut self,
        now: u32,
        mode: String,
        traffic_info_messages: Vec<TravelTimesMessage>,
    ) {
    }

    fn get_router_by_mode(&mut self, mode: u64) -> Option<&mut AltRouter> {
        self.router_by_mode.get_mut(&mode)
    }

    fn get_travel_times_by_mode_to_send(
        &mut self,
        collected_travel_times: &HashMap<u64, u32>,
    ) -> BTreeMap<String, HashMap<u64, u32>> {
        BTreeMap::new() //TODO
    }

    pub fn get_forward_backward_graph_by_mode(
        network: &Network,
        vehicle_types: &HashMap<Id<VehicleType>, VehicleType>,
    ) -> HashMap<u64, ForwardBackwardGraph> {
        NetworkConverter::convert_network_with_vehicle_types(network, vehicle_types)
    }
}
