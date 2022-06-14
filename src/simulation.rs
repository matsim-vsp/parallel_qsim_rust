use crate::container::network::Network;
use crate::container::population::Population;
use crate::simulation::activity_q::ActivityQ;
use crate::simulation::q_network::QNetwork;
use crate::simulation::q_population::{QPopulation, SimPlanElement, SimRoute};
use crate::simulation::q_vehicle::QVehicle;

mod activity_q;
mod q_network;
mod q_population;
mod q_vehicle;

pub fn run() {
    // read in some input data
    // 1. input network
    let network = Network::from_file("./assets/equil-network.xml");
    // 2. input population
    let population = Population::from_file("./assets/population-v6-34-persons.xml"); // this will not work. Population and network don't go together. maybe use output population from equi scenario

    // transform data into simulation structure
    // 1. network
    let mut q_network = QNetwork::from_container(&network);
    // 2. population
    let mut q_population = QPopulation::from_container(&population, &q_network);

    // prepare the simulation:
    // put all people into the activity q, assuming that everybody has activity as first plan element
    let mut activity_q = ActivityQ::new();
    for agent in &q_population.agents {
        activity_q.add(agent, 0);
    }
    let last_time_step = activity_q.next_wakeup_time + 100;

    // start simulation loop
    for now in activity_q.next_wakeup_time..last_time_step {
        // for each timestep it should check whether anybody should be put onto the network
        // this is part of the move link code in the original qsim.
        let agents_2_link = activity_q.wakeup(now);
        for id in agents_2_link {
            let agent = q_population.agents.get_mut(id).unwrap();
            agent.advance_plan();
            if let SimPlanElement::Leg(leg) = agent.current_plan_element() {
                if let SimRoute::NetworkRoute(route) = &leg.route {
                    // create a vehicle. use the agent's id as vehicle id for now. This should be changed to something else
                    let vehicle = QVehicle::new(agent.id, route);
                    let link_id = route.route.get(0).unwrap();
                    let link = q_network.links.get_mut(*link_id).unwrap();
                    // vehicles are put into the back of the queue, regardless.
                    link.push_vehicle(vehicle)
                }
            }
        }
        // for each timestep it should iterate each node and execute move_node on it.
        for node in q_network.nodes {}
    }
    //  for each timestep it should check whether anybody should be put onto the network
    //  for each timestep it should iterate each node and execute move_node on it.
    //    this could also check if a vehicle is eligible for exiting a link whether it has reached the
    //    end of its current route. If so the person inside the vehicle will be put into the q of the
    //    activity manager.
}
