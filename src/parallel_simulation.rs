use crate::parallel_simulation::activity_q::ActivityQ;
use crate::parallel_simulation::customs::Customs;
use crate::parallel_simulation::messages::Message;
use crate::parallel_simulation::splittable_network::Network;
use crate::parallel_simulation::splittable_population::{PlanElement, Population, Route};
use crate::parallel_simulation::splittable_scenario::Scenario;
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
    network: Network,
    customs: Customs,
    activity_q: ActivityQ,
}

impl Simulation {
    fn new(network: Network, population: Population, customs: Customs) -> Simulation {
        let mut q = ActivityQ::new();
        for (id, agent) in population.agents.into_iter() {
            q.add(agent, 0);
        }

        Simulation {
            network,
            customs,
            activity_q: q,
        }
    }

    fn create_runners(scenarios: Vec<Scenario>) -> Vec<Simulation> {
        let mut simulations: Vec<Simulation> = Vec::new();
        let mut senders: Vec<Sender<Message>> = Vec::new();

        // create simulations and store the receiver part of a channel in it
        for scenario in scenarios.into_iter() {
            let (sender, receiver) = mpsc::channel();
            let customs = Customs::new(receiver);
            let simulation = Simulation::new(scenario.network, scenario.population, customs);
            simulations.push(simulation);
            senders.push(sender);
        }

        // now, copy a sender for each simulation, so that each simulation can
        // send to every other simulation
        for (i_sim, simulation) in simulations.iter_mut().enumerate() {
            for (i_sender, sender) in senders.iter().enumerate() {
                if i_sim != i_sender {
                    simulation.customs.add_sender(i_sender, sender.clone());
                }
            }
        }

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
            self.receive(now);
            self.send(now);
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

        for mut agent in agents_2_link.into_iter() {
            agent.advance_plan();

            if let PlanElement::Leg(leg) = agent.current_plan_element() {
                if let Route::NetworkRoute(ref route) = leg.route {
                    let vehicle = Vehicle::new(route.vehicle_id, agent);
                    let link_id = route.route.get(0).unwrap();
                    let link = self.network.links.get_mut(link_id).unwrap();
                    // vehicles are put into the back of the queue, regardless.
                    link.push_vehicle(vehicle);
                }
            }
        }
    }

    fn move_nodes(&mut self, now: u32) {
        for node in self.network.nodes.values() {
            let veh_not_continuing = node.move_vehicles(&mut self.network.links, now);

            // here the agents at the end of their route must be put into the agent q
            // vehicles which cross a thread boundary must be put into customs
            todo!()
        }
    }

    fn receive(&mut self, now: u32) {
        let messages = self.customs.receive();
    }

    fn send(&mut self, now: u32) {
        self.customs.send();
    }

    fn active_agents(&self) -> usize {
        todo!() // this needs something else maybe a counter would do.
                //self.scenario.population.agents.len() - self.activity_q.finished_agents()
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
        let scenarios = Scenario::from_io(&network, &population);
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
