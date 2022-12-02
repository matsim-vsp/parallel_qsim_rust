use crate::parallel_simulation::splittable_population::{Agent, PlanElement, Route};
use crate::parallel_simulation::vehicles::Vehicle;
use std::mem::take;

/// Events takes a writer. This is the trait for that
pub trait EventsWriter: EventsWriterClone + Send {
    fn write(&self, buf: Vec<u8>);
}

/// Since we want Events to be clonable but we also want the writer to be a dynamic object, we need to
/// specify how to clone the Box<dyn EventsWriter> property in Events. The following three method are
/// concerned about that. Implemented according to https://stackoverflow.com/questions/50017987/cant-clone-vecboxtrait-because-trait-cannot-be-made-into-an-object
///
pub trait EventsWriterClone {
    fn clone_boxed(&self) -> Box<dyn EventsWriter>;
}

/// this implements clone_boxed for all owned EventsWriters e.g. Box<dyn EventsWriter>. The 'static
/// lifetime applies to all values that are owned. See last paragraph (Trait bound) of
/// https://doc.rust-lang.org/rust-by-example/scope/lifetime/static_lifetime.html
impl<T: EventsWriter + Clone + 'static> EventsWriterClone for T {
    fn clone_boxed(&self) -> Box<dyn EventsWriter> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn EventsWriter> {
    fn clone(&self) -> Self {
        self.clone_boxed()
    }
}

#[derive(Clone)]
pub struct ConsoleWriter {}

impl EventsWriter for ConsoleWriter {
    fn write(&self, buf: Vec<u8>) {
        println!("{}", String::from_utf8_lossy(&*buf));
    }
}

#[derive(Clone)]
pub struct SilentWriter {}

impl EventsWriter for SilentWriter {
    fn write(&self, _buf: Vec<u8>) {
        // nothing. just swallow all the messages.
    }
}

#[derive(Clone)]
pub struct Events {
    buffer: Vec<u8>,
    writer: Box<dyn EventsWriter>,
}

impl Events {
    pub fn new(writer: impl EventsWriter + 'static) -> Events {
        let header = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<events version=\"1.0\">\n";
        writer.write(header.as_bytes().to_vec());

        Events {
            writer: Box::new(writer),
            buffer: Vec::new(),
        }
    }

    pub fn new_with_console_writer() -> Events {
        Events {
            writer: Box::new(ConsoleWriter {}),
            buffer: Vec::new(),
        }
    }

    pub fn new_silent() -> Events {
        Events {
            writer: Box::new(SilentWriter {}),
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
