#[derive(Debug)]
pub struct QVehicle {
    pub id: usize,
    pub route: Vec<usize>,
    pub current_route_index: usize,
    pub exit_time: u32,
}

impl QVehicle {
    pub fn new(id: usize, route: Vec<usize>) -> QVehicle {
        QVehicle {
            id,
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
