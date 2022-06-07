use std::collections::VecDeque;
use std::slice::Iter;

type Q = VecDeque<Vehicle>;

struct Vehicle {
    id: u32,
    route: NetworkRoute,
    current_link: usize
}

struct NetworkRoute {
    link_ids: Vec<usize>,
}

struct Link {
    id: u32,
    q: Q,
}

struct Node {
    id: i32,
    in_links: Vec<usize>,
    out_links: Vec<usize>,
}

impl Node {
    fn move_vehicles(&self, links: &mut Vec<Link>) {

        println!("Move_vehicles on node #{}", self.id);
        for in_link_index in &self.in_links {
            let link = links.get_mut(*in_link_index).unwrap();

            match link.q.pop_front() {
                None => { println!("Link #{} has no vehicles in the Q", link.id) }
                Some(mut vehicle) => {

                    // increase the pointer into the route's link list by one.
                    let next_route_element = vehicle.current_link + 1;

                    // fetch the next link id of the vehicle's route. If None, the vehicle's trip is over
                    match vehicle.route.link_ids.get(next_route_element) {
                        None => {println!("Vehicle #{} has reached its destination. It will dissapear now. ", vehicle.id)}
                        Some(out_index) => {

                            // fetch the next link of the route.
                            let out_link = links.get_mut(*out_index).unwrap();

                            // make the transition by increasing the vehicle's pointer into the route array
                            // and by pushing the vehicle onto the next link.
                            println!("Pushing vehicle #{} to link #{}", vehicle.id, out_link.id);
                            vehicle.current_link = next_route_element;
                            out_link.q.push_back(vehicle);
                        }
                    }
                }
            }
        }
    }
}

pub fn run() {

    // have a network in the form of below.
    //        0
    //      /  \
    // 0---0    0----0
    //      \  /
    //       0
    //
    // also, have one vehicle travel the upper one the lower route.

    let vehicle1 = Vehicle { id: 1, route: NetworkRoute { link_ids: Vec::from([0, 1, 2, 5]) }, current_link: 0 };
    let vehicle2 = Vehicle { id: 2, route: NetworkRoute { link_ids: Vec::from([0, 3, 4, 5]) }, current_link: 0 };

    let link1 = Link { q: VecDeque::from([vehicle1, vehicle2]), id: 1};
    let link2 = Link { q: VecDeque::new(), id: 2};
    let link3 = Link { q: VecDeque::new(), id: 3};
    let link4 = Link { q: VecDeque::new(), id: 4};
    let link5 = Link { q: VecDeque::new(), id: 5};
    let link6 = Link { q: VecDeque::new(), id: 6};
    let mut links = vec![link1, link2, link3, link4, link5, link6];

    let node1 = Node { id: 1, in_links: Vec::new(), out_links: Vec::from([0])};
    let node2 = Node { id: 2, in_links: Vec::from([0]), out_links: Vec::from([1, 3])};
    let node3 = Node { id: 3, in_links: Vec::from([1]), out_links: Vec::from([2])};
    let node4 = Node { id: 4, in_links: Vec::from([3]), out_links: Vec::from([4])};
    let node5 = Node { id: 5, in_links: Vec::from([2, 4]), out_links: Vec::from([5])};
    let node6 = Node { id: 5, in_links: Vec::from([5]), out_links: Vec::new()};
    let nodes = vec![node1, node2, node3, node4, node5, node6];

    for i in 0..10 {
        println!("\nstep #{}", i);
        move_nodes(nodes.iter(), &mut links);
    }
}

fn move_nodes(nodes: Iter<Node>, links: &mut Vec<Link>) {
    println!("move nodes:");
    for node in nodes {
        node.move_vehicles(links);
    }
}

#[cfg(test)]
mod tests {
    use crate::link_automata_with_routes::run;

    #[test]
    fn test_run() {
        run();
    }

}

