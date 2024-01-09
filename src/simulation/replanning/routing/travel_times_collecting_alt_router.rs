use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Debug;
use std::rc::Rc;

use nohash_hasher::IntMap;
use tracing::{debug, info};

use crate::simulation::id::Id;
use crate::simulation::messaging::communication::communicators::SimCommunicator;
use crate::simulation::messaging::communication::message_broker::TravelTimesMessageBroker;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::network::global_network::Network;
use crate::simulation::replanning::routing::alt_router::AltRouter;
use crate::simulation::replanning::routing::graph::ForwardBackwardGraph;
use crate::simulation::replanning::routing::network_converter::NetworkConverter;
use crate::simulation::replanning::routing::router::{CustomQueryResult, Router};
use crate::simulation::replanning::routing::travel_time_collector::TravelTimeCollector;
use crate::simulation::wire_types::messages::TravelTimesMessage;
use crate::simulation::wire_types::vehicles::VehicleType;

pub struct TravelTimesCollectingAltRouter<C: SimCommunicator> {
    router_by_mode: BTreeMap<u64, AltRouter>,
    traffic_message_broker: TravelTimesMessageBroker<C>,
    link_ids_of_process: HashSet<u64>,
}

impl<C: SimCommunicator> Debug for TravelTimesCollectingAltRouter<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TravelTimesCollectingAltRouter")
    }
}

impl<C: SimCommunicator> Router for TravelTimesCollectingAltRouter<C> {
    fn query_links(&self, from_link: u64, to_link: u64, mode: u64) -> CustomQueryResult {
        self.get_router_by_mode(mode)
            .unwrap_or_else(|| {
                panic!(
                    "There is no router for mode {:?}. Check the vehicle definitions.",
                    mode
                )
            })
            .query_links(from_link, to_link)
    }

    fn next_time_step(&mut self, now: u32, events: &mut EventsPublisher) {
        let traffic_update_interval_in_min = 15;
        if now % (60 * traffic_update_interval_in_min) != 0 {
            return;
        }

        let _hour = now / 3600;
        let _min = (now % 3600) / 60;
        debug!(
            "#{:?} Traffic update triggered at {_hour}:{_min}",
            self.traffic_message_broker.rank()
        );

        //get travel times
        let collected_travel_times = Self::collect_travel_times(events, now);

        //compute all updates of partition
        let send_package = self.get_travel_times_by_mode_to_send(&collected_travel_times, now);

        let received_messages_by_mode = send_package
            .into_iter()
            .map(|(mode, updates)| {
                let received_messages = self.communicate_travel_times(now, updates);
                (mode, received_messages)
            })
            .collect::<BTreeMap<u64, Vec<TravelTimesMessage>>>();

        //handle travel times
        for (mode, message) in received_messages_by_mode.into_iter() {
            self.handle_traffic_info_messages(now, mode, message);
        }

        //reset travel times
        events
            .get_subscriber::<TravelTimeCollector>()
            .expect("There is no TravelTimeCollector as EventSubscriber.")
            .flush();
    }
}

impl<C: SimCommunicator> TravelTimesCollectingAltRouter<C> {
    #[tracing::instrument(level = "trace", skip(events))]
    fn collect_travel_times(events: &mut EventsPublisher, _now: u32) -> HashMap<u64, u32> {
        events
            .get_subscriber::<TravelTimeCollector>()
            .map(|travel_time_collector| travel_time_collector.get_travel_times())
            .unwrap()
    }

    #[tracing::instrument(level = "trace", skip(updates))]
    fn communicate_travel_times(
        &mut self,
        now: u32,
        updates: HashMap<u64, u32>,
    ) -> Vec<TravelTimesMessage> {
        self.traffic_message_broker.send_recv(now, updates)
    }
}

impl<C: SimCommunicator> TravelTimesCollectingAltRouter<C> {
    pub fn new(
        forward_backward_graph_by_mode: HashMap<u64, ForwardBackwardGraph>,
        communicator: Rc<C>,
        link_ids_of_process: HashSet<u64>,
    ) -> Self {
        let router_by_mode = forward_backward_graph_by_mode
            .iter()
            .map(|(&m, g)| (m, AltRouter::new(g.clone())))
            .collect::<BTreeMap<_, _>>();

        info!(
            "Created TravelTimesCollectingAltRouter with modes: {:?}",
            router_by_mode
                .keys()
                .map(|k| (k, String::from(Id::<String>::get(*k).external())))
                .collect::<HashMap<&u64, String>>()
        );

        TravelTimesCollectingAltRouter {
            router_by_mode,
            traffic_message_broker: TravelTimesMessageBroker::new(communicator),
            link_ids_of_process,
        }
    }

    #[tracing::instrument(level = "trace", skip(mode, traffic_info_messages))]
    fn handle_traffic_info_messages(
        &mut self,
        _now: u32,
        mode: u64,
        traffic_info_messages: Vec<TravelTimesMessage>,
    ) {
        if traffic_info_messages.is_empty() {
            return;
        }

        let number_of_links_with_traffic_info = traffic_info_messages
            .iter()
            .map(|info| info.travel_times_by_link_id.len())
            .sum::<usize>();

        let travel_times_by_link: HashMap<u64, u32> = traffic_info_messages
            .into_iter()
            .flat_map(|info| info.travel_times_by_link_id.into_iter())
            .collect();

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

    #[tracing::instrument(level = "trace", skip(self, collected_travel_times))]
    fn get_travel_times_by_mode_to_send(
        &mut self,
        collected_travel_times: &HashMap<u64, u32>,
        _now: u32,
    ) -> BTreeMap<u64, HashMap<u64, u32>> {
        let mut result = BTreeMap::new();
        for (mode, router) in self.router_by_mode.iter_mut() {
            let mut extended_travel_times_by_link_id = HashMap::new();
            for id in &self.link_ids_of_process {
                if let Some(travel_time) = collected_travel_times.get(id) {
                    // add collected travel time
                    let initial = router.get_initial_travel_time(*id);

                    if initial.is_none() {
                        continue;
                    }

                    let new_travel_time = (*travel_time).max(initial.unwrap());
                    extended_travel_times_by_link_id.insert(*id, new_travel_time);
                } else {
                    // add initial travel time for each link which has no new travel time
                    let initial = router.get_initial_travel_time(*id);

                    if initial.is_none() {
                        continue;
                    }

                    extended_travel_times_by_link_id.insert(*id, initial.unwrap());
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
