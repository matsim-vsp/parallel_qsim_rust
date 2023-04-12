use crate::simulation::network::node::NodeVehicle;

#[derive(Debug)]
pub struct Vehicle {
    pub id: usize,
    // instead of having a reference to the driver agent, we keep a reference to the network route
    // of its current leg, as well as its id. This makes borrowing easier.
    pub route: Vec<usize>,
    pub driver_id: usize,
    //pub exit_time: u32,
    pub route_index: usize,
    pub mode: String,
}

impl Vehicle {
    pub fn new(id: usize, driver_id: usize, route: Vec<usize>, mode: String) -> Vehicle {
        Vehicle {
            id,
            driver_id,
            //exit_time: 0,
            route_index: 0,
            route,
            mode,
        }
    }
}

impl NodeVehicle for Vehicle {
    fn id(&self) -> usize {
        self.id
    }

    fn advance_route_index(&mut self) {
        self.route_index += 1;
    }

    fn curr_link_id(&self) -> Option<usize> {
        self.route.get(self.route_index).map(|id| *id)
    }

    fn is_current_link_last(&self) -> bool {
        self.route_index + 1 >= self.route.len()
    }

    fn mode(&self) -> &str {
        self.mode.as_str()
    }
}
