use crate::io::non_blocking_io::NonBlocking;
use crate::parallel_simulation::agent_q::AgentQ;
use crate::parallel_simulation::customs::Customs;
use crate::parallel_simulation::events::Events;
use crate::parallel_simulation::splittable_network::{ExitReason, Link, NetworkPartition};
use crate::parallel_simulation::splittable_population::Agent;
use crate::parallel_simulation::splittable_population::NetworkRoute;
use crate::parallel_simulation::splittable_population::PlanElement;
use crate::parallel_simulation::splittable_population::Route;
use crate::parallel_simulation::splittable_scenario::{Scenario, ScenarioPartition};
use crate::parallel_simulation::vehicles::Vehicle;

mod agent_q;
mod customs;
mod events;
mod id_mapping;
mod messages;
mod partition_info;
mod splittable_network;
mod splittable_population;
mod splittable_scenario;
mod vehicles;

struct Simulation {
    scenario: ScenarioPartition,
    activity_q: AgentQ,
    teleportation_q: AgentQ,
    events: Events,
}

impl Simulation {
    fn new(scenario: ScenarioPartition, events: Events) -> Simulation {
        let mut q = AgentQ::new();
        for (_, agent) in scenario.population.agents.iter() {
            q.add(agent, 0);
        }

        Simulation {
            scenario,
            activity_q: q,
            teleportation_q: AgentQ::new(),
            events,
        }
    }

    fn create_simulation_partitions(scenario: Scenario, events: &Events) -> Vec<Simulation> {
        let simulations: Vec<_> = scenario
            .scenarios
            .into_iter()
            // this clones the sender end of the writer but not the worker part.
            .map(|partition| Simulation::new(partition, events.clone()))
            .collect();

        simulations
    }

    fn run(&mut self) {
        println!(
            "Simulation #{}: Starting simulation loop.",
            self.scenario.customs.id
        );

        // use fixed start and end times
        let mut now = 0;
        let end_time = 86400;
        println!(
            "\n #### Start the simulation for Scenario Slice #{} at timestep {}. Last timestep is set to {} ####\n",
            self.scenario.customs.id, now, end_time
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
            if now % 3600 == 0 {
                let hour = now / 3600;
                println!("Simulation #{}: At: {hour}:00:00", self.scenario.customs.id);
            }

            self.wakeup(now);
            self.teleportation_arrivals(now);
            self.move_nodes(now);
            self.send(now);
            self.events.flush();
            self.receive(now);
            now += 1;
        }
    }

    fn wakeup(&mut self, now: u32) {
        let agents_2_link = self.activity_q.wakeup(now);

        if !agents_2_link.is_empty() {
            println!(
                "Simulation #{}: {} agents woke up. Creating vehicles and putting them onto links at time {now}",
                self.scenario.customs.id,
                agents_2_link.len()
            );
        }

        for id in agents_2_link {
            let agent = self.scenario.population.agents.get_mut(&id).unwrap();
            self.events.handle_act_end(now, &agent);
            agent.advance_plan();
            self.events.handle_departure(now, agent);

            if let PlanElement::Leg(leg) = agent.current_plan_element() {
                match &leg.route {
                    Route::NetworkRoute(net_route) => {
                        Simulation::push_onto_network(
                            &mut self.scenario.network,
                            &mut self.events,
                            net_route,
                            0,
                            agent.id,
                            now,
                        );
                    }
                    Route::GenericRoute(_) => {
                        if Simulation::is_local_teleportation(agent, &self.scenario.customs) {
                            self.teleportation_q.add(agent, now);
                        } else {
                            // copy the id here, so that the reference to agent can be dropped before we
                            // attempt to own the agent to move it to customs.
                            let id = agent.id;
                            let agent = self.scenario.population.agents.remove(&id).unwrap();
                            self.scenario.customs.prepare_to_teleport(agent);
                        }
                    }
                }
            }
        }
    }

    fn teleportation_arrivals(&mut self, now: u32) {
        let agents_2_activity = self.teleportation_q.wakeup(now);
        for id in agents_2_activity {
            let agent = self.scenario.population.agents.get_mut(&id).unwrap();
            self.events.handle_travelled(now, agent);
            agent.advance_plan();
            self.activity_q.add(agent, now);
        }
    }

