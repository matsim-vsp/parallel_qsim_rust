use std::collections::HashMap;

pub struct QVehicles<'id> {
    next_id: usize,
    vehicle_id_mapping: HashMap<&'id str, usize>,
}

impl<'id> QVehicles<'id> {
    pub fn new() -> QVehicles<'id> {
        QVehicles {
            vehicle_id_mapping: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn map_vehicle_id(&mut self, matsim_id: &'id str) -> usize {
        let id = self
            .vehicle_id_mapping
            .entry(matsim_id)
            .or_insert(self.next_id);

        if self.next_id == *id {
            self.next_id += 1;
        }

        *id
    }
}

#[derive(Debug)]
pub struct QVehicle {
    pub id: usize,
    pub driver_id: usize,
    pub route: Vec<usize>,
    pub current_route_index: usize,
    pub exit_time: u32,
}

impl QVehicle {
    pub fn new(id: usize, driver_id: usize, route: Vec<usize>) -> QVehicle {
        QVehicle {
            id,
            driver_id,
            route,
            current_route_index: 0,
            exit_time: 0,
        }
    }

    pub fn advance_route_index(&mut self) {
        self.current_route_index += 1;
    }

    pub fn current_link_id(&self) -> Option<&usize> {
        self.route.get(self.current_route_index)
    }
}
