use crate::simulation::events::{
    LinkEnterEvent, LinkLeaveEvent, VehicleEntersTrafficEvent, VehicleLeavesTrafficEvent,
};
use crate::simulation::id::Id;
use crate::simulation::scenario::network::Link;
use crate::simulation::scenario::vehicles::InternalVehicle;
use crate::simulation::time::SimTime;
use nohash_hasher::{IntMap, IntSet};
use std::time::Duration;

#[derive(Clone, Debug)]
struct ActiveLinkEnter {
    link: Id<Link>,
    time: SimTime,
}

#[derive(Clone, Debug, Default)]
struct TravelTimeBin {
    mean_nanos: f64,
    count: u64,
}

#[derive(Debug)]
struct TravelTimeData {
    travel_time_bins: Vec<TravelTimeBin>,
    consolidated_travel_times: Option<Vec<Duration>>,
    dirty: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TravelTimeGetter {
    Average,
    LinearInterpolation,
}

#[derive(Debug)]
pub struct TravelTimeCalculator {
    modes: IntSet<Id<String>>,
    bin_size: Duration,
    num_bins: usize,
    active_link_enters_by_vehicle: IntMap<Id<InternalVehicle>, ActiveLinkEnter>,
    ignored_vehicles: IntSet<Id<InternalVehicle>>,
    travel_time_data_by_link: IntMap<Id<Link>, TravelTimeData>,
}

#[allow(dead_code)]
impl TravelTimeCalculator {
    pub fn new(modes: IntSet<Id<String>>, bin_size: Duration, max_time: Duration) -> Self {
        assert!(
            bin_size > Duration::ZERO,
            "travel time bin size must be greater than zero"
        );

        let num_bins = ((max_time.as_nanos() / bin_size.as_nanos()) + 1)
            .try_into()
            .expect("number of travel time bins does not fit into usize");

        TravelTimeCalculator {
            modes,
            bin_size,
            num_bins,
            active_link_enters_by_vehicle: IntMap::default(),
            ignored_vehicles: IntSet::default(),
            travel_time_data_by_link: IntMap::default(),
        }
    }

    pub fn process_vehicle_enters_traffic_event(&mut self, event: &VehicleEntersTrafficEvent) {
        if self.modes.is_empty() || self.modes.contains(&event.network_mode) {
            self.ignored_vehicles.remove(&event.vehicle);
        } else {
            // If the mode is not in the set of modes we are interested in, we ignore the vehicle.
            self.ignored_vehicles.insert(event.vehicle.clone());
            self.active_link_enters_by_vehicle.remove(&event.vehicle);
        }
    }

    pub fn process_link_enter_event(&mut self, event: &LinkEnterEvent) {
        if self.ignored_vehicles.contains(&event.vehicle) {
            return;
        }

        self.active_link_enters_by_vehicle.insert(
            event.vehicle.clone(),
            ActiveLinkEnter {
                link: event.link.clone(),
                time: event.time,
            },
        );
    }

    pub fn process_link_leave_event(&mut self, event: &LinkLeaveEvent) {
        let Some(active_enter) = self.active_link_enters_by_vehicle.remove(&event.vehicle) else {
            // Return if vehicle didn't enter link before, i.e., this is the first link leave after activity.
            return;
        };

        let travel_time = event.time.duration_since(active_enter.time);
        let time_slot = self.time_slot(active_enter.time);
        self.travel_time_data_by_link
            .entry(active_enter.link)
            .or_insert_with(|| TravelTimeData::new(self.num_bins))
            .observe(time_slot, travel_time);
    }

    pub fn process_vehicle_leaves_traffic_event(&mut self, event: &VehicleLeavesTrafficEvent) {
        self.clear_vehicle_state(&event.vehicle);
    }

    pub fn get_link_travel_time(
        &mut self,
        link: &Link,
        now: SimTime,
        vehicle: Option<&InternalVehicle>,
        getter: TravelTimeGetter,
    ) -> Duration {
        let time_slot = self.time_slot(now);
        let bin_size = self.bin_size;
        let observed = self
            .travel_time_data_by_link
            .entry(link.id.clone())
            .or_insert_with(|| TravelTimeData::new(self.num_bins))
            .get_travel_time(link, time_slot, now, bin_size, getter);

        if let Some(vehicle) = vehicle {
            if vehicle.max_v.is_finite() && vehicle.max_v > 0.0 {
                return observed.max(travel_time_from_speed(link.length, vehicle.max_v));
            }
        }

        observed
    }

