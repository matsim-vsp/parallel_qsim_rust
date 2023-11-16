use tracing::info;

use crate::simulation::id::Id;
use crate::simulation::io::population::{IOPlanElement, IOPopulation};
use crate::simulation::network::global_network::Network;
use crate::simulation::vehicles::garage::Garage;
use crate::simulation::wire_types::population::Person;

pub fn load_population(
    io_pop: &IOPopulation,
    net: &Network,
    garage: &mut Garage,
    num_parts: u32,
) -> Vec<crate::simulation::wire_types::population::Population> {
    create_ids(io_pop, garage);
    prepare_populations_to_send(io_pop, net, num_parts)
}

fn create_ids(io_pop: &IOPopulation, garage: &mut Garage) {
    info!("Creating person ids.");
    // create person ids and collect strings for vehicle ids
    let raw_veh: Vec<_> = io_pop
        .persons
        .iter()
        .map(|p| Id::<Person>::create(p.id.as_str()))
        .flat_map(|p_id| {
            garage
                .vehicle_types
                .keys()
                .map(move |type_id| (p_id.clone(), type_id.clone()))
        })
        .collect();

    info!("Creating interaction activity types");
    // add interaction activity type for each vehicle type
    for (_, id) in raw_veh.iter() {
        Id::<String>::create(&format!("{} interaction", id.external()));
    }

    info!("Creating vehicle ids");
    for (person_id, type_id) in raw_veh {
        let veh_id_ext = format!("{}_{}", person_id.external(), type_id.external());
        garage.add_veh_id(&person_id, &type_id);
    }

    info!("Creating activity types");
    // now iterate over all plans to extract activity ids
    io_pop
        .persons
        .iter()
        .flat_map(|person| person.plans.iter())
        .flat_map(|plan| plan.elements.iter())
        .filter_map(|element| match element {
            IOPlanElement::Activity(a) => Some(a),
            IOPlanElement::Leg(_) => None,
        })
        .map(|act| &act.r#type)
        .for_each(|act_type| {
            Id::<String>::create(act_type.as_str());
        });
}

fn prepare_populations_to_send(
    io_pop: &IOPopulation,
    net: &Network,
    num_parts: u32,
) -> Vec<crate::simulation::wire_types::population::Population> {
    let mut result = vec![
        crate::simulation::wire_types::population::Population {
            persons: Vec::default(),
        };
        num_parts as usize
    ];

    info!("Creating populations to send");
    io_pop.persons.iter().map(Person::from_io).for_each(|p| {
        let partition = partition_of_first_act(&p, net);
        result.get_mut(partition as usize).unwrap().persons.push(p);
    });

    result
}

fn partition_of_first_act(p: &Person, net: &Network) -> u32 {
    let link_id = p.curr_act().link_id;
    net.get_link(&Id::get(link_id)).partition
}
