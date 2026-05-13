use crate::simulation::events::{ActivityEndEvent, ActivityStartEvent, EventTrait, PersonArrivalEvent, PersonDepartureEvent};
use crate::simulation::id::Id;
use crate::simulation::scenario::Coordinate;
use crate::simulation::scenario::network::Link;
use crate::simulation::scenario::population::{InternalActivity, InternalLeg, InternalPerson, InternalPlanElement, InternalRoute};
use crate::simulation::scenario::population::InternalPlanElement::Activity;
use crate::simulation::scenario::vehicles::InternalVehicle;

pub struct BackpackPlan {
    elements: Vec<InternalPlanElement>,

    current_activity: Option<BackpackActivity>,
    current_leg: Option<BackpackLeg>,
}

impl Default for BackpackPlan {
    fn default() -> Self {
        Self {
            elements: Vec::default(),
            current_activity: None,
            current_leg: None,
        }
    }
}

impl BackpackPlan {
    fn handle_person_departure(&mut self, event: PersonDepartureEvent) {
        if self.current_leg.is_some() {
            panic!("Illegal state: Person departs while having an active leg!");
        }

        self.current_leg = Some(BackpackLeg::default());
        self.current_leg.as_mut().unwrap().handle_person_departure(event);
    }

    fn handle_person_arrival(&mut self, event: PersonArrivalEvent) {
        if self.current_leg.is_none() {
            panic!("Illegal state: Person arrives while having no active leg!");
        }

        self.current_leg.as_mut().unwrap().handle_person_arrival(event);
        self.elements.push(Activity(self.current_activity.take().unwrap().finish()));
    }

    fn handle_activity_start(&mut self, event: ActivityStartEvent) {
        if self.current_leg.is_some() {
            panic!("Illegal state: Person starts activity while doing an activity!");
        }

        self.current_activity = Some(BackpackActivity::default());
        self.current_activity.as_mut().unwrap().handle_activity_start(event);
    }

    fn handle_activity_end(&mut self, event: ActivityEndEvent) {
        if self.current_leg.is_none() {
            panic!("Illegal state: Person ends activity while not doing an activity!");
        }

        self.current_activity.as_mut().unwrap().handle_activity_end(event);
        self.elements.push(InternalPlanElement::Leg(self.current_leg.take().unwrap().finish()))
    }

    /// This function is only used to pass events to the BackpackRoute event handlers
    fn handle_event(&mut self, event: Box<dyn EventTrait>){
        if self.current_leg.is_some() {
            self.current_leg.as_mut().unwrap().handle_event(event);
        }
    }
}

// TODO Add verify() methods to the structs below!


struct BackpackLeg {
    pub mode: Option<Id<String>>,
    pub routing_mode: Option<Id<String>>, // TODO Seems like this var is not needed
    pub dep_time: Option<u32>,
    pub trav_time: Option<u32>,
    pub backpack_route: BackpackRoute,
}

impl Default for BackpackLeg {
    fn default() -> Self {
        Self {
            mode: None,
            routing_mode: None,
            dep_time: None,
            trav_time: None,
            backpack_route: BackpackRoute::default(),
        }
    }
}

impl BackpackLeg {

    // TODO handlers currently skip: backpack_route

    fn handle_person_departure(&mut self, event: PersonDepartureEvent) {
        self.mode = Some(event.leg_mode);
        self.routing_mode = Some(event.routing_mode);
        self.dep_time = Some(event.time);

        // self.backpack_route.handle_person_departure(); TODO
    }

    fn handle_person_arrival(&mut self, event: PersonArrivalEvent) {
        self.trav_time = Some(event.time - self.dep_time.unwrap());

        // self.backpack_route.handle_person_arrival(); TODO
    }

    fn handle_event(&mut self, event: Box<dyn EventTrait>) {
        self.backpack_route.handle_event(event);
    }

    fn finish(self) -> InternalLeg {
        InternalLeg::new(
            self.backpack_route
                .finish(),
            self.mode
                .unwrap_or_else(|| panic!("Tried to finish BackpackLeg without mode!"))
                .external(),
            self.trav_time
                .unwrap_or_else(|| panic!("Tried to finish BackpackLeg without trav_time!")),
            self.dep_time
        )
    }

}

enum BackpackRouteTypes{
    Generic,
    Network,
}

struct BackpackRoute {
    route_type: Option<BackpackRouteTypes>,

    // Generic Route Type
    start_link: Option<Id<Link>>,
    end_link: Option<Id<Link>>,
    trav_time: Option<u32>,
    distance: Option<f64>,
    vehicle: Option<Id<InternalVehicle>>,

    // Network Route Type
    route: Option<Vec<Id<Link>>>
}

impl Default for BackpackRoute {
    fn default() -> Self {
        Self {
            route_type: None,
            start_link: None,
            end_link: None,
            trav_time: None,
            distance: None,
            vehicle: None,
            route: None,
        }
    }
}

impl BackpackRoute {

    fn handle_event(&mut self, event: Box<dyn EventTrait>) {
        todo!()
    }

    fn finish(self) -> InternalRoute {
        todo!()
    }
}

struct BackpackActivity {
    pub act_type: Option<Id<String>>,
    pub link_id: Option<Id<Link>>,
    pub coordinate: Option<Coordinate>,
    pub start_time: Option<u32>,
    pub end_time: Option<u32>,
    // pub max_dur: Option<u32>, (not meant to be set in the experienced plans)
}

impl Default for BackpackActivity {
    fn default() -> Self {
        Self {
            act_type: None,
            link_id: None,
            coordinate: None,
            start_time: None,
            end_time: None,
        }
    }
}

impl BackpackActivity {
    fn handle_activity_start(&mut self, event: ActivityStartEvent) {
        self.act_type = Some(event.act_type);
        self.link_id = Some(event.link);
        self.coordinate = Some(event.coordinate);
        self.start_time = Some(event.time);
    }

    fn handle_activity_end(&mut self, event: ActivityEndEvent) {
        self.end_time = Some(event.time);
    }

    /// Consuming function turning BackpackActivity into an InternalActivity
    fn finish(self) -> InternalActivity {
        InternalActivity::new(
            self.coordinate
                .unwrap_or_else(|| panic!("Tried to finish BackpackActivity without coordinate!")),
            self.act_type
                .unwrap_or_else(|| panic!("Tried to finish BackpackActivity without act type!"))
                .external(),
            self.link_id
                .unwrap_or_else(|| panic!("Tried to finish BackpackActivity without link!")),
            self.start_time,
            self.end_time,
            None
        )
    }
}

/// Backpacks store the Events as well as a partial plan ([BackpackPlan]) for each agent.
/// The Backpack is not managed by the agent itself but by the [BackpackDataCollector], which exists
/// once for each partition. If an agent leaves the current partition, the Backpack is transmitted
/// to the partition the agent is currently entering.
pub struct Backpack{
    person_id: Id<InternalPerson>,
    events: Vec<Box<dyn EventTrait>>,
    backpack_plan: BackpackPlan,
    starting_partition: u32
}

impl Backpack {
    pub fn new(person_id: Id<InternalPerson>, starting_partition: u32) -> Self {
        Self {
            person_id,
            events: Default::default(),
            backpack_plan: BackpackPlan::default(),
            starting_partition
        }
    }

    // Node internal functions

    pub fn add_special_scoring_event(&mut self, event: Box<dyn EventTrait>) {
        self.events.push(event);
    }


    // Inter-node functions

    pub fn to_message(self) -> String {
        // TODO Serialize function
        String::from("Hello")
    }
}