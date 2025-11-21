use crate::simulation::events::{
    LinkEnterEvent, LinkLeaveEvent, OnEventFnBuilder, PersonLeavesVehicleEvent,
};
use crate::simulation::id::Id;
use crate::simulation::network::Link;
use crate::simulation::vehicles::InternalVehicle;
use nohash_hasher::IntMap;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct TravelTimeCollector {
    travel_times_by_link: HashMap<Id<Link>, Vec<u32>>,
    cache_enter_time_by_vehicle: IntMap<Id<InternalVehicle>, u32>,
}

impl Default for TravelTimeCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)]
impl TravelTimeCollector {
    pub fn new() -> Self {
        TravelTimeCollector {
            travel_times_by_link: HashMap::new(),
            cache_enter_time_by_vehicle: IntMap::default(),
        }
    }

    fn process_link_enter_event(&mut self, event: &LinkEnterEvent) {
        // link enter events will always be stored
        let time = event.time;
        self.cache_enter_time_by_vehicle
            .insert(event.vehicle.clone(), time);
    }

    fn process_link_leave_event(&mut self, event: &LinkLeaveEvent) {
        // remove traffic information of link enter and compute travel time (assumes that a vehicle will leave a link before it enters the same again)
        // if it's None, the LinkLeaveEvent is the begin of a leg, thus no travel time can be computed
        let time = event.time;
        if let Some(t) = self.cache_enter_time_by_vehicle.remove(&event.vehicle) {
            self.travel_times_by_link
                .entry(event.link.clone())
                .or_default()
                .push(time - t)
        }
    }

    fn process_person_leaves_vehicle_event(&mut self, event: &PersonLeavesVehicleEvent) {
        self.cache_enter_time_by_vehicle.remove(&event.vehicle);
    }

    fn get_travel_time_of_link(&self, link: &Id<Link>) -> Option<u32> {
        match self.travel_times_by_link.get(link) {
            None => None,
            Some(travel_times) => {
                let sum: u32 = travel_times.iter().sum();
                let len = travel_times.len();
                Some(sum / (len as u32))
            }
        }
    }

    fn get_travel_times(&self) -> HashMap<Id<Link>, u32> {
        self.travel_times_by_link
            .keys()
            .map(|id| (id.clone(), self.get_travel_time_of_link(id)))
            .filter(|(_, travel_time)| travel_time.is_some())
            .map(|(id, travel_time)| (id, travel_time.unwrap()))
            .collect::<HashMap<Id<Link>, u32>>()
    }

    fn flush(&mut self) {
        // Collected travel times will be dropped, but cached values not.
        // Vehicles of cached values haven't left the corresponding links yet.
        // A travel time of a link is considered when a vehicle leaves the link.
        self.travel_times_by_link = HashMap::new();
    }

    pub fn register() -> Box<OnEventFnBuilder> {
        Box::new(move |e| {
            let ttc = Rc::new(RefCell::new(TravelTimeCollector::new()));

            let ttc1 = ttc.clone();
            let ttc2 = ttc.clone();

            e.on::<LinkEnterEvent, _>(move |ev| {
                let e = ev.as_any().downcast_ref::<LinkEnterEvent>().unwrap();
                ttc.borrow_mut().process_link_enter_event(e);
            });
            e.on::<LinkLeaveEvent, _>(move |ev| {
                let e = ev.as_any().downcast_ref::<LinkLeaveEvent>().unwrap();
                ttc1.borrow_mut().process_link_leave_event(e);
            });
            e.on::<PersonLeavesVehicleEvent, _>(move |ev| {
                let e = ev
                    .as_any()
                    .downcast_ref::<PersonLeavesVehicleEvent>()
                    .unwrap();
                ttc2.borrow_mut().process_person_leaves_vehicle_event(e);
            })
        })
    }
}

#[cfg(test)]
mod test {
    use crate::simulation::events::{LinkEnterEvent, LinkLeaveEvent, PersonLeavesVehicleEvent};
    use crate::simulation::id::Id;
    use crate::simulation::network::Link;
    use crate::simulation::population::InternalPerson;
    use crate::simulation::replanning::routing::travel_time_collector::TravelTimeCollector;
    use crate::simulation::vehicles::InternalVehicle;
    use crate::simulation::InternalAttributes;
    use macros::integration_test;

    fn link_enter_event(
        time: u32,
        link: &Id<Link>,
        vehicle: &Id<InternalVehicle>,
    ) -> LinkEnterEvent {
        LinkEnterEvent {
            time,
            link: link.clone(),
            vehicle: vehicle.clone(),
            attributes: InternalAttributes::default(),
        }
    }

