use crate::generated::events::event::Type;
use crate::generated::events::{Event, LinkEnterEvent, LinkLeaveEvent, PersonLeavesVehicleEvent};
use crate::simulation::id::Id;
use crate::simulation::messaging::events::EventsSubscriber;
use crate::simulation::network::Link;
use crate::simulation::vehicles::InternalVehicle;
use nohash_hasher::IntMap;
use std::any::Any;
use std::collections::HashMap;

pub struct TravelTimeCollector {
    travel_times_by_link: HashMap<Id<Link>, Vec<u32>>,
    cache_enter_time_by_vehicle: IntMap<Id<InternalVehicle>, u32>,
}

impl Default for TravelTimeCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl TravelTimeCollector {
    pub fn new() -> Self {
        TravelTimeCollector {
            travel_times_by_link: HashMap::new(),
            cache_enter_time_by_vehicle: IntMap::default(),
        }
    }

    fn process_link_enter_event(&mut self, time: u32, event: &LinkEnterEvent) {
        // link enter events will always be stored
        self.cache_enter_time_by_vehicle
            .insert(Id::<InternalVehicle>::get_from_ext(&event.vehicle), time);
    }

    fn process_link_leave_event(&mut self, time: u32, event: &LinkLeaveEvent) {
        // remove traffic information of link enter and compute travel time (assumes that a vehicle will leave a link before it enters the same again)
        // if it's None, the LinkLeaveEvent is the begin of a leg, thus no travel time can be computed
        if let Some(t) = self
            .cache_enter_time_by_vehicle
            .remove(&Id::<InternalVehicle>::get_from_ext(&event.vehicle))
        {
            self.travel_times_by_link
                .entry(Id::<Link>::get_from_ext(&event.link))
                .or_default()
                .push(time - t)
        }
    }

    fn process_person_leaves_vehicle_event(
        &mut self,
        _time: u32,
        event: &PersonLeavesVehicleEvent,
    ) {
        self.cache_enter_time_by_vehicle
            .remove(&Id::<InternalVehicle>::get_from_ext(&event.vehicle));
    }

    pub fn get_travel_time_of_link(&self, link: &Id<Link>) -> Option<u32> {
        match self.travel_times_by_link.get(&link) {
            None => None,
            Some(travel_times) => {
                let sum: u32 = travel_times.iter().sum();
                let len = travel_times.len();
                Some(sum / (len as u32))
            }
        }
    }

