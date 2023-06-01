use crate::simulation::messaging::events::proto::event::Type;
use crate::simulation::messaging::events::proto::{
    Event, LinkEnterEvent, LinkLeaveEvent, PersonLeavesVehicleEvent,
};
use crate::simulation::messaging::events::EventsSubscriber;
use std::any::Any;
use std::collections::HashMap;

pub struct TravelTimeCollector {
    travel_times_by_link: HashMap<u64, Vec<u32>>,
    cache_traffic_information_by_link: HashMap<u64, Vec<TrafficInformation>>,
    current_link_by_vehicle: HashMap<u64, u64>,
}

#[derive(Debug, PartialEq, Clone)]
struct TrafficInformation {
    time: u32,
    vehicle: u64,
}

impl TravelTimeCollector {
    pub fn new() -> Self {
        TravelTimeCollector {
            travel_times_by_link: HashMap::new(),
            cache_traffic_information_by_link: HashMap::new(),
            current_link_by_vehicle: HashMap::new(),
        }
    }

    fn process_link_enter_event(&mut self, time: u32, event: &LinkEnterEvent) {
        // link enter events will always be stored
        self.cache_traffic_information_by_link
            .entry(event.link)
            .or_insert(Vec::new())
            .push(TrafficInformation {
                time,
                vehicle: event.vehicle,
            });

        self.current_link_by_vehicle
            .insert(event.vehicle, event.link);
    }

    fn process_link_leave_event(&mut self, time: u32, event: &LinkLeaveEvent) {
        // find link enter event
        let index_of_link_enter = self
            .cache_traffic_information_by_link
            .entry(event.link)
            .or_insert(Vec::new())
            .iter()
            .position(|e| e.vehicle == event.vehicle);

        // remove traffic information of link enter and compute travel time (assumes that a vehicle will leave a link before it enters the same again)
        // if it's None, the LinkLeaveEvent is the begin of a leg, thus no travel time can be computed
        if let Some(index) = index_of_link_enter {
            let traffic_information = self
                .cache_traffic_information_by_link
                .get_mut(&event.link)
                .unwrap()
                .remove(index);

            self.travel_times_by_link
                .entry(event.link)
                .or_insert(Vec::new())
                .push(time - traffic_information.time)
        }
    }

    fn process_person_leaves_vehicle_event(&mut self, time: u32, event: &PersonLeavesVehicleEvent) {
        let link_id = self
            .current_link_by_vehicle
            .get(&event.vehicle).copied()
            .expect("Before a person can leave a vehicle, it must have entered the link and thus a current link must be available.");

        let index_of_link_enter = self
            .get_index_of_link_enter_event(link_id, event.vehicle)
            .expect("Before a person can leave a vehicle, it must have entered the link and thus traffic information must be available.");

        self.cache_traffic_information_by_link
            .get_mut(&link_id)
            .unwrap()
            .remove(index_of_link_enter);
    }

    fn get_index_of_link_enter_event(&mut self, link: u64, vehicle: u64) -> Option<usize> {
        self.cache_traffic_information_by_link
            .entry(link)
            .or_insert(Vec::new())
            .iter()
            .position(|t| t.vehicle == vehicle)
    }

    pub fn get_travel_time_of_link(&self, link: u64) -> Option<u32> {
        match self.travel_times_by_link.get(&link) {
            None => None,
            Some(travel_times) => {
                let sum: u32 = travel_times.iter().sum();
                let len = travel_times.len() as u32;
                Some(sum / len)
            }
        }
    }

    pub fn get_travel_times(&self) -> HashMap<u64, u32> {
        self.travel_times_by_link
            .iter()
            .map(|(id, travel_time)| (*id, self.get_travel_time_of_link(*id)))
            .filter(|(id, travel_time)| travel_time.is_some())
            .map(|(id, travel_time)| (id, travel_time.unwrap()))
            .collect::<HashMap<u64, u32>>()
    }

    pub fn flush(&mut self) {
        // Collected travel times will be dropped, but cached values not.
        // Vehicles of cached values haven't left the corresponding links yet.
        // A travel time of a link is considered when a vehicle leaves the link.
        self.travel_times_by_link = HashMap::new();
    }
}

