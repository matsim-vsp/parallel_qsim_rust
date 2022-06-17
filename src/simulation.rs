use crate::simulation::activity_q::ActivityQ;
use crate::simulation::q_population::{SimPlanElement, SimRoute};
use crate::simulation::q_scenario::QScenario;
use crate::simulation::q_vehicle::QVehicle;

mod activity_q;
mod q_network;
mod q_population;
mod q_scenario;
mod q_vehicle;

struct Simulation<'a> {
    scenario: QScenario<'a>,
    activity_q: ActivityQ,
}

impl<'a> Simulation<'a> {
    fn new(scenario: QScenario<'a>) -> Simulation<'a> {
        Simulation {
            scenario,
            activity_q: ActivityQ::new(),
        }
    }

    fn run(&mut self) {
        // prepare the simulation:
        // put all people into the activity q, assuming that everybody has activity as first plan element
        println!("Put each agent into the AcivityQ");
        for agent in &self.scenario.population.agents {
            self.activity_q.add(agent, 0);
        }

        // calculate the start time
        let mut now = self.activity_q.next_wakeup();
        let end_time = 86400;
        println!(
            "\n #### Start the simulation at timestep {}. Last timestep is set to {} ####\n",
            now, end_time
        );

        while self.active_agents() > 0 && now <= end_time {
            self.wakeup(now);
            self.move_nodes(now);
            now += 1;
        }

        println!("\n #### Finished simulation. Last timestep was: #{now}. #### \n")
    }

    fn wakeup(&mut self, now: u32) {
        let agents_2_link = self.activity_q.wakeup(now);

        if agents_2_link.len() > 0 {
            println!(
                "{} agents woke up. Creating vehicles and putting them onto links",
                agents_2_link.len()
            );
        }

        for id in agents_2_link {
            let agent = self.scenario.population.agents.get_mut(id).unwrap();
            agent.advance_plan();
            if let SimPlanElement::Leg(leg) = agent.current_plan_element() {
                if let SimRoute::NetworkRoute(route) = &leg.route {
                    // create a vehicle which has a reference to its driver
                    let vehicle = QVehicle::new(route.vehicle_id, agent.id, route.route.clone());
                    let link_id = route.route.get(0).unwrap();
                    let link = self.scenario.network.links.get_mut(*link_id).unwrap();
                    // vehicles are put into the back of the queue, regardless.
                    link.push_vehicle(vehicle)
                }
            }
        }
    }

    fn move_nodes(&mut self, now: u32) {
        for node in &self.scenario.network.nodes {
            // move vehicles over nodes and collect the agents which are at the end of their route
            let vehicles_at_end_of_route =
                node.move_vehicles(&mut self.scenario.network.links, now);

            for vehicle in vehicles_at_end_of_route {
                let agent = self
                    .scenario
                    .population
                    .agents
                    .get_mut(vehicle.driver_id)
                    .unwrap();

                println!("Vehicle #{} has arrived at activity.", vehicle.id);
                agent.advance_plan();
                self.activity_q.add(agent, now);
            }
        }
    }

    fn active_agents(&self) -> usize {
        self.scenario.population.agents.len() - self.activity_q.finished_agents()
    }
}

#[cfg(test)]
mod tests {
    use crate::container::network::Network;
    use crate::container::population::Population;
    use crate::simulation::q_scenario::QScenario;
    use crate::simulation::Simulation;

    #[test]
    fn run_equil_scenario() {
        let network = Network::from_file("./assets/equil-network.xml");
        let population = Population::from_file("./assets/equil_output_plans.xml.gz");
        let scenario = QScenario::from_container(&network, &population);
        let mut simulation = Simulation::new(scenario);

        println!("Finished reading network and population. Call Simulation::run");
        simulation.run();
        println!("Finished simulation.")
    }
}
