use crate::simulation::id::Id;
use crate::simulation::io::xml::facilities::{
    IOFacilities, IOFacility, IOFacilityActivity, IOOpenDay, IOOpenTime,
};
use crate::simulation::pt::TransitStopFacility;
use crate::simulation::scenario::Coordinate;
use crate::simulation::scenario::network::Link;
use crate::simulation::time::SimTime;
use crate::simulation::{Attributable, Identifiable, InternalAttributes};
use nohash_hasher::IntMap;

/// Facility is a location that has modal access to the network.
#[derive(Debug, Clone, PartialEq)]
pub enum Facility {
    LinkWrapperFacility(LinkWrapperFacility),
    ActivityFacility(ActivityFacility),
    TransitFacility(TransitStopFacility),
}

impl Facility {
    pub fn coord(&self) -> &Coordinate {
        match self {
            Facility::LinkWrapperFacility(facility) => &facility.coord,
            Facility::ActivityFacility(facility) => &facility.coord,
            Facility::TransitFacility(facility) => &facility.coord,
        }
    }

    pub fn link(&self) -> &Id<Link> {
        match self {
            Facility::LinkWrapperFacility(facility) => &facility.link_id,
            Facility::ActivityFacility(facility) => &facility.link_id,
            Facility::TransitFacility(facility) => {
                facility.link_ref_id.as_ref().unwrap_or_else(|| {
                    panic!("Transit facility with id {} has no link id.", facility.id)
                })
            }
        }
    }

    pub fn modal_link(&self, mode: &Id<String>) -> Option<&Id<Link>> {
        match self {
            Facility::LinkWrapperFacility(facility) => facility.mode_to_link.get(mode),
            Facility::ActivityFacility(facility) => facility.mode_to_link.get(mode),
            Facility::TransitFacility(_) => None,
        }
    }

    pub fn new_link_wrapper_from(facility: &Facility, coordinate: Coordinate) -> Facility {
        let mut f = match facility {
            Facility::LinkWrapperFacility(f) => f.clone(),
            Facility::ActivityFacility(f) => f.into(),
            Facility::TransitFacility(f) => f.into(),
        };
        f.coord = coordinate;
        Facility::LinkWrapperFacility(f)
    }

