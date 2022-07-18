use crate::io::non_blocking_io::NonBlocking;
use crate::parallel_simulation::splittable_population::{Agent, PlanElement, Route};
use crate::parallel_simulation::vehicles::Vehicle;
use std::mem::take;

#[derive(Clone, Debug)]
pub struct Events {
    writer: NonBlocking,
    buffer: Vec<u8>,
}

impl Events {
    pub fn new(writer: NonBlocking) -> Events {
        let header = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<events version=\"1.0\">\n";
        writer.write(header.as_bytes().to_vec());

        Events {
            writer,
            buffer: Vec::new(),
        }
    }

    pub fn handle(&mut self, event: &str) {
        let mut vec = event.as_bytes().to_vec();
        self.buffer.append(&mut vec);
    }

    pub fn flush(&mut self) {
        let buffer = take(&mut self.buffer);
        self.writer.write(buffer)
    }

    pub fn finish(&mut self) {
        let closing_tag = "</events>";
        self.handle(closing_tag.as_ref());
        self.flush();
    }

    //pub fn handle_act_start(&self, now: u32, agent_id: usize, link_id: usize, act_type: &str) {
    pub fn handle_act_start(&mut self, now: u32, agent: &Agent) {
        if let PlanElement::Activity(act) = agent.current_plan_element() {
            let id = agent.id;
            let link = act.link_id;
            let act_type = &act.act_type;
            self.handle(format!("<event time=\"{now}\" type=\"actstart\" person=\"{id}\" link=\"{link}\" actType=\"{act_type}\" />\n").as_str());
        }
    }

    pub fn handle_act_end(&mut self, now: u32, agent: &Agent) {
        if let PlanElement::Activity(act) = agent.current_plan_element() {
            let id = agent.id;
            let link = act.link_id;
            let act_type = &act.act_type;
            self.handle(format!("<event time=\"{now}\" type=\"actend\" person=\"{id}\" link=\"{link}\" actType=\"{act_type}\" />\n").as_str());
        }
    }

    pub fn handle_departure(&mut self, now: u32, agent: &Agent) {
        if let PlanElement::Leg(leg) = agent.current_plan_element() {
            let (id, link, mode) = match &leg.route {
                Route::NetworkRoute(net_route) => (
                    agent.id,
                    *net_route.route.first().unwrap(),
                    leg.mode.as_str(),
                ),
                Route::GenericRoute(gen_route) => {
                    (agent.id, gen_route.start_link, leg.mode.as_str())
                }
            };
            self.handle(format!("<event time=\"{now}\" type=\"departure\" person=\"{id}\" link=\"{link}\" legMode=\"{mode}\" />\n").as_str())
        }
    }

    pub fn handle_travelled(&mut self, now: u32, agent: &Agent) {
        if let PlanElement::Leg(leg) = agent.current_plan_element() {
            if let Route::GenericRoute(route) = &leg.route {
                let id = agent.id;
                let distance = route.distance;
                let mode = leg.mode.as_str();
                self.handle(format!("<event time=\"{now}\" type=\"travelled\" person=\"{id}\" distance=\"{distance}\" mode=\"{mode}\" />\n").as_str())
            }
        }
    }

    pub fn handle_arrival(&mut self, now: u32, agent: &Agent) {
        if let PlanElement::Leg(leg) = agent.current_plan_element() {
            let (id, link, mode) = match &leg.route {
                Route::NetworkRoute(net_route) => (
                    agent.id,
                    *net_route.route.last().unwrap(),
                    leg.mode.as_str(),
                ),
                Route::GenericRoute(gen_route) => {
                    (agent.id, gen_route.start_link, leg.mode.as_str())
                }
            };
            self.handle(format!("<event time=\"{now}\" type=\"arrival\" person=\"{id}\" link=\"{link}\" legMode=\"{mode}\" />\n").as_str())
        }
    }

    pub fn handle_person_enters_vehicle(&mut self, now: u32, id: usize, vehicle: &Vehicle) {
        self.handle(format!("<event time=\"{now}\" type=\"PersonEntersVehicle\" person=\"{id}\" vehicle=\"{}\" />\n", vehicle.id).as_str())
    }

    pub fn handle_person_leaves_vehicle(&mut self, now: u32, agent: &Agent, vehicle: &Vehicle) {
        self.handle(format!("<event time=\"{now}\" type=\"PersonLeavesVehicle\" person=\"{}\" vehicle=\"{}\" />\n", agent.id, vehicle.id).as_str())
    }

    pub fn handle_vehicle_leaves_link(&mut self, now: u32, link_id: usize, vehicle_id: usize) {
        self.handle(format!("<event time=\"{now}\" type=\"left link\" link=\"{link_id}\" vehicle=\"{vehicle_id}\" />\n").as_ref())
    }

    pub fn handle_vehicle_enters_link(&mut self, now: u32, link_id: usize, vehicle_id: usize) {
        self.handle(format!("<event time=\"{now}\" type=\"entered link\" link=\"{link_id}\" vehicle=\"{vehicle_id}\" />\n").as_ref())
    }
}
