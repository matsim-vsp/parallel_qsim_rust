#[derive(Debug)]
struct QRoute {
    links: Vec<usize>,
}

#[derive(Debug)]
pub struct QVehicle {
    id: usize,
    route: QRoute,
    current_link: usize,
    exit_time: u32,
}