    pub fn flush(&mut self) {
        for data in self.travel_time_data_by_link.values_mut() {
            data.flush();
        }
    }

    fn clear_vehicle_state(&mut self, vehicle: &Id<InternalVehicle>) {
        self.active_link_enters_by_vehicle.remove(vehicle);
        self.ignored_vehicles.remove(vehicle);
    }

    /// Returns the index of the bin corresponding to the time.
    fn time_slot(&self, time: SimTime) -> usize {
        let slot = time.as_duration().as_nanos() / self.bin_size.as_nanos();
        let slot = usize::try_from(slot).unwrap_or(usize::MAX);
        slot.min(self.num_bins - 1)
    }
}

impl TravelTimeData {
    fn new(num_bins: usize) -> Self {
        TravelTimeData {
            travel_time_bins: vec![TravelTimeBin::default(); num_bins],
            consolidated_travel_times: None,
            dirty: true,
        }
    }

    fn observe(&mut self, slot: usize, travel_time: Duration) {
        let bin = &mut self.travel_time_bins[slot];
        let next_count = bin.count + 1;
        bin.mean_nanos = ((bin.mean_nanos * bin.count as f64) + duration_to_nanos_f64(travel_time))
            / next_count as f64;
        bin.count = next_count;
        self.dirty = true;
    }

    fn get_travel_time(
        &mut self,
        link: &Link,
        slot: usize,
        now: SimTime,
        bin_size: Duration,
        getter: TravelTimeGetter,
    ) -> Duration {
        let travel_times = self.consolidated_travel_times(link, bin_size);
        match getter {
            TravelTimeGetter::Average => travel_times[slot],
            TravelTimeGetter::LinearInterpolation => {
                interpolated_travel_time(travel_times, now, bin_size)
            }
        }
    }

    fn flush(&mut self) {
        self.travel_time_bins.fill(TravelTimeBin::default());
        self.consolidated_travel_times = None;
        self.dirty = true;
    }

    fn consolidated_travel_times(&mut self, link: &Link, bin_size: Duration) -> &[Duration] {
        if self.dirty || self.consolidated_travel_times.is_none() {
            self.consolidated_travel_times =
                Some(self.build_consolidated_travel_times(link, bin_size));
            self.dirty = false;
        }

        self.consolidated_travel_times
            .as_deref()
            .expect("consolidated travel times must exist after update")
    }

    fn build_consolidated_travel_times(&self, link: &Link, bin_size: Duration) -> Vec<Duration> {
        let freespeed_travel_time = travel_time_from_speed(link.length, link.freespeed);
        let mut result = vec![freespeed_travel_time; self.travel_time_bins.len()];

        for (i, bin) in self.travel_time_bins.iter().enumerate() {
            if bin.count > 0 {
                result[i] = Duration::from_secs_f64(bin.mean_nanos / 1_000_000_000.0);
            }
        }

        // MATSim does not let an empty bin immediately fall back to freespeed after a very slow
        // observed bin. Instead, each following bin may drop by at most one bin size. For example,
        // with 900s bins and a 3000s observation in bin 0, empty later bins become 2100s, 1200s,
        // 300s, and only then freespeed again.
        for i in 1..result.len() {
            let lower_bound = result[i - 1].saturating_sub(bin_size);
            if result[i] < lower_bound {
                result[i] = lower_bound;
            }
        }

        result
    }
}

fn duration_to_nanos_f64(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000_000_000.0
}

fn travel_time_from_speed(length: f64, speed: f64) -> Duration {
    assert!(
        speed.is_finite() && speed > 0.0,
        "speed must be finite and greater than zero"
    );
    Duration::from_secs_f64(length / speed)
}

fn interpolated_travel_time(
    travel_times: &[Duration],
    now: SimTime,
    bin_size: Duration,
) -> Duration {
    if travel_times.len() == 1 {
        return travel_times[0];
    }

    let bin_size_nanos = duration_to_nanos_f64(bin_size);
    let now_nanos = duration_to_nanos_f64(now.as_duration());
    let first_center = bin_size_nanos * 0.5;
    let last_center = (travel_times.len() as f64 - 0.5) * bin_size_nanos;

    if now_nanos <= first_center {
        return travel_times[0];
    }
    if now_nanos >= last_center {
        return *travel_times.last().unwrap();
    }

    let lower = ((now_nanos / bin_size_nanos) - 0.5).floor() as usize;
    let upper = lower + 1;
    let lower_center = (lower as f64 + 0.5) * bin_size_nanos;
    let fraction = (now_nanos - lower_center) / bin_size_nanos;

    let lower_nanos = duration_to_nanos_f64(travel_times[lower]);
    let upper_nanos = duration_to_nanos_f64(travel_times[upper]);
    Duration::from_secs_f64(
        ((lower_nanos + ((upper_nanos - lower_nanos) * fraction)) / 1_000_000_000.0).max(0.0),
    )
}

#[cfg(test)]
mod test {
    use crate::simulation::InternalAttributes;
    use crate::simulation::events::{
        LinkEnterEvent, LinkLeaveEvent, VehicleEntersTrafficEvent, VehicleLeavesTrafficEvent,
    };
    use crate::simulation::id::Id;
    use crate::simulation::replanning::routing::travel_time_collector::{
        TravelTimeCalculator, TravelTimeGetter,
    };
    use crate::simulation::scenario::network::{Link, Node};
    use crate::simulation::scenario::population::InternalPerson;
    use crate::simulation::scenario::vehicles::InternalVehicle;
    use crate::simulation::time::SimTime;
    use macros::deterministic_id_test;
    use nohash_hasher::IntSet;
    use std::time::Duration;