    pub fn get_travel_times(&self) -> HashMap<Id<Link>, u32> {
        self.travel_times_by_link
            .keys()
            .map(|id| (id.clone(), self.get_travel_time_of_link(id)))
            .filter(|(_, travel_time)| travel_time.is_some())
            .map(|(id, travel_time)| (id, travel_time.unwrap()))
            .collect::<HashMap<Id<Link>, u32>>()
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
    use crate::generated::events::Event;
    use crate::simulation::id::Id;
    use crate::simulation::messaging::events::EventsSubscriber;
    use crate::simulation::replanning::routing::travel_time_collector::TravelTimeCollector;

    #[test]
    fn test_one_vehicle() {
        let link1 = Id::create("1");
        let link2 = Id::create("2");

        let vehicle1 = Id::create("1");

        let mut collector = TravelTimeCollector::new();
        collector.receive_event(1, &Event::new_link_leave(&link1, &vehicle1));
        collector.receive_event(2, &Event::new_link_enter(&link2, &vehicle1));
        collector.receive_event(4, &Event::new_link_leave(&link2, &vehicle1));

        assert_eq!(collector.get_travel_time_of_link(&link2), Some(2));
        assert_eq!(collector.get_travel_time_of_link(&link1), None);
        assert_eq!(collector.get_travel_times().keys().len(), 1);
        assert_eq!(collector.get_travel_times().get(&link2), Some(&2u32));

        assert_eq!(collector.cache_enter_time_by_vehicle.get(&vehicle1), None)
    }

    #[test]
    /// Tests travel time collection with two vehicles passing link 2.
    /// Vehicle 1: 2s
    /// Vehicle 2: 4s
    fn test_two_vehicles() {
        let link1 = Id::create("1");
        let link2 = Id::create("2");

        let vehicle1 = Id::create("1");
        let vehicle2 = Id::create("2");
        let vehicle3 = Id::create("3");

        let mut collector = TravelTimeCollector::new();
        collector.receive_event(1, &Event::new_link_leave(&link1, &vehicle1));
        // vehicle 1 enters
        collector.receive_event(2, &Event::new_link_enter(&link2, &vehicle1));
        // vehicle 2 enters
        collector.receive_event(3, &Event::new_link_enter(&link2, &vehicle2));
        // vehicle 1 leaves
        collector.receive_event(4, &Event::new_link_leave(&link2, &vehicle1));
        // vehicle 3 enters
        collector.receive_event(5, &Event::new_link_enter(&link2, &vehicle3));
        // vehicle 2 leaves
        collector.receive_event(7, &Event::new_link_leave(&link2, &vehicle2));

        // The average travel time on link 2 is 3
        assert_eq!(Some(3), collector.get_travel_time_of_link(&link2));
        assert_eq!(None, collector.get_travel_time_of_link(&link1));

        assert_eq!(collector.get_travel_times().keys().len(), 1);
        assert_eq!(collector.get_travel_times().get(&link2), Some(&3u32));

        // vehicle 1 and 2 have no cached traffic information
        assert_eq!(collector.cache_enter_time_by_vehicle.get(&vehicle1), None);
        assert_eq!(collector.cache_enter_time_by_vehicle.get(&vehicle2), None);

        // vehicle 3 has cached traffic information
        assert_eq!(
            collector
                .cache_enter_time_by_vehicle
                .get(&vehicle3)
                .unwrap(),
            &5
        )
    }

    #[test]
    /// Tests whether PersonLeavesVehicleEvent discards travel time
    fn test_with_person_leaves_vehicle() {
        let link1 = Id::create("1");
        let vehicle1 = Id::create("1");
        let person1 = Id::create("p1");

        let mut collector = TravelTimeCollector::new();
        collector.receive_event(0, &Event::new_link_enter(&link1, &vehicle1));
        collector.receive_event(2, &Event::new_person_leaves_veh(&person1, &vehicle1));
        collector.receive_event(4, &Event::new_link_leave(&link1, &vehicle1));

        assert_eq!(collector.get_travel_time_of_link(&link1), None);
        assert_eq!(collector.cache_enter_time_by_vehicle.get(&vehicle1), None);
    }

    #[test]
    /// Tests whether PersonLeavesVehicleEvent discards travel time
    fn test_with_person_leaves_vehicle_complex() {
        let link1 = Id::create("1");
        let link2 = Id::create("2");
        let vehicle1 = Id::create("1");
        let vehicle2 = Id::create("2");
        let person1 = Id::create("p1");

        let mut collector = TravelTimeCollector::new();
        collector.receive_event(0, &Event::new_link_enter(&link1, &vehicle1));

        //intermediate veh 2 enters link 1
        collector.receive_event(1, &Event::new_link_enter(&link1, &vehicle2));

        collector.receive_event(2, &Event::new_person_leaves_veh(&person1, &vehicle1));

        //intermediate veh 2 leaves link 1
        collector.receive_event(3, &Event::new_link_leave(&link1, &vehicle2));

        collector.receive_event(10, &Event::new_link_leave(&link1, &vehicle1));
        collector.receive_event(10, &Event::new_link_enter(&link2, &vehicle1));
        collector.receive_event(20, &Event::new_link_leave(&link2, &vehicle1));

        assert_eq!(collector.get_travel_time_of_link(&link1), Some(2));
        assert_eq!(collector.get_travel_time_of_link(&link2), Some(10));
        assert_eq!(collector.cache_enter_time_by_vehicle.get(&vehicle1), None);
    }
}
