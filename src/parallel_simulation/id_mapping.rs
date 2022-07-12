use std::collections::HashMap;
use crate::container::matsim_id::MatsimId;

pub struct MatsimIdMappings {
    pub links: MatsimIdMapping,
    pub nodes: MatsimIdMapping,
    pub vehicles: MatsimIdMapping,
    pub agents: MatsimIdMapping
}

#[derive(Debug)]
pub struct MatsimIdMapping {
    // both maps have owned strings. This could be one owned string and the other could have a
    // ref to it. According to https://stackoverflow.com/questions/72941761/for-loop-struct-with-lifetime-a-cannot-borrow-as-mutable-because-it-is-also-b/72942371#72942371
    // this would require unsafe code.
    matsim_2_internal: HashMap<String, usize>,
    internal_2_matsim: HashMap<usize, String>,
}

impl MatsimIdMapping {
    fn new() -> MatsimIdMapping {
        MatsimIdMapping {
            matsim_2_internal: HashMap::new(),
            internal_2_matsim: HashMap::new(),
        }
    }

    /// I still don't understand how to override the from trait, so have this as separate method name here
    pub fn from_matsim_ids<T>(items: &Vec<T>) -> MatsimIdMapping where T: MatsimId {
        let mut mapping = MatsimIdMapping::new();

        for (i, item) in items.iter().enumerate() {
            mapping.insert(i, String::from(item.id()));
        }

        mapping
    }

    fn insert(&mut self, internal: usize, matsim: String) {
        self.internal_2_matsim.insert(internal, matsim);
        let mapped_matsim = self.internal_2_matsim.get(&internal).unwrap();
        self.matsim_2_internal.insert(mapped_matsim.clone(), internal);
    }

    pub fn get_internal(&self, external: &str) -> Option<&usize> {
        self.matsim_2_internal.get(external)
    }

    pub fn get_external(&self, internal: &usize) -> Option<&String> {
        self.internal_2_matsim.get(internal)
    }
}

#[derive(Debug)]
pub struct IdMapping {
    pub id_2_thread: HashMap<usize, usize>,
    pub matsim_id_2_id: HashMap<String, usize>,
}

impl IdMapping {
    pub fn new() -> IdMapping {
        IdMapping {
            id_2_thread: HashMap::new(),
            matsim_id_2_id: HashMap::new(),
        }
    }

    pub fn insert(&mut self, id: usize, thread: usize, matsim_id: String) {
        self.id_2_thread.insert(id, thread);
        self.matsim_id_2_id.insert(matsim_id, id);
    }

    pub fn get_from_matsim_id(&self, matsim_id: &str) -> usize {
        *self.matsim_id_2_id.get(matsim_id).unwrap()
    }

    pub fn get_thread(&self, id: &usize) -> usize {
        *self.id_2_thread.get(id).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::container::network::IONetwork;
    use crate::parallel_simulation::id_mapping::MatsimIdMapping;

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
