use std::collections::{BTreeMap, HashMap, HashSet};

use mpi::Rank;
use mpi::topology::SystemCommunicator;

use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::messaging::messages::proto::TravelTimesMessage;
use crate::simulation::routing::alt_router::AltRouter;
use crate::simulation::routing::router::{CustomQueryResult, Router};
use crate::simulation::routing::travel_times_message_broker::TravelTimesMessageBroker;

pub struct TravelTimesCollectingAltRouter {
    router_by_mode: BTreeMap<String, AltRouter>,
    traffic_message_broker: TravelTimesMessageBroker,
    link_ids_of_process: HashSet<u64>,
}

impl Router for TravelTimesCollectingAltRouter {
    fn query_links(&mut self, from_link: u64, to_link: u64, mode: &str) -> CustomQueryResult {
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
    pub fn new(communicator: SystemCommunicator, rank: Rank, link_ids_of_process: HashSet<u64>) -> Self {
        TravelTimesCollectingAltRouter {
            router_by_mode: Default::default(), //TODO
            traffic_message_broker: TravelTimesMessageBroker::new(communicator, rank),
            link_ids_of_process,
        }
    }

    fn handle_traffic_info_messages(
        &mut self,
        now: u32,
        mode: String,
        traffic_info_messages: Vec<TravelTimesMessage>,
    ) {}

    fn get_router_by_mode(&mut self, mode: &str) -> Option<&mut AltRouter> {
        self.router_by_mode.get_mut(mode)
    }

    fn get_travel_times_by_mode_to_send(
        &mut self,
        collected_travel_times: &HashMap<u64, u32>,
    ) -> BTreeMap<String, HashMap<u64, u32>> {
        BTreeMap::new() //TODO
    }
}