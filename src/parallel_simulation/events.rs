use crate::container::non_blocking_io::{NonBlocking, WorkerGuard};
use crate::parallel_simulation::splittable_population::{Agent, PlanElement, Route};
use crate::parallel_simulation::vehicles::Vehicle;

#[derive(Clone, Debug)]
pub struct Events<'a> {
    writer: NonBlocking,
    // force the guard to have a longer lifetime than any events struct
    guard: &'a WorkerGuard,
}

impl<'a> Events<'a> {
    pub fn new(writer: NonBlocking, guard: &'a WorkerGuard) -> Events<'a> {
        let header = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<events version=\"1.0\">\n";
        writer.write(header.as_ref());

        Events {
            writer,
            guard: &guard,
        }
    }

    pub fn handle(&self, event: &str) {
        self.writer.write(event.as_ref());
    }

    pub fn finish(&self) {
        let closing_tag = "</events>";
        self.writer.write(closing_tag.as_ref())
    }

    //pub fn handle_act_start(&self, now: u32, agent_id: usize, link_id: usize, act_type: &str) {
    pub fn handle_act_start(&self, now: u32, agent: &Agent) {
        if let PlanElement::Activity(act) = agent.current_plan_element() {
            let id = agent.id;
            let link = act.link_id;
            let act_type = &act.act_type;
            self.handle(format!("<event time=\"{now}\" type=\"actstart\" person=\"{id}\" link=\"{link}\" actType=\"{act_type}\" />\n").as_str());
        }
    }

    pub fn handle_act_end(&self, now: u32, agent: &Agent) {
        if let PlanElement::Activity(act) = agent.current_plan_element() {
            let id = agent.id;
            let link = act.link_id;
            let act_type = &act.act_type;
            self.handle(format!("<event time=\"{now}\" type=\"actend\" person=\"{id}\" link=\"{link}\" actType=\"{act_type}\" />\n").as_str());
        }
    }

    pub fn handle_departure(&self, now: u32, agent: &Agent) {
        if let PlanElement::Leg(leg) = agent.current_plan_element() {
            let (id, link, mode) = match &leg.route {
                Route::NetworkRoute(net_route) => (
                    agent.id,
                    *net_route.route.get(0).unwrap(),
                    leg.mode.as_str(),
                ),
                Route::GenericRoute(gen_route) => {
                    (agent.id, gen_route.start_link, leg.mode.as_str())
                }
            };
            self.handle(format!("<event time=\"{now}\" type=\"departure\" person=\"{id}\" link=\"{link}\" legMode=\"{mode}\" />\n").as_str())
        }
    }

    pub fn handle_travelled(&self, now: u32, agent: &Agent) {
        if let PlanElement::Leg(leg) = agent.current_plan_element() {
            if let Route::GenericRoute(route) = &leg.route {
                let id = agent.id;
                let distance = route.distance;
                let mode = leg.mode.as_str();
                self.handle(format!("<event time=\"{now}\" type=\"travelled\" person=\"{id}\" distance=\"{distance}\" mode=\"{mode}\" />\n").as_str())
            }
        }
    }

    pub fn handle_arrival(&self, now: u32, agent: &Agent) {
        if let PlanElement::Leg(leg) = agent.current_plan_element() {
            let (id, link, mode) = match &leg.route {
                Route::NetworkRoute(net_route) => (
                    agent.id,
                    *net_route.route.get(0).unwrap(),
                    leg.mode.as_str(),
                ),
                Route::GenericRoute(gen_route) => {
                    (agent.id, gen_route.start_link, leg.mode.as_str())
                }
            };
            self.handle(format!("<event time=\"{now}\" type=\"arrival\" person=\"{id}\" link=\"{link}\" legMode=\"{mode}\" />\n").as_str())
        }
    }

    pub fn handle_person_enters_vehicle(&self, now: u32, id: usize, vehicle: &Vehicle) {
        self.handle(format!("<event time=\"{now}\" type=\"PersonEntersVehicle\" person=\"{id}\" vehicle=\"{}\" />\n", vehicle.id).as_str())
    }

    pub fn handle_person_leaves_vehicle(&self, now: u32, agent: &Agent, vehicle: &Vehicle) {
        self.handle(format!("<event time=\"{now}\" type=\"PersonLeavesVehicle\" person=\"{}\" vehicle=\"{}\" />\n", agent.id, vehicle.id).as_str())
    }
}