    fn link_leave_event(
        time: u32,
        link: &Id<Link>,
        vehicle: &Id<InternalVehicle>,
    ) -> LinkLeaveEvent {
        LinkLeaveEvent {
            time,
            link: link.clone(),
            vehicle: vehicle.clone(),
            attributes: InternalAttributes::default(),
        }
    }

    fn person_leaves_vehicle_event(
        time: u32,
        person: &Id<InternalPerson>,
        vehicle: &Id<InternalVehicle>,
    ) -> PersonLeavesVehicleEvent {
        PersonLeavesVehicleEvent {
            time,
            person: person.clone(),
            vehicle: vehicle.clone(),
            attributes: InternalAttributes::default(),
        }
    }

    #[integration_test]
    fn test_one_vehicle() {
        let link1 = Id::create("1");
        let link2 = Id::create("2");
        let vehicle1 = Id::create("1");

        let mut collector = TravelTimeCollector::new();
        collector.process_link_leave_event(&link_leave_event(1, &link1, &vehicle1));
        collector.process_link_enter_event(&link_enter_event(2, &link2, &vehicle1));
        collector.process_link_leave_event(&link_leave_event(4, &link2, &vehicle1));

        assert_eq!(collector.get_travel_time_of_link(&link2), Some(2));
        assert_eq!(collector.get_travel_time_of_link(&link1), None);
        assert_eq!(collector.get_travel_times().keys().len(), 1);
        assert_eq!(collector.get_travel_times().get(&link2), Some(&2u32));
        assert_eq!(collector.cache_enter_time_by_vehicle.get(&vehicle1), None)
    }

    #[integration_test]
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
        collector.process_link_leave_event(&link_leave_event(1, &link1, &vehicle1));
        collector.process_link_enter_event(&link_enter_event(2, &link2, &vehicle1));
        collector.process_link_enter_event(&link_enter_event(3, &link2, &vehicle2));
        collector.process_link_leave_event(&link_leave_event(4, &link2, &vehicle1));
        collector.process_link_enter_event(&link_enter_event(5, &link2, &vehicle3));
        collector.process_link_leave_event(&link_leave_event(7, &link2, &vehicle2));

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

    #[integration_test]
    /// Tests whether PersonLeavesVehicleEvent discards travel time
    fn test_with_person_leaves_vehicle() {
        let link1 = Id::create("1");
        let vehicle1 = Id::create("1");
        let person1 = Id::create("p1");

        let mut collector = TravelTimeCollector::new();
        collector.process_link_enter_event(&link_enter_event(0, &link1, &vehicle1));
        collector.process_person_leaves_vehicle_event(&person_leaves_vehicle_event(
            2, &person1, &vehicle1,
        ));
        collector.process_link_leave_event(&link_leave_event(4, &link1, &vehicle1));

        assert_eq!(collector.get_travel_time_of_link(&link1), None);
        assert_eq!(collector.cache_enter_time_by_vehicle.get(&vehicle1), None);
    }

    #[integration_test]
    /// Tests whether PersonLeavesVehicleEvent discards travel time
    fn test_with_person_leaves_vehicle_complex() {
        let link1 = Id::create("1");
        let link2 = Id::create("2");
        let vehicle1 = Id::create("1");
        let vehicle2 = Id::create("2");
        let person1 = Id::create("p1");

        let mut collector = TravelTimeCollector::new();
        collector.process_link_enter_event(&link_enter_event(0, &link1, &vehicle1));

        //intermediate veh 2 enters link 1
        collector.process_link_enter_event(&link_enter_event(1, &link1, &vehicle2));
        collector.process_person_leaves_vehicle_event(&person_leaves_vehicle_event(
            2, &person1, &vehicle1,
        ));

        //intermediate veh 2 leaves link 1
        collector.process_link_leave_event(&link_leave_event(3, &link1, &vehicle2));
        collector.process_link_leave_event(&link_leave_event(10, &link1, &vehicle1));
        collector.process_link_enter_event(&link_enter_event(10, &link2, &vehicle1));
        collector.process_link_leave_event(&link_leave_event(20, &link2, &vehicle1));

        assert_eq!(collector.get_travel_time_of_link(&link1), Some(2));
        assert_eq!(collector.get_travel_time_of_link(&link2), Some(10));
        assert_eq!(collector.cache_enter_time_by_vehicle.get(&vehicle1), None);
    }
}
