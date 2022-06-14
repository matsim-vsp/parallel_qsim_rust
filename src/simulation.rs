use crate::container::network::Network;
use crate::container::population::Population;
use crate::simulation::activity_q::ActivityQ;
use crate::simulation::q_network::QNetwork;
use crate::simulation::q_population::QPopulation;

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
    let q_network = QNetwork::from_container(&network);
    // 2. population
    let q_population = QPopulation::from_container(&population, &q_network);

    // prepare the simulation:
    // put all people into the activity q, assuming that everybody has activity as first plan element
    let mut activity_q = ActivityQ::new();
    for agent in &q_population.agents {
        activity_q.add(agent, 0);
    }

    // start simulation loop
    //  for each timestep it should check whether anybody should be put onto the network
    //  for each timestep it should iterate each node and execute move_node on it.
    //    this could also check if a vehicle is eligible for exiting a link whether it has reached the
    //    end of its current route. If so the person inside the vehicle will be put into the q of the
    //    activity manager.
}
