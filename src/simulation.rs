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

pub fn run(network: &Network, population: &Population) {
    // transform data into simulation structure
    // 1. network
    println!("Convert Network container into QNetwork.");
    let mut q_network = QNetwork::from_container(&network);
    // 2. population
    println!("Convert Population container into QPopulation");
    let mut q_population = QPopulation::from_container(&population, &q_network);

    // prepare the simulation:
    // put all people into the activity q, assuming that everybody has activity as first plan element
    println!("Put each agent into the AcivityQ");
    let mut activity_q = ActivityQ::new();
    for agent in &q_population.agents {
        activity_q.add(agent, 0);
    }

    // calculate the start time
    let start_time = activity_q.next_wakeup();
    let end_time = start_time + 100;
    println!(
        "Start the simulation at timestep {}. Last timestep is set to {}",
        start_time, end_time
    );

    // start simulation loop
    for now in start_time..end_time {
        println!("\n ####### Timestep {} ########\n", now);
        // for each timestep it should check whether anybody should be put onto the network
        // this is part of the move link code in the original qsim.
        let agents_2_link = activity_q.wakeup(now);

        if agents_2_link.len() > 0 {
            println!(
                "{} agents woke up. Creating vehicles and putting them onto links",
                agents_2_link.len()
            );
        }

        for id in agents_2_link {
            let agent = q_population.agents.get_mut(id).unwrap();
            agent.advance_plan();
            if let SimPlanElement::Leg(leg) = agent.current_plan_element() {
                if let SimRoute::NetworkRoute(route) = &leg.route {
                    // create a vehicle. use the agent's id as vehicle id for now. This should be changed to something else
                    // also copy the route here because the borrow checker would complain otherwise.
                    let vehicle = QVehicle::new(agent.id, route.route.clone());
                    let link_id = route.route.get(0).unwrap();
                    let link = q_network.links.get_mut(*link_id).unwrap();
                    // vehicles are put into the back of the queue, regardless.
                    link.push_vehicle(vehicle)
                }
            }
        }
        // for each timestep it should iterate each node and execute move_node on it.
        println!("Starting move node step.");
        for node in &q_network.nodes {
            // move vehicles over nodes and collect the agents which are at the end of their route
            let vehicles_at_end_of_route = node.move_vehicles(&mut q_network.links, now);

            // those agents which are done need to be put into the activity queue
            for vehicle in vehicles_at_end_of_route {
                let agent = q_population
                    .agents
                    .iter_mut()
                    .find(|a| a.id == vehicle.id)
                    .unwrap();

                agent.advance_plan();

                activity_q.add(agent, now);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::container::network::Network;
    use crate::container::population::Population;
    use crate::simulation::run;

    #[test]
    fn run_equil_scenario() {
        let network = Network::from_file("./assets/equil-network.xml");
        let population = Population::from_file("./assets/equil_output_plans.xml.gz");

        println!("Finished reading network and population. Call Simulation::run");
        run(&network, &population);

        println!("Finished simulation.")
    }
}
