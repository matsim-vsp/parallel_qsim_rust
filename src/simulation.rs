use crate::container::network::Network;
use crate::container::population::Population;
use crate::simulation::activity_q::ActivityQ;

use crate::simulation::q_population::{SimPlanElement, SimRoute};
use crate::simulation::q_scenario::QScenario;
use crate::simulation::q_vehicle::QVehicle;

mod activity_q;
mod q_network;
mod q_population;
mod q_scenario;
mod q_vehicle;

pub fn run(network_container: &Network, population_container: &Population) {
    let mut scenario = QScenario::from_container(network_container, population_container);

    // prepare the simulation:
    // put all people into the activity q, assuming that everybody has activity as first plan element
    println!("Put each agent into the AcivityQ");
    let mut activity_q = ActivityQ::new();
    for agent in &scenario.population.agents {
        activity_q.add(agent, 0);
    }

    // calculate the start time
    let start_time = activity_q.next_wakeup();
    let end_time = start_time + 1000;
    println!(
        "Start the simulation at timestep {}. Last timestep is set to {}",
        start_time, end_time
    );

    // start simulation loop
    for now in start_time..end_time {
        wakeup(&mut scenario, &mut activity_q, now);
        move_nodes(&mut scenario, &mut activity_q, now);
    }
}

fn wakeup(scenario: &mut QScenario, activity_q: &mut ActivityQ, now: u32) {
    let agents_2_link = activity_q.wakeup(now);

    if agents_2_link.len() > 0 {
        println!(
            "{} agents woke up. Creating vehicles and putting them onto links",
            agents_2_link.len()
        );
    }

    for id in agents_2_link {
        let agent = scenario.population.agents.get_mut(id).unwrap();
        agent.advance_plan();
        if let SimPlanElement::Leg(leg) = agent.current_plan_element() {
            if let SimRoute::NetworkRoute(route) = &leg.route {
                // create a vehicle which has a reference to its driver
                let vehicle = QVehicle::new(route.vehicle_id, agent.id, route.route.clone());
                let link_id = route.route.get(0).unwrap();
                let link = scenario.network.links.get_mut(*link_id).unwrap();
                // vehicles are put into the back of the queue, regardless.
                link.push_vehicle(vehicle)
            }
        }
    }
}

fn move_nodes(scenario: &mut QScenario, activity_q: &mut ActivityQ, now: u32) {
    for node in &scenario.network.nodes {
        // move vehicles over nodes and collect the agents which are at the end of their route
        let vehicles_at_end_of_route = node.move_vehicles(&mut scenario.network.links, now);

        // those agents which are done need to be put into the activity queue
        for vehicle in vehicles_at_end_of_route {
            let agent = scenario
                .population
                .agents
                .get_mut(vehicle.driver_id)
                .unwrap();

            println!("Vehicle #{} has arrived at activity.", vehicle.id);
            agent.advance_plan();
            activity_q.add(agent, now);
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
