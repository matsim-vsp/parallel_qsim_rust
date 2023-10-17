use crate::simulation::messaging::messages::proto::{Activity, Agent, Leg, Plan, Route};

pub fn create_agent(id: u64, route: Vec<u64>) -> Agent {
    let route = Route {
        veh_id: id,
        distance: 0.0,
        route,
    };
    let leg = Leg::new(route, 0, 0, None);
    let act = Activity::new(0., 0., 0, 1, None, None, None);
    let mut plan = Plan::new();
    plan.add_act(act);
    plan.add_leg(leg);
    let mut agent = Agent::new(id, plan);
    agent.advance_plan();

    agent
}
