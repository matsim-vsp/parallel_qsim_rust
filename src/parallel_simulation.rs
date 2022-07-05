use crate::parallel_simulation::activity_q::ActivityQ;
use crate::parallel_simulation::customs::Customs;
use crate::parallel_simulation::messages::Message;
use crate::parallel_simulation::splittable_network::{ExitReason, Network};
use crate::parallel_simulation::splittable_population::{
    Agent, Leg, NetworkRoute, PlanElement, Population, Route,
};
use crate::parallel_simulation::splittable_scenario::{Scenario, ScenarioSlice};
use crate::parallel_simulation::vehicles::Vehicle;

use std::sync::mpsc;
use std::sync::mpsc::Sender;

mod activity_q;
mod customs;
mod id_mapping;
mod messages;
mod splittable_network;
mod splittable_population;
mod splittable_scenario;
mod vehicles;

struct Simulation {
    scenario: ScenarioSlice,
    activity_q: ActivityQ,
}

impl Simulation {
    fn new(scenario: ScenarioSlice) -> Simulation {
        let mut q = ActivityQ::new();
        for (_, agent) in scenario.population.agents.iter() {
            q.add(agent, 0);
        }

        Simulation {
            scenario,
            activity_q: q,
        }
    }

    fn create_runners(scenario: Scenario) -> Vec<Simulation> {
        let simulations: Vec<_> = scenario
            .scenarios
            .into_iter()
            .map(|slice| Simulation::new(slice))
            .collect();

        simulations
    }

    fn run(&mut self) {
        println!("This will run some simulation loop.");

        // calculate the start time
        let mut now = self.activity_q.next_wakeup();
        let end_time = 86400;
        println!(
            "\n #### Start the simulation at timestep {}. Last timestep is set to {} ####\n",
            now, end_time
        );

        // conceptually this should do the following in the main loop:

        // put received agents onto out part of the split links
        // move nodes
        // send agents to neighbours
        // receive vehicles and teleported agents from neighbours - this must be at the end, so that the loop can start.
        //   add received agents to agent_q
        //   put agents from agent_q onto links -- wakeup

        // this will shut down the thread once everybody has left to other
        // simulation parts. Think about better sync method, so that a thread only
        // terminates, if all threads are done.
        while self.active_agents() > 0 && now <= end_time {
            self.wakeup(now);
            self.move_nodes(now);
            self.send(now);
            self.receive(now);
        }
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
            let agent = self.scenario.population.agents.get_mut(&id).unwrap();
            agent.advance_plan();
            Simulation::push_onto_network(&mut self.scenario.network, &agent, 0);
        }
    }

    fn move_nodes(&mut self, now: u32) {
        for node in self.scenario.network.nodes.values() {
            let exited_vehicles = node.move_vehicles(&mut self.scenario.network.links, now);

            for exit_reason in exited_vehicles {
                match exit_reason {
                    ExitReason::FinishRoute(vehicle) => {
                        let agent = self
                            .scenario
                            .population
                            .agents
                            .get_mut(&vehicle.driver_id)
                            .unwrap();
                        agent.advance_plan();
                        self.activity_q.add(agent, now);
                    }
                    ExitReason::ReachedBoundary(vehicle) => {
                        let agent = self
                            .scenario
                            .population
                            .agents
                            .remove(&vehicle.driver_id)
                            .unwrap();
                        self.scenario.customs.prepare_to_send(agent, vehicle);
                    }
                }
            }
        }
    }

    fn receive(&mut self, now: u32) {
        let messages = self.scenario.customs.receive();
        for message in messages {
            for vehicle in message.vehicles {
                let agent = vehicle.0;
                let route_index = vehicle.1;
                Simulation::push_onto_network(&mut self.scenario.network, &agent, route_index);
                self.scenario.population.agents.insert(agent.id, agent);
            }
        }
    }

    fn send(&mut self, now: u32) {
        self.scenario.customs.send(now);
    }

    fn active_agents(&self) -> usize {
        todo!() // this needs something else maybe a counter would do.
                //self.scenario.population.agents.len() - self.activity_q.finished_agents()
    }

    fn push_onto_network(network: &mut Network, agent: &Agent, route_index: usize) {
        if let PlanElement::Leg(leg) = agent.current_plan_element() {
            if let Route::NetworkRoute(ref route) = leg.route {
                let vehicle = Vehicle::new(route.vehicle_id, agent.id, &route.route);
                let link_id = route.route.get(route_index).unwrap();
                let link = network.links.get_mut(link_id).unwrap();
                // vehicles are put into the back of the queue, regardless.
                link.push_vehicle(vehicle);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::container::network::IONetwork;
    use crate::container::population::IOPopulation;
    use crate::parallel_simulation::splittable_scenario::Scenario;
    use crate::parallel_simulation::Simulation;
    use std::thread;
    use std::thread::JoinHandle;

    #[test]
    fn run_equil_scenario() {
        // load input files
        let network = IONetwork::from_file("./assets/equil-network.xml");
        let population = IOPopulation::from_file("./assets/equil_output_plans.xml.gz");

        // convert input into simulation
        let scenarios = Scenario::from_io(&network, &population, 2, Scenario::split);
        let simulations = Simulation::create_runners(scenarios);

        // create threads and start them
        let join_handles: Vec<JoinHandle<()>> = simulations
            .into_iter()
            .map(|mut simulation| thread::spawn(move || simulation.run()))
            .collect();

        // wait for all threads to finish
        for handle in join_handles {
            handle.join().unwrap();
        }

        println!("all simulation threads have finished. ")
    }
}
