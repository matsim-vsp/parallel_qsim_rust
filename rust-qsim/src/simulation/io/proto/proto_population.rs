use crate::generated;
use crate::generated::population::leg::Route;
use crate::generated::population::{
    Activity, GenericRoute, Header, Leg, NetworkRoute, Person, Plan, PtRoute, PtRouteDescription,
};
use crate::generated::MessageIter;
use crate::simulation::id::Id;
use crate::simulation::population::{
    InternalActivity, InternalGenericRoute, InternalLeg, InternalNetworkRoute, InternalPerson,
    InternalPlan, InternalPtRoute, InternalPtRouteDescription, InternalRoute, Population,
};
use prost::Message;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use tracing::info;

pub fn load_from_proto<F>(path: &Path, filter: F) -> Population
where
    F: Fn(&InternalPerson) -> bool,
{
    info!("Loading population from file at: {path:?}");
    let file = File::open(path).unwrap_or_else(|_| panic!("Could not open File at {path:?}"));
    let mut reader = BufReader::new(file);

    if let Some(header_delim) = generated::read_delimiter(&mut reader) {
        let mut buffer = vec![0; header_delim];
        reader
            .read_exact(&mut buffer)
            .expect("Failed to read delimited buffer.");
        let header = Header::decode(buffer.as_slice()).expect("oh nono");
        info!("Header Info: {header:?}");
    }

    let mut persons = HashMap::new();

    for person in MessageIter::<Person, BufReader<File>>::new(reader) {
        let id = Id::get_from_ext(&person.id);
        let internal_person = InternalPerson::from(person);

        if filter(&internal_person) {
            persons.insert(id, internal_person);
        }
    }

    info!("Finished loading population");

    Population { persons }
}

pub fn write_to_proto(population: &Population, path: &Path) {
    info!("Converting Population into wire format");

    let prefix = path.parent().unwrap();
    fs::create_dir_all(prefix).unwrap();
    let file = File::create(path).unwrap_or_else(|_| panic!("Failed to create file at: {path:?}"));
    let mut writer = BufWriter::new(file);
    //write header
    let header = Header {
        version: 1,
        size: population.persons.len() as u32,
    };
    let mut bytes = Vec::new();
    header
        .encode_length_delimited(&mut bytes)
        .expect("TODO: panic message");
    writer.write_all(&bytes).expect("Failed to write");

    for person in population.persons.values() {
        bytes.clear();
        Person::from(person)
            .encode_length_delimited(&mut bytes)
            .expect("Failed to encode person");
        writer.write_all(&bytes).expect("failed to write buffer");
    }

    writer.flush().expect("Failed to flush buffer");
}

impl Person {
    pub fn from(value: &InternalPerson) -> Self {
        Self {
            id: value.id().external().to_string(),
            plan: value.plans().iter().map(Plan::from).collect(),
            attributes: Default::default(),
        }
    }
}

impl Plan {
    fn from(value: &InternalPlan) -> Self {
        Self {
            selected: value.selected,
            legs: value.legs().iter().map(|p| Leg::from(p)).collect(),
            acts: value.acts().iter().map(|l| Activity::from(l)).collect(),
        }
    }
}

impl Activity {
    fn from(value: &InternalActivity) -> Self {
        Self {
            act_type: value.act_type.external().to_string(),
            link_id: value.link_id.external().to_string(),
            x: value.x,
            y: value.y,
            start_time: value.start_time,
            end_time: value.end_time,
            max_dur: value.max_dur,
        }
    }
}

impl Leg {
    fn from(value: &InternalLeg) -> Self {
        Self {
            mode: value.mode.external().to_string(),
            routing_mode: value
                .routing_mode
                .as_ref()
                .map(|r| r.external().to_string()),
            dep_time: value.dep_time,
            trav_time: value.trav_time,
            attributes: value.attributes.as_cloned_map(),
            route: value.route.as_ref().map(Route::from),
        }
    }
}

impl Route {
    fn from(value: &InternalRoute) -> Self {
        match value {
            InternalRoute::Generic(g) => Route::GenericRoute(GenericRoute::from(g)),
            InternalRoute::Network(n) => Route::NetworkRoute(NetworkRoute::from(n)),
            InternalRoute::Pt(p) => Route::PtRoute(PtRoute::from(p)),
        }
    }
}

impl GenericRoute {
    fn from(value: &InternalGenericRoute) -> Self {
        Self {
            start_link: value.start_link().external().to_string(),
            end_link: value.end_link().external().to_string(),
            trav_time: value.trav_time(),
            distance: value.distance(),
            veh_id: value.vehicle().as_ref().map(|v| v.external().to_string()),
        }
    }
}

impl NetworkRoute {
    fn from(value: &InternalNetworkRoute) -> Self {
        Self {
            delegate: Some(GenericRoute::from(value.generic_delegate())),
            route: value
                .route()
                .iter()
                .map(|id| id.external().to_string())
                .collect(),
        }
    }
}

impl PtRoute {
    fn from(value: &InternalPtRoute) -> Self {
        Self {
            delegate: Some(GenericRoute::from(value.generic_delegate())),
            information: Some(PtRouteDescription::from(value.description())),
        }
    }
}

impl PtRouteDescription {
    fn from(value: &InternalPtRouteDescription) -> Self {
        Self {
            transit_route_id: value.transit_route_id.clone(),
            boarding_time: value.boarding_time,
            transit_line_id: value.transit_line_id.clone(),
            access_facility_id: value.access_facility_id.clone(),
            egress_facility_id: value.egress_facility_id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::id::Id;
    use crate::simulation::network::Network;
    use crate::simulation::population::InternalPerson;
    use crate::simulation::population::Population;
    use crate::simulation::vehicles::garage::Garage;
    use macros::integration_test;
    use std::path::PathBuf;

    #[integration_test]
    fn test_proto() {
        let _net = Network::from_file_as_is(&PathBuf::from("./assets/equil/equil-network.xml"));
        let mut garage = Garage::from_file(&PathBuf::from("./assets/equil/equil-vehicles.xml"));
        let pop = Population::from_file(
            &PathBuf::from("./assets/equil/equil-plans.xml.gz"),
            &mut garage,
        );

        let file_path =
            PathBuf::from("./test_output/simulation/population/io/test_proto/plans.binpb");
        pop.to_file(&file_path);

        let proto_pop = Population::from_file(&file_path, &mut garage);

        for (id, person) in pop.persons {
            assert!(proto_pop.persons.contains_key(&id));
            let proto_person = proto_pop.persons.get(&id).unwrap();
            assert_eq!(person.id(), proto_person.id());
        }
    }

    #[integration_test]
    fn test_filtered_proto() {
        let _net = Network::from_file_as_is(&PathBuf::from("./assets/equil/equil-network.xml"));
        let mut garage = Garage::from_file(&PathBuf::from("./assets/equil/equil-vehicles.xml"));
        let pop = Population::from_file(
            &PathBuf::from("./assets/equil/equil-plans.xml.gz"),
            &mut garage,
        );

        let file_path =
            PathBuf::from("./test_output/simulation/population/io/test_filtered_proto/plans.binpb");
        pop.to_file(&file_path);

        let proto_pop =
            Population::from_file_filtered(&file_path, &mut garage, |p| p.id().external() == "1");

        let expected_id: Id<InternalPerson> = Id::get_from_ext("1");
        assert_eq!(1, proto_pop.persons.len());
        assert!(proto_pop.persons.contains_key(&expected_id));
    }
}
