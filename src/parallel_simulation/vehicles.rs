use std::collections::HashMap;

pub struct VehiclesIdMapping<'id> {
    next_id: usize,
    matsim_id_2_id: HashMap<&'id str, usize>,
}

impl<'id> VehiclesIdMapping<'id> {
    pub fn new() -> VehiclesIdMapping<'id> {
        VehiclesIdMapping {
            matsim_id_2_id: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn map_vehicle_id(&mut self, matsim_id: &'id str) -> usize {
        let id = self.matsim_id_2_id.entry(matsim_id).or_insert(self.next_id);

        if self.next_id == *id {
            self.next_id += 1;
        }

        *id
    }

    pub fn get_from_matsim_id(&self, matsim_id: &str) -> usize {
        * self.matsim_id_2_id.get(matsim_id).unwrap()
    }
}
