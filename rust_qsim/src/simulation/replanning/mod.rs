use crate::simulation::scenario::population::InternalPerson;

pub mod routing;

// General TODO:
// Introduce TripRouter, NetworkRoutingModule
// rename ALT router to ALT least cost path calculator

struct StrategyManager {
    // store the weights of the strategies by subpopulation
}

impl StrategyManager {
    // pick a strategy for a plan and run the strategy
    // remove plans if memory is full
}

// This is responsible for picking a plan, copying it and replanning it.
trait PlanStrategy {
    fn handle(&mut self, person: &mut InternalPerson);
}

// This is the smallest replanning unit (e.g., routes a plan).
trait PlanStrategyModule {
    fn handle(&mut self, person: &mut InternalPerson, plan_index: usize);
}

struct ReRouteModule {
    // hold reference to scenario
    // hold reference to router
}

impl PlanStrategyModule for ReRouteModule {
    fn handle(&mut self, person: &mut InternalPerson, plan_index: usize) {
        //extract the trips from the plan
        //extract vehicle from the scenario
        //call the router correspondingly
        todo!()
    }
}
