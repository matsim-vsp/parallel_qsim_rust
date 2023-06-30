use crate::simulation::io::matsim_id::MatsimId;
use crate::simulation::io::network::IONetwork;
use crate::simulation::io::population::{IOPlanElement, IOPopulation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::info;

#[derive(Debug)]
pub struct MatsimIdMappings {
    pub links: Arc<MatsimIdMapping>,
    pub nodes: Arc<MatsimIdMapping>,
    pub vehicles: Arc<MatsimIdMapping>,
    pub agents: Arc<MatsimIdMapping>,
}

impl MatsimIdMappings {
    pub fn from_io(io_network: &IONetwork, io_population: &IOPopulation) -> MatsimIdMappings {
        info!("Create link id mapping.");
        let links = MatsimIdMapping::from_matsim_ids(io_network.links());
        info!("Create node id mapping.");
        let nodes = MatsimIdMapping::from_matsim_ids(io_network.nodes());
        info!("Create agent id mapping.");
        let agents = MatsimIdMapping::from_matsim_ids(&io_population.persons);
        info!("Create vehicle id mapping.");
        let vehicles_ids: Vec<_> = io_population
            .persons
            .iter()
            .map(|person| person.selected_plan())
            .flat_map(|plan| plan.elements.iter())
            .filter(|el| matches!(el, IOPlanElement::Leg(_)))
            .map(|el| match el {
                IOPlanElement::Leg(leg) => leg,
                _ => panic!(""),
            })
            .filter(|leg| leg.route.r#type == "links")
            // Filter vehicles with id null. This is a special case for modes which are routed on the
            // network but not simulated on the network. Filter those out here, and fix this otherwise
            // in matsim main. https://github.com/matsim-org/matsim-libs/issues/2098
            .filter(|leg| {
                return if let Some(vehicle) = &leg.route.vehicle {
                    !vehicle.eq("null")
                } else {
                    false
                };
            })
            .map(|leg| leg.route.vehicle.as_ref().unwrap())
            .map(|id| IdRef { id: id.as_str() })
            .collect();
        let vehicles = MatsimIdMapping::from_matsim_ids(&vehicles_ids);

        MatsimIdMappings {
            nodes: Arc::new(nodes),
            links: Arc::new(links),
            agents: Arc::new(agents),
            vehicles: Arc::new(vehicles),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MatsimIdMapping {
    // both maps have owned strings. This could be one owned string and the other could have a
    // ref to it. According to https://stackoverflow.com/questions/72941761/for-loop-struct-with-lifetime-a-cannot-borrow-as-mutable-because-it-is-also-b/72942371#72942371
    // this would require unsafe code.
    matsim_2_internal: HashMap<String, usize>,
    internal_2_matsim: HashMap<usize, String>,
}

impl MatsimIdMapping {
    pub fn new() -> MatsimIdMapping {
        MatsimIdMapping {
            matsim_2_internal: HashMap::new(),
            internal_2_matsim: HashMap::new(),
        }
    }

    /// I still don't understand how to override the from trait, so have this as separate method name here
    pub fn from_matsim_ids<T>(items: &Vec<T>) -> MatsimIdMapping
    where
        T: MatsimId,
    {
        let mut mapping = MatsimIdMapping::new();

        for (i, item) in items.iter().enumerate() {
            mapping.insert(i, String::from(item.id()));
        }

        mapping
    }

    fn insert(&mut self, internal: usize, matsim: String) {
        self.internal_2_matsim.insert(internal, matsim);
        let mapped_matsim = self.internal_2_matsim.get(&internal).unwrap();
        self.matsim_2_internal
            .insert(mapped_matsim.clone(), internal);
    }

    pub fn get_internal(&self, external: &str) -> Option<&usize> {
        self.matsim_2_internal.get(external)
    }

    pub fn get_external(&self, internal: &usize) -> Option<&String> {
        self.internal_2_matsim.get(internal)
    }
}

struct IdRef<'a> {
    id: &'a str,
}

impl MatsimId for IdRef<'_> {
    fn id(&self) -> &str {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::id_mapping::MatsimIdMapping;
    use crate::simulation::io::network::IONetwork;

    #[test]
    fn insert() {
        let id = String::from("test-id");
        let index = 1;
        let mut mapping = MatsimIdMapping::new();

        mapping.insert(index, id.clone());

        assert_eq!(&id, mapping.get_external(&index).unwrap());
        assert_eq!(&index, mapping.get_internal(&id).unwrap());
    }

    #[test]
    fn from_matsim_id() {
        let io_network = IONetwork::from_file("./assets/3-links/3-links-network.xml");
        let mapping = MatsimIdMapping::from_matsim_ids(io_network.links());

        assert_eq!(&String::from("link1"), mapping.get_external(&0).unwrap());
        assert_eq!(&2, mapping.get_internal("link3").unwrap());
    }
}
