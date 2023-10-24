use std::collections::{BTreeMap, HashMap, HashSet};

use mpi::topology::SystemCommunicator;
use mpi::Rank;
use nohash_hasher::IntMap;
use tracing::debug;

use crate::simulation::id::Id;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::messaging::messages::proto::TravelTimesMessage;
use crate::simulation::messaging::travel_time_collector::TravelTimeCollector;
use crate::simulation::network::global_network::Network;
use crate::simulation::plan_modification::routing::alt_router::AltRouter;
use crate::simulation::plan_modification::routing::graph::ForwardBackwardGraph;
use crate::simulation::plan_modification::routing::network_converter::NetworkConverter;
use crate::simulation::plan_modification::routing::router::{CustomQueryResult, Router};
use crate::simulation::plan_modification::routing::travel_times_message_broker::TravelTimesMessageBroker;
use crate::simulation::vehicles::vehicle_type::VehicleType;

pub struct TravelTimesCollectingAltRouter {
    router_by_mode: BTreeMap<u64, AltRouter>,
    traffic_message_broker: TravelTimesMessageBroker,
    link_ids_of_process: HashSet<u64>,
}

impl Router for TravelTimesCollectingAltRouter {
    fn query_links(&self, from_link: u64, to_link: u64, mode: u64) -> CustomQueryResult {
        self.get_router_by_mode(mode)
            .expect(&*format!(
                "There is no router for mode {:?}. Check the vehicle definitions.",
                mode
            ))
            .query_links(from_link, to_link)
    }

    fn next_time_step(&mut self, now: u32, events: &mut EventsPublisher) {
        let traffic_update_interval_in_min = 15;
        if !(now % (60 * traffic_update_interval_in_min) == 0) {
            return;
        }

        let _hour = now / 3600;
        let _min = (now % 3600) / 60;
        debug!(
            "#{:?} Traffic update triggered at {_hour}:{_min}",
            self.traffic_message_broker.rank
        );

        //get travel times
        let collected_travel_times = events
            .get_subscriber::<TravelTimeCollector>()
            .map(|travel_time_collector| travel_time_collector.get_travel_times())
            .unwrap();

        //compute all updates of partition
        let send_package = self.get_travel_times_by_mode_to_send(&collected_travel_times);

        // self.get_travel_times_by_mode_to_send(&collected_travel_times);

        let received_messages_by_mode = send_package
            .into_iter()
            .map(|(mode, updates)| {
                let received_messages = self.traffic_message_broker.send_recv(now, updates);
                (mode, received_messages)
            })
            .collect::<BTreeMap<u64, Vec<TravelTimesMessage>>>();

        //handle travel times
        for (mode, message) in received_messages_by_mode.into_iter() {
            // let number_of_updates: usize = message
            //     .iter()
            //     .map(|m| m.travel_times_by_link_id.len())
            //     .sum();
            self.handle_traffic_info_messages(now, mode, message);
        }

        //reset travel times
        events
            .get_subscriber::<TravelTimeCollector>()
            .expect("There is no TravelTimeCollector as EventSubscriber.")
            .flush();
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
        _now: u32,
        mode: u64,
        traffic_info_messages: Vec<TravelTimesMessage>,
    ) {
        if traffic_info_messages.is_empty() {
            return;
        }

        let travel_times_by_link = traffic_info_messages
            .iter()
            .map(|info| &info.travel_times_by_link_id)
            .fold(HashMap::new(), |result, value| {
                result.into_iter().chain(value).collect()
            });

        let number_of_links_with_traffic_info = traffic_info_messages
            .iter()
            .map(|info| info.travel_times_by_link_id.len())
            .sum::<usize>();

        assert_eq!(
            number_of_links_with_traffic_info,
            travel_times_by_link.len()
        );

        let new_graph = self
            .router_by_mode
            .get(&mode)
            .unwrap()
            .current_graph()
            .clone_with_new_travel_times_by_link(travel_times_by_link);

        self.router_by_mode
            .get_mut(&mode)
            .unwrap()
            .update(new_graph);
    }

    fn get_router_by_mode(&self, mode: u64) -> Option<&AltRouter> {
        self.router_by_mode.get(&mode)
    }

    fn get_travel_times_by_mode_to_send(
        &mut self,
        collected_travel_times: &HashMap<u64, u32>,
    ) -> BTreeMap<u64, HashMap<u64, u32>> {
        let mut result = BTreeMap::new();
        for (mode, router) in self.router_by_mode.iter_mut() {
            let mut extended_travel_times_by_link_id = HashMap::new();
            for id in &self.link_ids_of_process {
                if let Some(travel_time) = collected_travel_times.get(&id) {
                    // for each collected travel time: add if currently known travel time is different
                    let new_travel_time = (*travel_time).max(router.get_initial_travel_time(*id));
                    if new_travel_time != router.get_current_travel_time(*id) {
                        extended_travel_times_by_link_id.insert(*id, new_travel_time);
                    }
                } else {
                    // for each link which has no new travel time: add initial travel time if currently known travel time is different
                    let initial_travel_time = router.get_initial_travel_time(*id);
                    if router.get_current_travel_time(*id) != initial_travel_time {
                        extended_travel_times_by_link_id.insert(*id, initial_travel_time);
                    }
                }
            }
            result.insert(*mode, extended_travel_times_by_link_id);
        }
        result
    }

    pub fn get_forward_backward_graph_by_mode(
        network: &Network,
        vehicle_types: &IntMap<Id<VehicleType>, VehicleType>,
    ) -> HashMap<u64, ForwardBackwardGraph> {
        NetworkConverter::convert_network_with_vehicle_types(network, vehicle_types)
    }
}