impl EventsSubscriber for TravelTimeCollector {
    fn receive_event(&mut self, time: u32, event: &Event) {
        match event.r#type.as_ref().unwrap() {
            Type::LinkEnter(e) => self.process_link_enter_event(time, e),
            Type::LinkLeave(e) => self.process_link_leave_event(time, e),
            Type::PersonLeavesVeh(e) => self.process_person_leaves_vehicle_event(time, e),
            _ => {}
        }
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod test {
    use crate::simulation::messaging::events::proto::Event;
    use crate::simulation::messaging::events::EventsSubscriber;
    use crate::simulation::messaging::travel_time_collector::{
        TrafficInformation, TravelTimeCollector,
    };

    #[test]
    fn test_one_vehicle() {
        let mut collector = TravelTimeCollector::new();
        collector.receive_event(1, &Event::new_link_leave(1, 1));
        collector.receive_event(2, &Event::new_link_enter(2, 1));
        collector.receive_event(4, &Event::new_link_leave(2, 1));

        assert_eq!(collector.get_travel_time_of_link(2), Some(2));
        assert_eq!(collector.get_travel_time_of_link(1), None);
        assert_eq!(collector.get_travel_times().keys().len(), 1);
        assert_eq!(collector.get_travel_times().get(&2), Some(&2u32));

        assert_eq!(
            collector
                .cache_traffic_information_by_link
                .get(&1)
                .unwrap()
                .first(),
            None
        )
    }

    #[test]
    /// Tests travel time collection with two vehicles passing link 2.
    /// Vehicle 1: 2s
    /// Vehicle 2: 4s
    fn test_two_vehicles() {
        let mut collector = TravelTimeCollector::new();
        collector.receive_event(1, &Event::new_link_leave(1, 1));
        // vehicle 1 enters
        collector.receive_event(2, &Event::new_link_enter(2, 1));
        // vehicle 2 enters
        collector.receive_event(3, &Event::new_link_enter(2, 2));
        // vehicle 1 leaves
        collector.receive_event(4, &Event::new_link_leave(2, 1));
        // vehicle 3 enters
        collector.receive_event(5, &Event::new_link_enter(2, 3));
        // vehicle 2 leaves
        collector.receive_event(7, &Event::new_link_leave(2, 2));

        // The average travel time on link 2 is 3
        assert_eq!(Some(3), collector.get_travel_time_of_link(2));
        assert_eq!(None, collector.get_travel_time_of_link(1));

        assert_eq!(collector.get_travel_times().keys().len(), 1);
        assert_eq!(collector.get_travel_times().get(&2), Some(&3u32));

        // link 1 has no cached traffic information
        assert_eq!(
            collector
                .cache_traffic_information_by_link
                .get(&1)
                .unwrap()
                .first(),
            None
        );

        // link 2 has cached traffic information from vehicle 3
        assert_eq!(
            collector
                .cache_traffic_information_by_link
                .get(&2)
                .unwrap()
                .first(),
            Some(&TrafficInformation {
                time: 5,
                vehicle: 3,
            })
        );
    }

    #[test]
    /// Tests whether PersonLeavesVehicleEvent discards travel time
    fn test_with_person_leaves_vehicle() {
        let mut collector = TravelTimeCollector::new();
        collector.receive_event(0, &Event::new_link_enter(1, 1));
        collector.receive_event(2, &Event::new_person_leaves_veh(1, 1));
        collector.receive_event(4, &Event::new_link_leave(1, 1));

        assert_eq!(collector.get_travel_time_of_link(1), None);
        assert_eq!(
            collector
                .cache_traffic_information_by_link
                .get(&1)
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    /// Tests whether PersonLeavesVehicleEvent discards travel time
    fn test_with_person_leaves_vehicle_complex() {
        let mut collector = TravelTimeCollector::new();
        collector.receive_event(0, &Event::new_link_enter(1, 1));

        //intermediate veh 2 enters link 1
        collector.receive_event(1, &Event::new_link_enter(1, 2));

        collector.receive_event(2, &Event::new_person_leaves_veh(1, 1));

        //intermediate veh 2 leaves link 1
        collector.receive_event(3, &Event::new_link_leave(1, 2));

        collector.receive_event(10, &Event::new_link_leave(1, 1));
        collector.receive_event(10, &Event::new_link_enter(2, 1));
        collector.receive_event(20, &Event::new_link_leave(2, 1));

        assert_eq!(collector.get_travel_time_of_link(1), Some(2));
        assert_eq!(collector.get_travel_time_of_link(2), Some(10));
        assert_eq!(
            collector
                .cache_traffic_information_by_link
                .get(&1)
                .unwrap()
                .len(),
            0
        );
    }
}