    pub fn new_link_wrapper(coord: Coordinate, link_id: Id<Link>) -> Facility {
        Facility::LinkWrapperFacility(LinkWrapperFacility {
            coord,
            link_id,
            mode_to_link: IntMap::default(),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActivityFacilities {
    pub facilities: IntMap<Id<ActivityFacility>, ActivityFacility>,
    pub name: Option<String>,
    pub aggregation_layer: Option<String>,
    pub lang: Option<String>,
    pub attributes: InternalAttributes,
}

impl ActivityFacilities {
    pub fn new(
        name: Option<String>,
        aggregation_layer: Option<String>,
        lang: Option<String>,
        attributes: InternalAttributes,
    ) -> Self {
        Self {
            facilities: IntMap::default(),
            name,
            aggregation_layer,
            lang,
            attributes,
        }
    }

    pub fn add_facility(&mut self, facility: ActivityFacility) {
        let id = facility.id().clone();
        let previous = self.facilities.insert(id.clone(), facility);
        assert!(
            previous.is_none(),
            "Facility with id {} already exists.",
            id
        );
    }

    pub fn get(&self, id: &Id<ActivityFacility>) -> Option<&ActivityFacility> {
        self.facilities.get(id)
    }
}

impl Default for ActivityFacilities {
    fn default() -> Self {
        Self::new(None, None, None, InternalAttributes::default())
    }
}

impl From<IOFacilities> for ActivityFacilities {
    fn from(io: IOFacilities) -> Self {
        let mut facilities = ActivityFacilities::new(
            io.name,
            io.aggregation_layer,
            io.lang,
            io.attributes.map(Into::into).unwrap_or_default(),
        );

        for io_facility in io.facilities {
            facilities.add_facility(ActivityFacility::from(io_facility));
        }

        facilities
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActivityFacility {
    pub id: Id<ActivityFacility>,
    pub coord: Coordinate,
    pub link_id: Id<Link>,
    pub mode_to_link: IntMap<Id<String>, Id<Link>>,
    pub desc: Option<String>,
    pub activities: Vec<ActivityOption>,
    pub attributes: InternalAttributes,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ActivityOption {
    pub activity_type: Id<String>,
    pub capacity: Option<f64>,
    pub open_times: Vec<OpeningTime>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OpeningTime {
    pub day: OpenDay,
    pub start_time: SimTime,
    pub end_time: SimTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenDay {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
    Wkday,
    Wkend,
    Wk,
}

impl ActivityFacility {
    pub fn desc(&self) -> Option<&str> {
        self.desc.as_deref()
    }

    pub fn activities(&self) -> &[ActivityOption] {
        &self.activities
    }
}

impl Identifiable<ActivityFacility> for ActivityFacility {
    fn id(&self) -> &Id<ActivityFacility> {
        &self.id
    }
}

impl Attributable for ActivityFacility {
    fn attributes(&self) -> &InternalAttributes {
        &self.attributes
    }

    fn attributes_mut(&mut self) -> &mut InternalAttributes {
        &mut self.attributes
    }
}

impl From<IOFacility> for ActivityFacility {
    fn from(io: IOFacility) -> Self {
        let coord = match (io.x, io.y) {
            (Some(x), Some(y)) => Coordinate::new_3d(x, y, io.z.unwrap_or(0.0)),
            _ => panic!("Facility with id {} must have x and y coordinates.", io.id),
        };
        let link_id = io
            .link_id
            .unwrap_or_else(|| panic!("Facility with id {} must have a link id.", io.id));

        Id::<String>::create(&io.id);
        ActivityFacility {
            id: Id::create(&io.id),
            coord,
            link_id: Id::create(&link_id),
            mode_to_link: IntMap::default(),
            desc: io.desc,
            activities: io.activities.into_iter().map(Into::into).collect(),
            attributes: io.attributes.map(Into::into).unwrap_or_default(),
        }
    }
}

impl From<IOFacilityActivity> for ActivityOption {
    fn from(io: IOFacilityActivity) -> Self {
        ActivityOption {
            activity_type: Id::create(&io.activity_type),
            capacity: io.capacity.map(|capacity| capacity.value),
            open_times: io.open_times.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<IOOpenTime> for OpeningTime {
    fn from(io: IOOpenTime) -> Self {
        OpeningTime {
            day: OpenDay::from(io.day),
            start_time: parse_open_time(&io.start_time),
            end_time: parse_open_time(&io.end_time),
        }
    }
}

impl From<IOOpenDay> for OpenDay {
    fn from(day: IOOpenDay) -> Self {
        match day {
            IOOpenDay::Mon => OpenDay::Mon,
            IOOpenDay::Tue => OpenDay::Tue,
            IOOpenDay::Wed => OpenDay::Wed,
            IOOpenDay::Thu => OpenDay::Thu,
            IOOpenDay::Fri => OpenDay::Fri,
            IOOpenDay::Sat => OpenDay::Sat,
            IOOpenDay::Sun => OpenDay::Sun,
            IOOpenDay::Wkday => OpenDay::Wkday,
            IOOpenDay::Wkend => OpenDay::Wkend,
            IOOpenDay::Wk => OpenDay::Wk,
        }
    }
}

fn parse_open_time(value: &str) -> SimTime {
    SimTime::parse(value)
        .unwrap_or_else(|err| panic!("Invalid facility opentime value {value}: {err}"))
}

#[derive(Debug, Clone, PartialEq)]
pub struct LinkWrapperFacility {
    pub coord: Coordinate,
    pub link_id: Id<Link>,
    pub mode_to_link: IntMap<Id<String>, Id<Link>>,
}

impl From<&ActivityFacility> for LinkWrapperFacility {
    fn from(value: &ActivityFacility) -> Self {
        LinkWrapperFacility {
            coord: value.coord.clone(),
            link_id: value.link_id.clone(),
            mode_to_link: value.mode_to_link.clone(),
        }
    }
}

impl From<&TransitStopFacility> for LinkWrapperFacility {
    fn from(value: &TransitStopFacility) -> Self {
        LinkWrapperFacility {
            coord: value.coord.clone(),
            link_id: value
                .link_ref_id
                .clone()
                .unwrap_or_else(|| panic!("Transit facility with id {} has no link id.", value.id)),
            mode_to_link: IntMap::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::InternalAttributes;
    use crate::simulation::id::Id;
    use crate::simulation::io::xml::facilities::{
        IOCapacity, IOFacilities, IOFacility, IOFacilityActivity, IOOpenDay, IOOpenTime,
    };
    use crate::simulation::pt::TransitStopFacility;
    use crate::simulation::scenario::Coordinate;
    use crate::simulation::scenario::facilities::{
        ActivityFacilities, ActivityFacility, ActivityOption, Facility, LinkWrapperFacility,
        OpenDay,
    };
    use crate::simulation::scenario::network::Link;
    use crate::simulation::time::SimTime;
    use macros::integration_test;
    use nohash_hasher::IntMap;

    #[integration_test]
    fn conversion_creates_facilities_by_id() {
        let facilities = ActivityFacilities::from(IOFacilities {
            name: Some("test".to_string()),
            aggregation_layer: Some("parcel".to_string()),
            lang: Some("en-US".to_string()),
            attributes: None,
            facilities: vec![IOFacility {
                id: "f1".to_string(),
                x: Some(1.0),
                y: Some(2.0),
                z: None,
                link_id: Some("l1".to_string()),
                desc: Some("facility".to_string()),
                activities: vec![IOFacilityActivity {
                    activity_type: "work".to_string(),
                    capacity: Some(IOCapacity { value: 12.5 }),
                    open_times: vec![IOOpenTime {
                        day: IOOpenDay::Mon,
                        start_time: "08:00:00".to_string(),
                        end_time: "17:30:00".to_string(),
                    }],
                }],
                attributes: None,
            }],
        });

        let facility_id: Id<ActivityFacility> = Id::get_from_ext("f1");
        let facility = facilities.get(&facility_id).unwrap();

        assert_eq!(Coordinate::new_3d(1.0, 2.0, 0.0), facility.coord);
        assert_eq!(Id::<Link>::get_from_ext("l1"), facility.link_id);
        assert!(facility.mode_to_link.is_empty());
        assert_eq!("facility", facility.desc.as_deref().unwrap());
        assert_eq!(1, facility.activities.len());
        assert_eq!(
            Id::<String>::get_from_ext("work"),
            facility.activities[0].activity_type
        );
        assert_eq!(Some(12.5), facility.activities[0].capacity);
        assert_eq!(OpenDay::Mon, facility.activities[0].open_times[0].day);
        assert_eq!(
            SimTime::parse("17:30:00").unwrap(),
            facility.activities[0].open_times[0].end_time
        );
    }

    #[integration_test]
    #[should_panic(expected = "Facility with id f1 already exists.")]
    fn conversion_panics_on_duplicate_facility_ids() {
        let _ = ActivityFacilities::from(IOFacilities {
            name: None,
            aggregation_layer: None,
            lang: None,
            attributes: None,
            facilities: vec![
                io_facility_with_id_coord_and_link("f1"),
                io_facility_with_id_coord_and_link("f1"),
            ],
        });
    }

    #[integration_test]
    fn conversion_creates_activity_type_and_link_ids() {
        let facilities = ActivityFacilities::from(IOFacilities {
            name: None,
            aggregation_layer: None,
            lang: None,
            attributes: None,
            facilities: vec![IOFacility {
                id: "f1".to_string(),
                x: Some(1.0),
                y: Some(2.0),
                z: Some(5.0),
                link_id: Some("l1".to_string()),
                desc: None,
                activities: vec![IOFacilityActivity {
                    activity_type: "shop".to_string(),
                    capacity: None,
                    open_times: vec![IOOpenTime {
                        day: IOOpenDay::Wk,
                        start_time: "00:00:00".to_string(),
                        end_time: "24:00:00".to_string(),
                    }],
                }],
                attributes: None,
            }],
        });

        let facility = facilities.get(&Id::get_from_ext("f1")).unwrap();

        assert_eq!(Coordinate::new_3d(1.0, 2.0, 5.0), facility.coord);
        assert_eq!(Id::<Link>::get_from_ext("l1"), facility.link_id);
        assert_eq!(
            Id::<String>::get_from_ext("shop"),
            facility.activities[0].activity_type
        );
        assert!(facility.mode_to_link.is_empty());
    }

    #[integration_test]
    #[should_panic(expected = "Facility with id f1 must have x and y coordinates.")]
    fn conversion_panics_without_coord() {
        let _ = ActivityFacilities::from(IOFacilities {
            name: None,
            aggregation_layer: None,
            lang: None,
            attributes: None,
            facilities: vec![io_facility_with_id_and_link("f1")],
        });
    }

    #[integration_test]
    #[should_panic(expected = "Facility with id f1 must have a link id.")]
    fn conversion_panics_without_link_id() {
        let _ = ActivityFacilities::from(IOFacilities {
            name: None,
            aggregation_layer: None,
            lang: None,
            attributes: None,
            facilities: vec![io_facility_with_id_and_coord("f1")],
        });
    }

    #[integration_test]
    fn activity_facility_modal_link_uses_mode_mapping() {
        let car = Id::create("car");
        let base_link = Id::create("base-link");
        let car_link = Id::create("car-link");
        let mut mode_to_link = IntMap::default();
        mode_to_link.insert(car.clone(), car_link.clone());

        let facility = ActivityFacility {
            id: Id::create("f1"),
            coord: Coordinate::new_2d(1.0, 2.0),
            link_id: base_link.clone(),
            mode_to_link,
            desc: None,
            activities: vec![ActivityOption {
                activity_type: Id::create("work"),
                capacity: None,
                open_times: Vec::new(),
            }],
            attributes: InternalAttributes::default(),
        };
        let facility = Facility::ActivityFacility(facility);

        assert_eq!(Some(&car_link), facility.modal_link(&car));
        assert_eq!(None, facility.modal_link(&Id::create("bike")));
        assert_eq!(&base_link, facility.link());
    }

    #[integration_test]
    fn link_wrapper_facility_provides_coord_link_and_modal_link() {
        let walk = Id::create("walk");
        let base_link = Id::create("base-link");
        let walk_link = Id::create("walk-link");
        let mut mode_to_link = IntMap::default();
        mode_to_link.insert(walk, walk_link.clone());

        let facility = Facility::LinkWrapperFacility(LinkWrapperFacility {
            coord: Coordinate::new_2d(3.0, 4.0),
            link_id: base_link.clone(),
            mode_to_link,
        });

        assert_eq!(&Coordinate::new_2d(3.0, 4.0), facility.coord());
        assert_eq!(&base_link, facility.link());
        assert_eq!(Some(&walk_link), facility.modal_link(&Id::create("walk")));
        assert_eq!(None, facility.modal_link(&Id::create("car")));
    }

    #[integration_test]
    #[should_panic(expected = "Transit facility with id stop-1 has no link id.")]
    fn transit_facility_link_panics_without_link_ref_id() {
        let facility = Facility::TransitFacility(TransitStopFacility {
            id: Id::create("stop-1"),
            coord: Coordinate::new_2d(1.0, 2.0),
            link_ref_id: None,
            name: None,
            stop_area_id: None,
            is_blocking: None,
            attributes: InternalAttributes::default(),
        });

        facility.link();
    }

    fn io_facility_with_id_and_link(id: &str) -> IOFacility {
        IOFacility {
            id: id.to_string(),
            x: None,
            y: None,
            z: None,
            link_id: Some("l1".to_string()),
            desc: None,
            activities: Vec::new(),
            attributes: None,
        }
    }

    fn io_facility_with_id_and_coord(id: &str) -> IOFacility {
        IOFacility {
            id: id.to_string(),
            x: Some(1.0),
            y: Some(2.0),
            z: None,
            link_id: None,
            desc: None,
            activities: Vec::new(),
            attributes: None,
        }
    }

    fn io_facility_with_id_coord_and_link(id: &str) -> IOFacility {
        IOFacility {
            id: id.to_string(),
            x: Some(1.0),
            y: Some(2.0),
            z: None,
            link_id: Some("l1".to_string()),
            desc: None,
            activities: Vec::new(),
            attributes: None,
        }
    }
}
