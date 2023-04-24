use crate::simulation::id_mapping::MatsimIdMappings;
use crate::simulation::io::network::IONetwork;
use crate::simulation::io::vehicle_definitions::VehicleDefinitions;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::messaging::messages::proto::{Plan, TravelTimesMessage};
use crate::simulation::messaging::travel_time_collector::TravelTimeCollector;
use crate::simulation::performance_profiling::measure_duration;
use crate::simulation::routing::network_converter::NetworkConverter;
use crate::simulation::routing::road_router::RoadRouter;
use crate::simulation::routing::router::{CustomQueryResult, Router};
use crate::simulation::routing::travel_times_message_broker::TravelTimesMessageBroker;
use mpi::topology::SystemCommunicator;
use mpi::Rank;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::debug;

pub struct TravelTimesCollectingRoadRouter<'router> {
    router_by_mode: HashMap<String, RoadRouter<'router>>,
    traffic_message_broker: TravelTimesMessageBroker,
    vehicle_definitions: Option<VehicleDefinitions>,
}

impl<'router> Router for TravelTimesCollectingRoadRouter<'router> {
    fn query_links(&mut self, from_link: u64, to_link: u64, mode: &str) -> CustomQueryResult {
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
        let collected_travel_times: HashMap<u64, u32> =
            measure_duration(Some(now), "travel_time_aggregation", None, || {
                let collected_travel_times = events
                    .get_subscriber::<TravelTimeCollector>()
                    .map(|travel_time_collector| travel_time_collector.get_travel_times())
                    .unwrap();

                if !collected_travel_times.is_empty() {
                    debug!("Collected travel times are: {:?}", collected_travel_times);
                }
                collected_travel_times
            });

        //send travel times
        let updates = collected_travel_times.len() as u64;
        let vec = measure_duration(
            Some(now),
            "travel_time_send",
            Some(json!({ "updates": updates })),
            || {
                self.traffic_message_broker
                    .send_recv(now, collected_travel_times)
            },
        );

        measure_duration(
            Some(now),
            "travel_time_handling",
            Some(json!({ "updates": updates })),
            || {
                self.handle_traffic_info_messages(vec);
            },
        );

        //reset travel times
        events
            .get_subscriber::<TravelTimeCollector>()
            .expect("There is no TravelTimeCollector as EventSubscriber.")
            .flush();
    }
}

impl<'router> TravelTimesCollectingRoadRouter<'router> {
    pub fn new(
        io_network: IONetwork, //TODO change it to matsim internal network representation
        id_mappings: Option<&MatsimIdMappings>,
        communicator: SystemCommunicator,
        rank: Rank,
        output_dir: PathBuf,
        vehicle_definitions: Option<VehicleDefinitions>,
    ) -> Self {
        let router_by_mode = if let Some(vehicle_definitions) = vehicle_definitions.as_ref() {
            NetworkConverter::convert_io_network_with_vehicle_definitions(
                io_network,
                id_mappings,
                vehicle_definitions,
            )
            .iter()
            .map(|(m, r)| (m.clone(), RoadRouter::new(r, output_dir.join(m))))
            .collect::<HashMap<_, _>>()
        } else {
            let mut map = HashMap::new();
            map.insert(
                Plan::DEFAULT_ROUTING_MODE.to_string(),
                RoadRouter::new(
                    &NetworkConverter::convert_io_network(io_network, id_mappings, None, None),
                    output_dir,
                ),
            );
            map
        };

        TravelTimesCollectingRoadRouter {
            router_by_mode,
            traffic_message_broker: TravelTimesMessageBroker::new(communicator, rank),
            vehicle_definitions,
        }
    }

    fn handle_traffic_info_messages(&mut self, traffic_info_messages: Vec<TravelTimesMessage>) {
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

        // For each router: evaluate new travel time by link. If travel time for link was sent,
        // use maximum(new travel time, initial travel time). Otherwise use initial travel time.
        for (mode, router) in self.router_by_mode.iter_mut() {
            let mut extended_travel_times_by_link_id = HashMap::new();

            for link_id in router.get_current_network().link_ids.iter() {
                let new_travel_time = if let Some(travel_time) = travel_times_by_link.get(link_id) {
                    **travel_time.max(&&router.get_initial_travel_time(*link_id))
                } else {
                    router.get_initial_travel_time(*link_id)
                };

                extended_travel_times_by_link_id.insert(link_id, new_travel_time);
            }

            let network = router
                .get_current_network()
                .clone_with_new_travel_times_by_link(&extended_travel_times_by_link_id);

            if network.has_different_travel_times(router.get_current_network()) {
                debug!(
                    "There are travel time changes. Router for mode {:?} will be customized.",
                    mode
                );
                router.customize(network);
            }
        }
    }

    fn get_router_by_mode(&mut self, mode: &str) -> Option<&mut RoadRouter<'router>> {
        self.router_by_mode.get_mut(mode)
    }
}
