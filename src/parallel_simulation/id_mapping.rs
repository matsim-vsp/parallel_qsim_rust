use std::collections::HashMap;

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