    use super::TravelTimeData;

    fn calculator(
        modes: IntSet<Id<String>>,
        bin_size_secs: u64,
        max_time_secs: u64,
    ) -> TravelTimeCalculator {
        TravelTimeCalculator::new(
            modes,
            Duration::from_secs(bin_size_secs),
            Duration::from_secs(max_time_secs),
        )
    }

    fn link(id: &str, length: f64, freespeed: f64) -> Link {
        Link {
            id: Id::create(id),
            from: Id::<Node>::create(&format!("{id}_from")),
            to: Id::<Node>::create(&format!("{id}_to")),
            length,
            capacity: 3600.0,
            freespeed,
            permlanes: 1.0,
            modes: IntSet::default(),
            partition: 0,
            attributes: InternalAttributes::default(),
        }
    }

    fn vehicle(id: &str, max_v: f64) -> InternalVehicle {
        InternalVehicle {
            id: Id::create(id),
            max_v,
            pce: 1.0,
            vehicle_type: Id::create("default"),
            attributes: InternalAttributes::default(),
        }
    }

    fn link_enter_event(
        time: SimTime,
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
        time: SimTime,
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

    fn vehicle_enters_traffic_event(
        time: SimTime,
        link: &Id<Link>,
        person: &Id<InternalPerson>,
        vehicle: &Id<InternalVehicle>,
        network_mode: &Id<String>,
    ) -> VehicleEntersTrafficEvent {
        VehicleEntersTrafficEvent {
            time,
            vehicle: vehicle.clone(),
            link: link.clone(),
            person: person.clone(),
            network_mode: network_mode.clone(),
            relative_position: 1.0,
            attributes: InternalAttributes::default(),
        }
    }

    fn vehicle_leaves_traffic_event(
        time: SimTime,
        link: &Id<Link>,
        person: &Id<InternalPerson>,
        vehicle: &Id<InternalVehicle>,
        network_mode: &Id<String>,
    ) -> VehicleLeavesTrafficEvent {
        VehicleLeavesTrafficEvent {
            time,
            vehicle: vehicle.clone(),
            link: link.clone(),
            person: person.clone(),
            network_mode: network_mode.clone(),
            relative_position: 1.0,
            attributes: InternalAttributes::default(),
        }
    }

    fn average_travel_time(
        calculator: &mut TravelTimeCalculator,
        link: &Link,
        now_secs: u64,
    ) -> Duration {
        calculator.get_link_travel_time(
            link,
            SimTime::from_secs(now_secs),
            None,
            TravelTimeGetter::Average,
        )
    }

    #[deterministic_id_test]
    fn leave_without_enter_is_ignored_and_empty_link_uses_freespeed() {
        let link = link("1", 100.0, 10.0);
        let vehicle = vehicle("v1", 20.0);
        let mut calculator = calculator(IntSet::default(), 10, 100);

        calculator.process_link_leave_event(&link_leave_event(
            SimTime::from_secs(5),
            &link.id,
            &vehicle.id,
        ));

        assert_eq!(
            Duration::from_secs(10),
            average_travel_time(&mut calculator, &link, 0)
        );
    }

    #[deterministic_id_test]
    fn records_travel_time_in_enter_time_slot() {
        let link = link("1", 100.0, 100.0);
        let vehicle = vehicle("v1", 20.0);

        // 10 bin à 10s
        let mut calculator = calculator(IntSet::default(), 10, 100);

        // in bin 1
        calculator.process_link_enter_event(&link_enter_event(
            SimTime::from_secs(9),
            &link.id,
            &vehicle.id,
        ));

        // in bin 2
        calculator.process_link_leave_event(&link_leave_event(
            SimTime::from_secs(14),
            &link.id,
            &vehicle.id,
        ));

        // observed travel time registered in bin 1
        assert_eq!(
            Duration::from_secs(5),
            average_travel_time(&mut calculator, &link, 0)
        );

        // freespeed travel time registered in bin 2
        assert_eq!(
            Duration::from_secs(1),
            average_travel_time(&mut calculator, &link, 10)
        );
    }

    #[deterministic_id_test]
    fn calculates_running_mean_per_link_and_slot() {
        let link = link("1", 100.0, 100.0);
        let vehicle1 = vehicle("v1", 20.0);
        let vehicle2 = vehicle("v2", 20.0);
        let mut calculator = calculator(IntSet::default(), 10, 100);

        // Travel time 2
        calculator.process_link_enter_event(&link_enter_event(
            SimTime::from_secs(2),
            &link.id,
            &vehicle1.id,
        ));
        calculator.process_link_leave_event(&link_leave_event(
            SimTime::from_secs(4),
            &link.id,
            &vehicle1.id,
        ));

        // Travel time 4
        calculator.process_link_enter_event(&link_enter_event(
            SimTime::from_secs(3),
            &link.id,
            &vehicle2.id,
        ));
        calculator.process_link_leave_event(&link_leave_event(
            SimTime::from_secs(7),
            &link.id,
            &vehicle2.id,
        ));

        // mean travel time is 3
        assert_eq!(
            Duration::from_secs(3),
            average_travel_time(&mut calculator, &link, 0)
        );
    }

    #[deterministic_id_test]
    fn clamps_enter_time_slot_to_max_time() {
        let link = link("1", 100.0, 100.0);
        let vehicle = vehicle("v1", 20.0);
        let mut calculator = calculator(IntSet::default(), 10, 25);

        calculator.process_link_enter_event(&link_enter_event(
            SimTime::from_secs(99),
            &link.id,
            &vehicle.id,
        ));
        calculator.process_link_leave_event(&link_leave_event(
            SimTime::from_secs(104),
            &link.id,
            &vehicle.id,
        ));

        assert_eq!(
            Duration::from_secs(5),
            average_travel_time(&mut calculator, &link, 20)
        );
    }

    #[deterministic_id_test]
    fn filters_link_events_by_vehicle_network_mode() {
        let link = link("1", 100.0, 100.0);
        let person: Id<InternalPerson> = Id::create("p1");
        let vehicle = vehicle("v1", 20.0);
        let car: Id<String> = Id::create("car");
        let bike: Id<String> = Id::create("bike");
        let mut modes = IntSet::default();
        modes.insert(car.clone());
        let mut calculator = calculator(modes, 10, 100);

        calculator.process_vehicle_enters_traffic_event(&vehicle_enters_traffic_event(
            SimTime::from_secs(0),
            &link.id,
            &person,
            &vehicle.id,
            &bike,
        ));
        calculator.process_link_enter_event(&link_enter_event(
            SimTime::from_secs(0),
            &link.id,
            &vehicle.id,
        ));
        calculator.process_link_leave_event(&link_leave_event(
            SimTime::from_secs(9),
            &link.id,
            &vehicle.id,
        ));

        // returns freespeed travel time because the vehicle was ignored due to its network mode
        assert_eq!(
            Duration::from_secs(1),
            average_travel_time(&mut calculator, &link, 0)
        );

        calculator.process_vehicle_enters_traffic_event(&vehicle_enters_traffic_event(
            SimTime::from_secs(10),
            &link.id,
            &person,
            &vehicle.id,
            &car,
        ));
        calculator.process_link_enter_event(&link_enter_event(
            SimTime::from_secs(10),
            &link.id,
            &vehicle.id,
        ));
        calculator.process_link_leave_event(&link_leave_event(
            SimTime::from_secs(15),
            &link.id,
            &vehicle.id,
        ));

        // returns the observed travel time
        assert_eq!(
            Duration::from_secs(5),
            average_travel_time(&mut calculator, &link, 10)
        );
    }

    #[deterministic_id_test]
    fn vehicle_leaves_traffic_discards_active_enter() {
        let link = link("1", 100.0, 100.0);
        let person: Id<InternalPerson> = Id::create("p1");
        let vehicle = vehicle("v1", 20.0);
        let car: Id<String> = Id::create("car");
        let mut calculator = calculator(IntSet::default(), 10, 100);

        calculator.process_link_enter_event(&link_enter_event(
            SimTime::from_secs(0),
            &link.id,
            &vehicle.id,
        ));
        calculator.process_vehicle_leaves_traffic_event(&vehicle_leaves_traffic_event(
            SimTime::from_secs(2),
            &link.id,
            &person,
            &vehicle.id,
            &car,
        ));
        calculator.process_link_leave_event(&link_leave_event(
            SimTime::from_secs(5),
            &link.id,
            &vehicle.id,
        ));

        assert_eq!(
            Duration::from_secs(1),
            average_travel_time(&mut calculator, &link, 0)
        );
    }

    #[deterministic_id_test]
    fn consolidation_cascades_previous_bin_minus_bin_size() {
        let link = link("1", 100.0, 100.0);
        let vehicle = vehicle("v1", 20.0);
        let mut calculator = calculator(IntSet::default(), 900, 3600);

        calculator.process_link_enter_event(&link_enter_event(
            SimTime::from_secs(0),
            &link.id,
            &vehicle.id,
        ));
        calculator.process_link_leave_event(&link_leave_event(
            SimTime::from_secs(3000),
            &link.id,
            &vehicle.id,
        ));

        assert_eq!(
            Duration::from_secs(3000),
            average_travel_time(&mut calculator, &link, 0)
        );
        assert_eq!(
            Duration::from_secs(2100),
            average_travel_time(&mut calculator, &link, 900)
        );
        assert_eq!(
            Duration::from_secs(1200),
            average_travel_time(&mut calculator, &link, 1800)
        );
        assert_eq!(
            Duration::from_secs(300),
            average_travel_time(&mut calculator, &link, 2700)
        );
        assert_eq!(
            Duration::from_secs(1),
            average_travel_time(&mut calculator, &link, 3600)
        );
    }

    #[deterministic_id_test]
    fn linear_interpolation_uses_neighboring_bin_midpoints() {
        let link = link("1", 100.0, 100.0);
        let vehicle1 = vehicle("v1", 20.0);
        let vehicle2 = vehicle("v2", 20.0);
        let mut calculator = calculator(IntSet::default(), 10, 100);

        calculator.process_link_enter_event(&link_enter_event(
            SimTime::from_secs(0),
            &link.id,
            &vehicle1.id,
        ));
        calculator.process_link_leave_event(&link_leave_event(
            SimTime::from_secs(10),
            &link.id,
            &vehicle1.id,
        ));
        calculator.process_link_enter_event(&link_enter_event(
            SimTime::from_secs(10),
            &link.id,
            &vehicle2.id,
        ));
        calculator.process_link_leave_event(&link_leave_event(
            SimTime::from_secs(30),
            &link.id,
            &vehicle2.id,
        ));

        assert_eq!(
            Duration::from_secs(15),
            calculator.get_link_travel_time(
                &link,
                SimTime::from_secs(10),
                None,
                TravelTimeGetter::LinearInterpolation,
            )
        );
    }

    #[deterministic_id_test]
    fn vehicle_max_speed_clamps_empirical_travel_time() {
        let link = link("1", 100.0, 100.0);
        let slow_vehicle = vehicle("v1", 10.0);
        let mut calculator = calculator(IntSet::default(), 10, 100);

        calculator.process_link_enter_event(&link_enter_event(
            SimTime::from_secs(0),
            &link.id,
            &slow_vehicle.id,
        ));
        calculator.process_link_leave_event(&link_leave_event(
            SimTime::from_secs(1),
            &link.id,
            &slow_vehicle.id,
        ));

        assert_eq!(
            Duration::from_secs(10),
            calculator.get_link_travel_time(
                &link,
                SimTime::from_secs(0),
                Some(&slow_vehicle),
                TravelTimeGetter::Average,
            )
        );
    }

    #[deterministic_id_test]
    fn data_running_mean_per_slot() {
        let link = link("1", 100.0, 100.0);
        let mut data = TravelTimeData::new(3);

        data.observe(0, Duration::from_secs(2));
        data.observe(0, Duration::from_secs(4));

        assert_eq!(
            Duration::from_secs(3),
            data.get_travel_time(
                &link,
                0,
                SimTime::from_secs(0),
                Duration::from_secs(10),
                TravelTimeGetter::Average,
            )
        );
    }

    #[deterministic_id_test]
    fn data_empty_bins_use_freespeed() {
        let link = link("1", 100.0, 10.0);
        let mut data = TravelTimeData::new(3);

        assert_eq!(
            Duration::from_secs(10),
            data.get_travel_time(
                &link,
                1,
                SimTime::from_secs(10),
                Duration::from_secs(10),
                TravelTimeGetter::Average,
            )
        );
    }

    #[deterministic_id_test]
    fn data_consolidation_cascades_previous_bin_minus_bin_size() {
        let link = link("1", 100.0, 100.0);
        let mut data = TravelTimeData::new(4);
        let bin_size = Duration::from_secs(900);

        data.observe(0, Duration::from_secs(3000));

        assert_eq!(
            Duration::from_secs(2100),
            data.get_travel_time(
                &link,
                1,
                SimTime::from_secs(900),
                bin_size,
                TravelTimeGetter::Average,
            )
        );
        assert_eq!(
            Duration::from_secs(1200),
            data.get_travel_time(
                &link,
                2,
                SimTime::from_secs(1800),
                bin_size,
                TravelTimeGetter::Average,
            )
        );
        assert_eq!(
            Duration::from_secs(300),
            data.get_travel_time(
                &link,
                3,
                SimTime::from_secs(2700),
                bin_size,
                TravelTimeGetter::Average,
            )
        );
    }

    #[deterministic_id_test]
    fn data_linear_interpolation_uses_neighboring_bin_midpoints() {
        let link = link("1", 100.0, 100.0);
        let mut data = TravelTimeData::new(3);

        data.observe(0, Duration::from_secs(10));
        data.observe(1, Duration::from_secs(20));

        assert_eq!(
            Duration::from_secs(15),
            data.get_travel_time(
                &link,
                1,
                SimTime::from_secs(10),
                Duration::from_secs(10),
                TravelTimeGetter::LinearInterpolation,
            )
        );
    }

    #[deterministic_id_test]
    fn data_reconsolidates_after_new_observation() {
        let link = link("1", 100.0, 100.0);
        let mut data = TravelTimeData::new(3);

        data.observe(0, Duration::from_secs(10));
        assert_eq!(
            Duration::from_secs(10),
            data.get_travel_time(
                &link,
                0,
                SimTime::from_secs(0),
                Duration::from_secs(10),
                TravelTimeGetter::Average,
            )
        );

        data.observe(0, Duration::from_secs(20));
        assert_eq!(
            Duration::from_secs(15),
            data.get_travel_time(
                &link,
                0,
                SimTime::from_secs(0),
                Duration::from_secs(10),
                TravelTimeGetter::Average,
            )
        );
    }

    #[deterministic_id_test]
    fn data_flush_clears_observations() {
        let link = link("1", 100.0, 100.0);
        let mut data = TravelTimeData::new(3);

        data.observe(0, Duration::from_secs(10));
        data.flush();

        assert_eq!(
            Duration::from_secs(1),
            data.get_travel_time(
                &link,
                0,
                SimTime::from_secs(0),
                Duration::from_secs(10),
                TravelTimeGetter::Average,
            )
        );
    }
}
