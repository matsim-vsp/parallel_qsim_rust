use crate::container::population::{IOPlanElement, IOPopulation};
use crate::parallel_simulation::splittable_population::{Agent, PlanElement, Route};
use std::collections::HashMap;

pub struct Vehicle {
    pub id: usize,
    pub driver: Agent,
    pub exit_time: u32,
    pub route_index: usize,
}

impl Vehicle {
    pub fn new(id: usize, driver: Agent) -> Vehicle {
        Vehicle {
            id,
            driver,
            exit_time: 0,
            route_index: 0,
        }
    }

    pub fn advance_route_index(&mut self) {
        self.route_index += 1;
    }

    pub fn current_link_id(&self) -> usize {
        if let PlanElement::Leg(leg) = self.driver.current_plan_element() {
            if let Route::NetworkRoute(ref route) = leg.route {
                return *route.route.get(self.route_index).unwrap();
            }
        }

        panic!(
            "could not get link id for vehicle {} with driver {} for link index {}",
            self.id, self.driver.id, self.route_index
        );
    }
}

pub struct VehiclesIdMapping<'id> {
    next_id: usize,
    matsim_id_2_id: HashMap<&'id str, usize>,
}

impl<'id> VehiclesIdMapping<'id> {
    pub fn new() -> VehiclesIdMapping<'id> {
        VehiclesIdMapping {
            matsim_id_2_id: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn from_population(population: &IOPopulation) -> VehiclesIdMapping {
        let mut vehicle_id_mapping = VehiclesIdMapping::new();

        population
            .persons
            .iter()
            .map(|p| p.selected_plan())
            .flat_map(|p| p.elements.iter())
            .filter(|el| matches!(el, IOPlanElement::Leg(_)))
            .map(|el| match el {
                IOPlanElement::Leg(leg) => leg,
                _ => panic!(""),
            })
            .filter(|leg| leg.route.r#type == "links")
            .map(|leg| leg.route.vehicle.as_ref().unwrap())
            .for_each(|veh_id| {
                vehicle_id_mapping.map_vehicle_id(veh_id);
            });

        vehicle_id_mapping
    }

    pub fn map_vehicle_id(&mut self, matsim_id: &'id str) -> usize {
        let id = self.matsim_id_2_id.entry(matsim_id).or_insert(self.next_id);

        if self.next_id == *id {
            self.next_id += 1;
        }

        *id
    }

    pub fn get_from_matsim_id(&self, matsim_id: &str) -> usize {
        *self.matsim_id_2_id.get(matsim_id).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::container::population::IOPopulation;
    use crate::parallel_simulation::vehicles::VehiclesIdMapping;

    #[test]
    fn id_mapping_from_population() {
        let population = IOPopulation::from_file("./assets/equil_output_plans.xml.gz");
        let mapping = VehiclesIdMapping::from_population(&population);

        // for our test set up each person has 1 vehicle.
        assert_eq!(population.persons.len(), mapping.matsim_id_2_id.len());
    }
}