    fn move_nodes(&mut self, now: u32) {
        for node in self.scenario.network.nodes.values() {
            let exited_vehicles =
                node.move_vehicles(&mut self.scenario.network.links, now, &mut self.events);

            for exit_reason in exited_vehicles {
                match exit_reason {
                    ExitReason::FinishRoute(vehicle) => {
                        let agent = self
                            .scenario
                            .population
                            .agents
                            .get_mut(&vehicle.driver_id)
                            .unwrap();

                        self.events
                            .handle_person_leaves_vehicle(now, agent, &vehicle);
                        self.events.handle_arrival(now, agent);

                        agent.advance_plan();
                        self.events.handle_act_start(now, agent);
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

    fn send(&mut self, now: u32) {
        self.scenario.customs.send(now);
    }

    fn receive(&mut self, now: u32) {
        let messages = self.scenario.customs.receive();
        for message in messages {
            for vehicle in message.vehicles {
                let agent = vehicle.0;
                let route_index = vehicle.1;
                println!(
                    "Thread #{} has received Agent #{} with route index {} at time {now}",
                    self.scenario.customs.id, agent.id, route_index
                );
                if let PlanElement::Leg(leg) = agent.current_plan_element() {
                    match &leg.route {
                        Route::NetworkRoute(net_route) => {
                            Simulation::push_onto_network(
                                &mut self.scenario.network,
                                &mut self.events,
                                net_route,
                                route_index,
                                agent.id,
                                now,
                            );
                            self.scenario.population.agents.insert(agent.id, agent);
                        }
                        Route::GenericRoute(_) => {
                            self.teleportation_q.add(&agent, now);
                        }
                    }
                }
            }
        }
    }

    fn active_agents(&self) -> usize {
        1 // this needs something else maybe a counter would do.
          //self.scenario.population.agents.len() - self.activity_q.finished_agents()
    }

    fn push_onto_network(
        network: &mut NetworkPartition,
        events: &mut Events,
        route: &NetworkRoute,
        route_index: usize,
        driver_id: usize,
        now: u32,
    ) -> Option<Vehicle> {
        let mut vehicle = Vehicle::new(route.vehicle_id, driver_id, route.route.clone());
        vehicle.route_index = route_index;
        let link_id = route.route.get(route_index).unwrap();
        let link = network.links.get_mut(link_id).unwrap();

        match link {
            Link::LocalLink(local_link) => {
                if route_index == 0 {
                    events.handle_person_enters_vehicle(now, driver_id, &vehicle)
                } else {
                    events.handle_vehicle_enters_link(now, local_link.id, vehicle.id);
                }

                local_link.push_vehicle(vehicle, now);
                None
            }
            // I am not sure whether this is even possible.
            Link::SplitLink(_) => Some(vehicle),
        }
    }

    fn is_local_teleportation(agent: &Agent, customs: &Customs) -> bool {
        let (start_thread, end_thread) =
            Simulation::get_thread_ids_for_generic_route(agent, customs);
        start_thread == end_thread
    }

    fn get_thread_ids_for_generic_route(agent: &Agent, customs: &Customs) -> (usize, usize) {
        if let PlanElement::Leg(leg) = agent.current_plan_element() {
            if let Route::GenericRoute(route) = &leg.route {
                let start_thread = *customs.get_thread_id(&route.start_link);
                let end_thread = *customs.get_thread_id(&route.end_link);
                return (start_thread, end_thread);
            }
        }
        panic!("This should not happen!!!")
    }
}

#[cfg(test)]
mod test {
    use crate::io::network::IONetwork;
    use crate::io::non_blocking_io::NonBlocking;
    use crate::io::population::IOPopulation;
    use crate::parallel_simulation::events::Events;
    use crate::parallel_simulation::splittable_scenario::Scenario;
    use crate::parallel_simulation::Simulation;
    use std::fs::File;
    use std::thread;
    use std::thread::JoinHandle;

    /// This creates a scenario with three links and one agent. The scenario is not split up, therefore
    /// a single threaded simulation is run. This test exists to see whether the logic of the simulation
    /// without passing messages to other simulation slices works.
    #[test]
    fn run_single_agent_single_slice() {
        let mut network = IONetwork::from_file("./assets/3-links/3-links-network.xml");
        let population = IOPopulation::from_file("./assets/3-links/1-agent.xml");

        let scenario = Scenario::from_io(&mut network, &population, 1);
        let (writer, guard) = NonBlocking::from_file("./test-events.xml");
        let mut events = Events::new(writer);
        let mut simulations = Simulation::create_simulation_partitions(scenario, &events);

        assert_eq!(1, simulations.len());
        let mut simulation = simulations.remove(0);
        simulation.run();
        events.finish();

        println!("done.")
    }

    /// This creates a scenario with three links and one agent. The scenario is split into two domains.
    /// The scenario should contain one split link "link2". Nodes 1 and 2 should be in the first, 3 and 4
    /// should end up in the second domain. The agent starts at link1, enters link2, gets passed to
    /// the other domain, leaves link2, enters link3 and finishes its route on link3
    #[test]
    fn run_single_agent_two_slices() {
        let mut network = IONetwork::from_file("./assets/3-links/3-links-network.xml");
        let population = IOPopulation::from_file("./assets/3-links/1-agent.xml");

        let scenario = Scenario::from_io(&mut network, &population, 2);
        let (writer, guard) = NonBlocking::from_file("./test-log.txt");
        let mut events = Events::new(writer);
        let simulations = Simulation::create_simulation_partitions(scenario, &events);

        let join_handles: Vec<_> = simulations
            .into_iter()
            .map(|mut simulation| thread::spawn(move || simulation.run()))
            .collect();

        for handle in join_handles {
            handle.join().unwrap();
        }
        events.finish();
    }
    /*
    #[test]
    fn run_equil_scenario() {
        // load input files
        let mut network = IONetwork::from_file("./assets/equil-network.xml");
        let population = IOPopulation::from_file("./assets/equil_output_plans.xml.gz");

        // convert input into simulation
        let scenarios = Scenario::from_io(&mut network, &population, 2);
        let simulations = Simulation::create_simulation_partitions(scenarios);

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

    #[test]
    #[ignore]
    fn run_berlin_scenario() {
        let mut network = IONetwork::from_file("/home/janek/test-files/berlin-test-network.xml.gz");
        let population =
            IOPopulation::from_file("/home/janek/test-files/berlin-all-plans-without-pt.xml.gz");

        let scenarios = Scenario::from_io(&mut network, &population, 16);
        let simulations = Simulation::create_simulation_partitions(scenarios);

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

     */
}
