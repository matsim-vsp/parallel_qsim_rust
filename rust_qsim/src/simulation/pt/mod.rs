use crate::simulation::InternalAttributes;
use crate::simulation::id::Id;
use crate::simulation::io::xml::attributes::{IOAttribute, IOAttributes};
use crate::simulation::io::xml::transit;
use crate::simulation::io::xml::transit::{
    IODeparture, IOMinimalTransferRelation, IORouteStop, IOStopFacility, IOTransitLine,
    IOTransitRoute, IOTransitSchedule,
};
use crate::simulation::scenario::Coordinate;
use crate::simulation::scenario::network::Link;
use crate::simulation::time::SimTime;
use nohash_hasher::IntMap;
use std::path::Path;
use std::time::Duration;
use tracing::info;

#[derive(Debug, PartialEq, Clone)]
pub struct TransitLine {
    pub id: Id<TransitLine>,
    pub name: String,
    pub routes: IntMap<Id<TransitRoute>, TransitRoute>,
    pub attributes: InternalAttributes,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TransitRoute {
    pub id: Id<TransitRoute>,
    pub description: Option<String>,
    pub transport_mode: Id<String>,
    pub stops: Vec<TransitRouteStop>,
    pub network_route: Vec<Id<Link>>,
    pub departures: Vec<TransitDeparture>,
    pub attributes: InternalAttributes,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TransitRouteStop {
    pub facility_id: Id<TransitStopFacility>,
    pub arrival_offset: Option<Duration>,
    pub departure_offset: Option<Duration>,
    pub await_departure: Option<bool>,
    pub allow_boarding: bool,
    pub allow_alighting: bool,
    pub minimum_stop_duration: Duration,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TransitDeparture {
    pub id: Id<TransitDeparture>,
    pub departure_time: SimTime,
    pub vehicle_ref_id: Option<Id<String>>,
    pub attributes: InternalAttributes,
    // TODO in java there are chainedDepartures. Not sure, if we need this.
}

#[derive(Debug, PartialEq, Clone)]
pub struct MinimalTransferTime {
    pub from_stop: Id<TransitStopFacility>,
    pub to_stop: Id<TransitStopFacility>,
    pub transfer_time: f64,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TransitStopFacility {
    pub id: Id<TransitStopFacility>,
    pub coord: Coordinate,
    pub link_ref_id: Option<Id<Link>>,
    pub name: Option<String>,
    pub stop_area_id: Option<String>,
    pub is_blocking: Option<bool>,
    pub attributes: InternalAttributes,
}

#[derive(Debug, PartialEq, Clone)]
pub struct TransitSchedule {
    lines: IntMap<Id<TransitLine>, TransitLine>,
    facilities: IntMap<Id<TransitStopFacility>, TransitStopFacility>,
    minimal_transfer_times: Vec<MinimalTransferTime>,
    attributes: InternalAttributes,
}

impl TransitSchedule {
    pub fn from_file(file_path: &Path) -> Self {
        info!("Reading transit schedule");
        let schedule = TransitSchedule::from(transit::load_from_xml(file_path));
        info!(
            "Finished reading transit schedule. Found {} lines, {} routes and {} facilities.",
            schedule.lines.len(),
            schedule.num_routes(),
            schedule.facilities.len()
        );
        schedule
    }

    pub fn lines(&self) -> &IntMap<Id<TransitLine>, TransitLine> {
        &self.lines
    }

    pub fn facilities(&self) -> &IntMap<Id<TransitStopFacility>, TransitStopFacility> {
        &self.facilities
    }

    pub fn minimal_transfer_times(&self) -> &Vec<MinimalTransferTime> {
        &self.minimal_transfer_times
    }

    pub fn attributes(&self) -> &InternalAttributes {
        &self.attributes
    }

    pub fn get_line(&self, id: &Id<TransitLine>) -> &TransitLine {
        self.lines.get(id).unwrap()
    }

    pub fn get_facility(&self, id: &Id<TransitStopFacility>) -> &TransitStopFacility {
        self.facilities.get(id).unwrap()
    }

    pub fn num_routes(&self) -> usize {
        self.lines.values().map(|line| line.routes.len()).sum()
    }
}

impl From<IOTransitSchedule> for TransitSchedule {
    fn from(io: IOTransitSchedule) -> Self {
        let facilities = io
            .transit_stops
            .stop_facilities
            .into_iter()
            .map(TransitStopFacility::from)
            .map(|facility| (facility.id.clone(), facility))
            .collect();

        let lines = io
            .transit_lines
            .into_iter()
            .map(TransitLine::from)
            .map(|line| (line.id.clone(), line))
            .collect();

        let minimal_transfer_times = io
            .minimal_transfer_times
            .map(|times| {
                times
                    .relations
                    .into_iter()
                    .map(MinimalTransferTime::from)
                    .collect()
            })
            .unwrap_or_default();

        TransitSchedule {
            lines,
            facilities,
            minimal_transfer_times,
            attributes: io.attributes.map(Into::into).unwrap_or_default(),
        }
    }
}

impl From<IOTransitLine> for TransitLine {
    fn from(io: IOTransitLine) -> Self {
        Id::<String>::create(&io.id);
        let routes = io
            .transit_routes
            .into_iter()
            .map(TransitRoute::from)
            .map(|route| (route.id.clone(), route))
            .collect();

        TransitLine {
            id: Id::create(&io.id),
            name: io.name.unwrap_or_default(),
            routes,
            attributes: io.attributes.map(Into::into).unwrap_or_default(),
        }
    }
}

impl From<IOTransitRoute> for TransitRoute {
    fn from(io: IOTransitRoute) -> Self {
        Id::<String>::create(&io.id);
        TransitRoute {
            id: Id::create(&io.id),
            description: io.description,
            transport_mode: Id::create(&io.transport_mode),
            stops: io
                .route_profile
                .stops
                .into_iter()
                .map(TransitRouteStop::from)
                .collect(),
            network_route: io
                .route
                .links
                .into_iter()
                .map(|link| Id::create(&link.ref_id))
                .collect(),
            departures: io
                .departures
                .departures
                .into_iter()
                .map(TransitDeparture::from)
                .collect(),
            attributes: io.attributes.map(Into::into).unwrap_or_default(),
        }
    }
}

impl From<IORouteStop> for TransitRouteStop {
    fn from(io: IORouteStop) -> Self {
        let await_departure = io
            .await_departure
            .or_else(|| find_bool_attr(&io.attributes, "awaitDeparture"));

        TransitRouteStop {
            facility_id: Id::create(&io.ref_id),
            arrival_offset: parse_duration_opt(&io.arrival_offset),
            departure_offset: parse_duration_opt(&io.departure_offset),
            await_departure,
            allow_boarding: find_bool_attr(&io.attributes, "allowBoarding").unwrap_or(true),
            allow_alighting: find_bool_attr(&io.attributes, "allowAlighting").unwrap_or(true),
            minimum_stop_duration: find_duration_attr(&io.attributes, "minimumStopDuration")
                .unwrap_or_default(),
        }
    }
}

impl From<&TransitRouteStop> for IORouteStop {
    fn from(stop: &TransitRouteStop) -> Self {
        let mut attributes = Vec::new();
        attributes.push(IOAttribute::new_with_class(
            "allowBoarding".to_string(),
            "java.lang.Boolean".to_string(),
            stop.allow_boarding.to_string(),
        ));
        attributes.push(IOAttribute::new_with_class(
            "allowAlighting".to_string(),
            "java.lang.Boolean".to_string(),
            stop.allow_alighting.to_string(),
        ));
        if let Some(await_departure) = stop.await_departure {
            attributes.push(IOAttribute::new_with_class(
                "awaitDeparture".to_string(),
                "java.lang.Boolean".to_string(),
                await_departure.to_string(),
            ));
        }
        attributes.push(IOAttribute::new_with_class(
            "minimumStopDuration".to_string(),
            "java.lang.Integer".to_string(),
            stop.minimum_stop_duration.as_secs().to_string(),
        ));

        IORouteStop {
            ref_id: stop.facility_id.external().to_string(),
            arrival_offset: format_duration_opt(stop.arrival_offset),
            departure_offset: format_duration_opt(stop.departure_offset),
            await_departure: stop.await_departure,
            attributes: Some(IOAttributes { attributes }),
        }
    }
}

impl From<IODeparture> for TransitDeparture {
    fn from(io: IODeparture) -> Self {
        TransitDeparture {
            id: Id::create(&io.id),
            departure_time: parse_time_required(&io.departure_time, "departureTime"),
            vehicle_ref_id: io.vehicle_ref_id.map(|id| Id::create(&id)),
            attributes: io.attributes.map(Into::into).unwrap_or_default(),
        }
    }
}

impl From<IOMinimalTransferRelation> for MinimalTransferTime {
    fn from(io: IOMinimalTransferRelation) -> Self {
        MinimalTransferTime {
            from_stop: Id::create(&io.from_stop),
            to_stop: Id::create(&io.to_stop),
            transfer_time: io.transfer_time,
        }
    }
}

impl From<IOStopFacility> for TransitStopFacility {
    fn from(io: IOStopFacility) -> Self {
        Id::<String>::create(&io.id);
        TransitStopFacility {
            id: Id::create(&io.id),
            coord: Coordinate::new_3d(io.x, io.y, io.z.unwrap_or_default()),
            link_ref_id: io.link_ref_id.map(|id| Id::create(&id)),
            name: io.name,
            stop_area_id: io.stop_area_id,
            is_blocking: io.is_blocking,
            attributes: io.attributes.map(Into::into).unwrap_or_default(),
        }
    }
}

fn parse_duration_opt(value: &Option<String>) -> Option<Duration> {
    value.as_deref().and_then(parse_duration)
}

fn format_duration_opt(value: Option<Duration>) -> Option<String> {
    value.map(format_duration)
}

fn parse_time_required(value: &str, field_name: &str) -> SimTime {
    parse_time(value)
        .unwrap_or_else(|| panic!("Invalid transit time value for {field_name}: {value}"))
}

fn parse_time(value: &str) -> Option<SimTime> {
    SimTime::parse(value).ok()
}

#[allow(dead_code)]
fn format_time(value: SimTime) -> String {
    value.format_hh_mm_ss_trimmed()
}

fn parse_duration(value: &str) -> Option<Duration> {
    parse_time(value).map(SimTime::as_duration)
}

fn format_duration(value: Duration) -> String {
    SimTime::from_duration(value).format_hh_mm_ss_trimmed()
}

fn find_bool_attr(attributes: &Option<IOAttributes>, name: &str) -> Option<bool> {
    attributes
        .as_ref()
        .and_then(|attrs| attrs.find(name))
        .and_then(|value| value.parse::<bool>().ok())
}

fn find_duration_attr(attributes: &Option<IOAttributes>, name: &str) -> Option<Duration> {
    let value = attributes.as_ref()?.find(name)?;
    parse_duration(value).or_else(|| {
        value
            .parse::<u64>()
            .ok()
            .map(Duration::from_secs)
            .or_else(|| {
                value
                    .parse::<f64>()
                    .ok()
                    .map(|v| Duration::from_secs_f64(v))
            })
    })
}

#[cfg(test)]
mod tests {
    use super::{format_duration, format_time, parse_duration, parse_time};
    use crate::simulation::id::Id;
    use crate::simulation::pt::{
        TransitDeparture, TransitLine, TransitRoute, TransitSchedule, TransitStopFacility,
    };
    use crate::simulation::scenario::network::Link;
    use crate::simulation::time::SimTime;
    use macros::deterministic_id_test;
    use std::time::Duration;

    #[deterministic_id_test]
    fn tutorial_schedule_is_loaded_as_domain_model() {
        let schedule =
            TransitSchedule::from_file("./assets/pt_tutorial/transitschedule.xml".as_ref());

        assert_eq!(1, schedule.lines().len());
        assert_eq!(4, schedule.facilities().len());
        assert_eq!(2, schedule.num_routes());

        let line_id: Id<TransitLine> = Id::get_from_ext("Blue Line");
        let line = schedule.get_line(&line_id);
        assert_eq!("", line.name);
        assert_eq!(2, line.routes.len());

        let route_1to3_id: Id<TransitRoute> = Id::get_from_ext("1to3");
        let route_1to3 = line.routes.get(&route_1to3_id).unwrap();
        assert_eq!(
            Id::<String>::get_from_ext("train"),
            route_1to3.transport_mode
        );
        assert_eq!(3, route_1to3.stops.len());
        assert_eq!(4, route_1to3.network_route.len());
        assert_eq!(50, route_1to3.departures.len());
    }

    #[deterministic_id_test]
    fn dresden_schedule_is_loaded_as_domain_model() {
        let schedule = TransitSchedule::from_file(
            "./assets/dresden/dresden-v1.0-transitSchedule.xml.gz".as_ref(),
        );

        assert_eq!(376, schedule.lines().len());
        assert_eq!(9150, schedule.facilities().len());
        assert_eq!(4037, schedule.num_routes());

        let facility_id: Id<TransitStopFacility> = Id::get_from_ext("long_1");
        let facility = schedule.get_facility(&facility_id);
        assert_eq!(733340.71, facility.coord.x);
        assert_eq!(5304341.79, facility.coord.y);
        assert_eq!(0., facility.coord.z);
        assert_eq!(
            Some(Id::<Link>::get_from_ext("pt_99")),
            facility.link_ref_id.clone()
        );
        assert_eq!(Some("Rosenheim"), facility.name.as_deref());
        assert_eq!(Some("long_1"), facility.stop_area_id.as_deref());
        assert_eq!(Some(false), facility.is_blocking);
        assert_eq!(
            Some(String::from("station_S/U/RE/RB")),
            facility.attributes.get("stopFilter")
        );

        let line_id: Id<TransitLine> = Id::get_from_ext("long_EC 12---27");
        let line = schedule.get_line(&line_id);
        assert_eq!("EC 12", line.name);
        assert_eq!(
            Some(String::from("10")),
            line.attributes.get("gtfs_agency_id")
        );
        assert_eq!(
            Some(String::from("EC 12")),
            line.attributes.get("gtfs_route_short_name")
        );
        assert_eq!(
            Some(String::from("2")),
            line.attributes.get("gtfs_route_type")
        );

        let route_id: Id<TransitRoute> = Id::get_from_ext("long_EC 12---27_0");
        let route = line.routes.get(&route_id).unwrap();
        assert_eq!(Id::<String>::get_from_ext("rail"), route.transport_mode);
        assert_eq!(
            Some(String::from("rail")),
            route.attributes.get("simple_route_type")
        );
        assert_eq!(15, route.stops.len());
        assert_eq!(15, route.network_route.len());
        assert_eq!(1, route.departures.len());
        assert_eq!(
            Id::<TransitStopFacility>::get_from_ext("long_630"),
            route.stops[0].facility_id
        );
        assert_eq!(Some(Duration::from_secs(0)), route.stops[0].arrival_offset);
        assert_eq!(
            Some(Duration::from_secs(0)),
            route.stops[0].departure_offset
        );
        assert_eq!(Some(true), route.stops[0].await_departure);
        assert_eq!(Duration::from_secs(0), route.stops[0].minimum_stop_duration);
        assert_eq!(Id::<Link>::get_from_ext("pt_0"), route.network_route[0]);
        assert_eq!(
            Id::<TransitDeparture>::get_from_ext("long_1882_0"),
            route.departures[0].id
        );
        assert_eq!(
            SimTime::from_secs(20 * 3600 + 10 * 60),
            route.departures[0].departure_time
        );
        assert_eq!(
            Some(Id::<String>::get_from_ext("pt_long_EC 12---27_0_0")),
            route.departures[0].vehicle_ref_id.clone()
        );
    }

    #[test]
    fn transit_time_parsing_and_formatting_preserves_subseconds() {
        let parsed = parse_time("20:10:00.123456789").unwrap();
        assert_eq!(SimTime::from_nanos(72_600_123_456_789), parsed);
        assert_eq!("20:10:00.123456789", format_time(parsed));
    }

    #[test]
    fn transit_duration_parsing_and_formatting_preserves_subseconds() {
        let parsed = parse_duration("00:05:00.123456789").unwrap();
        assert_eq!(Duration::from_nanos(300_123_456_789), parsed);
        assert_eq!("00:05:00.123456789", format_duration(parsed));
    }
}
