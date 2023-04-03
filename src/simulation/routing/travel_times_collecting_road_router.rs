use crate::simulation::id_mapping::MatsimIdMappings;
use crate::simulation::io::network::IONetwork;
use crate::simulation::io::vehicle_definitions::VehicleDefinitions;
use crate::simulation::messaging::events::EventsPublisher;
use crate::simulation::messaging::messages::proto::{TravelTimesMessage, Vehicle};
use crate::simulation::messaging::travel_time_collector::TravelTimeCollector;
use crate::simulation::network::link::Link;
use crate::simulation::network::network_partition::NetworkPartition;
use crate::simulation::network::routing_kit_network::RoutingKitNetwork;
use crate::simulation::routing::network_converter::NetworkConverter;
use crate::simulation::routing::road_router::RoadRouter;
use crate::simulation::routing::router::{CustomQueryResult, Router};
use crate::simulation::routing::travel_times_message_broker::TravelTimesMessageBroker;
use log::debug;
use mpi::topology::SystemCommunicator;
use mpi::Rank;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

pub struct TravelTimesCollectingRoadRouter<'router> {
    router_by_mode: HashMap<&'router str, RoadRouter<'router>>,
    traffic_message_broker: TravelTimesMessageBroker,
    link_ids_of_process: HashSet<u64>,
    vehicle_definitions: Option<&'router VehicleDefinitions>,
}

impl<'router> Router for TravelTimesCollectingRoadRouter<'router> {
    fn query_links(&mut self, from_link: u64, to_link: u64) -> CustomQueryResult {
        self.router.query_links(from_link, to_link)
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

        //send travel times
        let vec = self.traffic_message_broker.send_recv(
            now,
            self.get_travel_times_by_link_to_send(collected_travel_times),
        );

        self.handle_traffic_info_messages(vec);

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
        network_partition: &NetworkPartition<Vehicle>,
        vehicle_definitions: Option<&VehicleDefinitions>,
    ) -> Self {
        let full_network = NetworkConverter::convert_io_network(io_network, id_mappings);

        let link_ids_of_process = network_partition
            .links
            .iter()
            .filter(|(id, link)| match link {
                Link::LocalLink(_) => true,
                Link::SplitInLink(_) => true,
                Link::SplitOutLink(_) => false,
            })
            .map(|(id, _)| *id as u64)
            .collect::<HashSet<u64>>();

        let mut router_by_mode = HashMap::new();

        if let Some(vehicle_definitions) = vehicle_definitions {
            router_by_mode = vehicle_definitions
                .vehicle_types
                .iter()
                //TODO network
                .map(|v| (v.id.as_str(), RoadRouter::new(&full_network, output_dir)))
                .collect::<HashMap<_, _>>()
        } else {
            todo!()
        }

        TravelTimesCollectingRoadRouter {
            router_by_mode,
            traffic_message_broker: TravelTimesMessageBroker::new(communicator, rank),
            link_ids_of_process,
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

        let network_with_new_travel_times = self
            .router
            .get_current_network()
            .clone_with_new_travel_times_by_link(travel_times_by_link);

        debug!(
            "#{:?} network with new travel times: {:?}",
            self.traffic_message_broker.rank, network_with_new_travel_times
        );

        debug!("There are travel time changes. Router will be customized.");
        self.router.customize(network_with_new_travel_times);
    }

    fn get_travel_times_by_link_to_send(
        &self,
        collected_travel_times: HashMap<u64, u32>,
    ) -> HashMap<u64, u32> {
        let mut result = HashMap::new();

        // for each collected travel time: add if currently known travel time is different
        for (id, travel_time) in &collected_travel_times {
            if *travel_time != self.router.get_current_travel_time(*id) {
                result.insert(*id, *travel_time);
            } else {
                debug!(
                    "Process {:?} | (link {:?}, travel time: {:?}) was already there.",
                    self.traffic_message_broker.rank, id, travel_time
                );
            }
        }

        // for each link about which no travel time was collected: add initial travel time if currently known travel time is different
        for id in self
            .link_ids_of_process
            .difference(&collected_travel_times.into_keys().collect::<HashSet<u64>>())
        {
            let initial_travel_time = self.router.get_initial_travel_time(*id);
            if self.router.get_current_travel_time(*id) != initial_travel_time {
                result.insert(*id, initial_travel_time);
            }
        }
        if !result.is_empty() {
            debug!("Traffic update to be sent: {:?}", result);
        }
        result
    }

    fn get_router_by_mode(&mut self, mode: &str) -> Option<&mut RoadRouter<'router>> {
        self.router_by_mode.get_mut(mode)
    }
}
