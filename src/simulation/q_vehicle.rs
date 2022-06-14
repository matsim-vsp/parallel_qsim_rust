use crate::simulation::q_population::NetworkRoute;

#[derive(Debug)]
pub struct QVehicle<'a> {
    pub id: usize,
    pub route: &'a NetworkRoute,
    pub current_route_index: usize,
    pub exit_time: u32,
}

impl<'a> QVehicle<'a> {
    pub fn new(id: usize, route: &'a NetworkRoute) -> QVehicle {
        QVehicle {
            id, route, current_route_index: 0, exit_time: 0
        }  
    }
    
    pub fn advance_route_index(&mut self) {
        self.current_route_index += 1;
    }
    
    pub fn current_link_id(&self) -> Option<&usize> {
        self.route.route.get(self.current_route_index)
    }
}
