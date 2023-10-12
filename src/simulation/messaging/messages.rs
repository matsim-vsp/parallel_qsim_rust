use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::Cursor;

use log::debug;
use prost::Message;

use crate::simulation::id::Id;
use crate::simulation::io::attributes::Attrs;
use crate::simulation::io::population::{
    IOActivity, IOLeg, IOPerson, IOPlan, IOPlanElement, IORoute,
};
use crate::simulation::messaging::messages::proto::{
    Activity, Agent, ExperimentalMessage, Leg, Plan, Route, TravelTimesMessage, Vehicle,
    VehicleMessage,
};
use crate::simulation::network::global_network::Network;
use crate::simulation::population::population::Population;
use crate::simulation::time_queue::EndTime;
use crate::simulation::vehicles::garage::Garage;

// Include the `messages` module, which is generated from messages.proto.
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/mpi.messages.rs"));
}

impl ExperimentalMessage {
    pub fn new() -> ExperimentalMessage {
        ExperimentalMessage {
            counter: 0,
            timestamp: 0,
            additional_message: String::new(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(self.encoded_len());
        self.encode(&mut buf).unwrap();
        buf
    }

    pub fn deserialize(buf: &[u8]) -> ExperimentalMessage {
        ExperimentalMessage::decode(&mut Cursor::new(buf)).unwrap()
    }
}

impl VehicleMessage {
    pub fn new(time: u32, from: u32, to: u32) -> VehicleMessage {
        VehicleMessage {
            time,
            from_process: from,
            to_process: to,
            vehicles: Vec::new(),
        }
    }

    pub fn add(&mut self, vehicle: Vehicle) {
        self.vehicles.push(vehicle);
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(self.encoded_len());
        self.encode(&mut buffer).unwrap();
        buffer
    }

    pub fn deserialize(buffer: &[u8]) -> VehicleMessage {
        VehicleMessage::decode(&mut Cursor::new(buffer)).unwrap()
    }
}

impl TravelTimesMessage {
    pub fn new() -> Self {
        TravelTimesMessage {
            travel_times_by_link_id: HashMap::new(),
        }
    }

    pub fn from(map: HashMap<u64, u32>) -> Self {
        TravelTimesMessage {
            travel_times_by_link_id: map,
        }
    }

    pub fn add_travel_time(&mut self, link: u64, travel_time: u32) {
        self.travel_times_by_link_id.insert(link, travel_time);
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(self.encoded_len());
        self.encode(&mut buffer).unwrap();
        buffer
    }

    pub fn deserialize(buffer: &[u8]) -> TravelTimesMessage {
        TravelTimesMessage::decode(&mut Cursor::new(buffer)).unwrap()
    }
}

// Implementation for ordering, so that vehicle messages can be put into a message queue sorted by time
impl PartialOrd for VehicleMessage {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for VehicleMessage {}

impl Ord for VehicleMessage {
    fn cmp(&self, other: &Self) -> Ordering {
        other.time.cmp(&self.time)
    }
}

impl Vehicle {
    // todo, fix type and mode
    pub fn new(id: u64, veh_type: u64, max_v: f32, pce: f32, agent: Option<Agent>) -> Vehicle {
        Vehicle {
            id,
            agent,
            curr_route_elem: 0,
            r#type: veh_type,
            max_v,
            pce,
        }
    }

    pub fn agent(&self) -> &Agent {
        self.agent.as_ref().unwrap()
    }

    pub fn id(&self) -> usize {
        self.id as usize
    }

    pub fn advance_route_index(&mut self) {
        self.curr_route_elem += 1;
    }

    // todo I have changed the way this works. Probably one needs to call
    // advance route index for teleported legs now, once the person is woken up from the activity queue
    pub fn curr_link_id(&self) -> Option<usize> {
        let leg = self.agent().curr_leg();
        let route = leg.route.as_ref().unwrap();
        let index = self.curr_route_elem as usize;
        route.route.get(index).map(|link_id| *link_id as usize)
    }

    // todo same as above
    pub fn is_current_link_last(&self) -> bool {
        let leg = self.agent().curr_leg();
        let route = leg.route.as_ref().unwrap();
        self.curr_route_elem + 1 >= route.route.len() as u32
    }

    pub fn peek_next_route_element(&self) -> Option<usize> {
        let route = self.agent().curr_leg().route.as_ref().unwrap();
        let next_i = self.curr_route_elem as usize + 1;
        route.route.get(next_i).map(|i| *i as usize)
    }
}

impl EndTime for Vehicle {
    fn end_time(&self, now: u32) -> u32 {
        self.agent().end_time(now)
    }
}

impl Agent {
    pub fn from_io(
        io_person: &IOPerson,
        net: &Network,
        pop: &Population,
        garage: &Garage,
    ) -> Agent {
        let person_id = pop.agent_ids.get_from_ext(&io_person.id);

        let plan = Plan::from_io(io_person.selected_plan(), &person_id, net, pop, garage);

        if plan.acts.is_empty() {
            debug!("There is an empty plan for person {:?}", io_person.id);
        }

        if plan.acts.len() == 1 {
            debug!(
                "There is a plan with one activity only for person {:?}",
                io_person.id
            );
        }

        Agent {
            id: person_id.internal() as u64,
            plan: Some(plan),
            curr_plan_elem: 0,
        }
    }

    pub fn new(id: u64, plan: Plan) -> Self {
        Agent {
            id,
            curr_plan_elem: 0,
            plan: Some(plan),
        }
    }

    pub fn id(&self) -> usize {
        self.id as usize
    }

    pub fn curr_act(&self) -> &Activity {
        if self.curr_plan_elem % 2 != 0 {
            panic!("Current element is not an activity");
        }
        let act_index = self.curr_plan_elem / 2;
        self.get_act_at_index(act_index)
    }

    pub fn curr_act_mut(&mut self) -> &mut Activity {
        if self.curr_plan_elem % 2 != 0 {
            panic!("Current element is not an activity");
        }
        let act_index = self.curr_plan_elem / 2;
        self.get_act_at_index_mut(act_index)
    }

    pub fn next_act(&self) -> &Activity {
        let act_index = self.next_act_index();
        self.get_act_at_index(act_index)
    }

    pub fn next_act_mut(&mut self) -> &mut Activity {
        let act_index = self.next_act_index();
        self.get_act_at_index_mut(act_index)
    }

    fn next_act_index(&self) -> u32 {
        match self.curr_plan_elem % 2 {
            //current element is an activity => two elements after is the next activity
            0 => (self.curr_plan_elem + 2) / 2,
            //current element is a leg => one element after is the next activity
            1 => (self.curr_plan_elem + 1) / 2,
            _ => {
                panic!(
                    "There was an error while getting the next activity of agent {:?}",
                    self.id
                )
            }
        }
    }

    pub fn curr_leg(&self) -> &Leg {
        if self.curr_plan_elem % 2 != 1 {
            panic!("Current element is not a leg.");
        }

        let leg_index = (self.curr_plan_elem - 1) / 2;
        self.plan
            .as_ref()
            .unwrap()
            .legs
            .get(leg_index as usize)
            .unwrap()
    }

    pub fn next_leg(&self) -> &Leg {
        let next_leg_index = self.next_leg_index();
        self.get_leg_at_index(next_leg_index)
    }

    fn next_leg_index(&self) -> u32 {
        match self.curr_plan_elem % 2 {
            //current element is an activity => one element after is the next leg
            0 => (self.curr_plan_elem + 1) / 2,
            //current element is a leg => two elements after is the next leg
            1 => (self.curr_plan_elem + 2) / 2,
            _ => {
                panic!(
                    "There was an error while getting the next leg of agent {:?}",
                    self.id
                )
            }
        }
    }

    fn get_act_at_index(&self, index: u32) -> &Activity {
        self.plan
            .as_ref()
            .unwrap()
            .acts
            .get(index as usize)
            .unwrap()
    }

    fn get_act_at_index_mut(&mut self, index: u32) -> &mut Activity {
        self.plan
            .as_mut()
            .unwrap()
            .acts
            .get_mut(index as usize)
            .unwrap()
    }

    fn get_leg_at_index(&self, index: u32) -> &Leg {
        self.plan
            .as_ref()
            .unwrap()
            .legs
            .get(index as usize)
            .unwrap()
    }

    pub fn advance_plan(&mut self) {
        let next = self.curr_plan_elem + 1;
        if self.plan.as_ref().unwrap().acts.len() + self.plan.as_ref().unwrap().legs.len()
            == next as usize
        {
            panic!(
                "Agent: Advance plan was called on agent #{}, but no element is remaining.",
                self.id
            )
        }
        self.curr_plan_elem = next;
    }
}

impl EndTime for Agent {
    fn end_time(&self, now: u32) -> u32 {
        return if self.curr_plan_elem % 2 == 0 {
            self.curr_act().cmp_end_time(now)
        } else {
            self.curr_leg().trav_time.unwrap() + now
        };
    }
}

impl Plan {
    pub const DEFAULT_ROUTING_MODE: &'static str = "car";

    pub fn new() -> Plan {
        Plan {
            acts: Vec::new(),
            legs: Vec::new(),
        }
    }

    fn from_io(
        io_plan: &IOPlan,
        person_id: &Id<Agent>,
        net: &Network,
        pop: &Population,
        garage: &Garage,
    ) -> Plan {
        assert!(!io_plan.elements.is_empty());
        if let IOPlanElement::Leg(_leg) = io_plan.elements.get(0).unwrap() {
            panic!("First plan element must be an activity! But was a leg.");
        };

        Plan::get_full_plan_no_routing(io_plan, person_id, net, pop, garage)
    }

    fn get_full_plan_no_routing(
        io_plan: &IOPlan,
        person_id: &Id<Agent>,
        net: &Network,
        pop: &Population,
        garage: &Garage,
    ) -> Plan {
        let mut result = Plan::new();

        for element in &io_plan.elements {
            match element {
                IOPlanElement::Activity(io_act) => {
                    let act = Activity::from_io(io_act, net, pop);
                    result.acts.push(act);
                }
                IOPlanElement::Leg(io_leg) => {
                    let leg = Leg::from_io(io_leg, person_id, net, garage);
                    result.legs.push(leg);
                }
            }
        }

        if result.acts.len() - result.legs.len() != 1 {
            panic!("Plan {:?} has less legs than expected", io_plan);
        }

        result
    }

    pub fn add_leg(&mut self, leg: Leg) {
        self.legs.push(leg);
    }

    pub fn add_act(&mut self, activity: Activity) {
        self.acts.push(activity);
    }
}

impl Activity {
    fn from_io(io_act: &IOActivity, net: &Network, pop: &Population) -> Self {
        let link_id = net.link_ids.get_from_ext(&io_act.link);
        let act_type = pop.act_types.get_from_ext(&io_act.r#type);
        Activity {
            x: io_act.x,
            y: io_act.y,
            act_type: act_type.internal() as u64,
            link_id: link_id.internal() as u64,
            start_time: parse_time_opt(&io_act.start_time),
            end_time: parse_time_opt(&io_act.end_time),
            max_dur: parse_time_opt(&io_act.max_dur),
        }
    }

    pub fn new(
        x: f32,
        y: f32,
        act_type: u64,
        link_id: u64,
        start_time: Option<u32>,
        end_time: Option<u32>,
        max_dur: Option<u32>,
    ) -> Self {
        Activity {
            x,
            y,
            act_type,
            link_id,
            start_time,
            end_time,
            max_dur,
        }
    }

    fn cmp_end_time(&self, now: u32) -> u32 {
        if let Some(end_time) = self.end_time {
            end_time
        } else if let Some(max_dur) = self.max_dur {
            now + max_dur
        } else {
            // supposed to be an equivalent for OptionalTime.undefined() in the java code
            u32::MAX
        }
    }
}

impl Leg {
    fn from_io(io_leg: &IOLeg, person_id: &Id<Agent>, net: &Network, garage: &Garage) -> Self {
        let routing_mode_ext = Attrs::find_or_else_opt(&io_leg.attributes, "routingMode", || "car");

        let routing_mode = garage.modes.get_from_ext(routing_mode_ext);
        let mode = garage.modes.get_from_ext(io_leg.mode.as_str());
        let route = Route::from_io(&io_leg.route, person_id, &mode, net, garage);

        Self {
            route: Some(route),
            mode: mode.internal() as u64,
            trav_time: parse_time_opt(&io_leg.trav_time),
            dep_time: parse_time_opt(&io_leg.dep_time),
            routing_mode: routing_mode.internal() as u64,
        }
    }

    pub fn new(route: Route, mode: u64, trav_time: Option<u32>, dep_time: Option<u32>) -> Self {
        Self {
            route: Some(route),
            mode,
            trav_time,
            dep_time,
            routing_mode: 0,
        }
    }
}

impl Route {
    pub fn start_link(&self) -> u64 {
        *self.route.first().unwrap()
    }

    pub fn end_link(&self) -> u64 {
        *self.route.last().unwrap()
    }

    fn from_io(
        io_route: &IORoute,
        person_id: &Id<Agent>,
        mode: &Id<String>,
        net: &Network,
        garage: &Garage,
    ) -> Self {
        let route = match io_route.r#type.as_str() {
            "generic" => Self::from_io_generic(io_route, person_id, mode, net, garage),
            "links" => Self::from_io_net_route(io_route, person_id, mode, net, garage),
            _t => panic!("Unsupported route type: '{_t}'"),
        };

        route
    }

    fn from_io_generic(
        io_route: &IORoute,
        person_id: &Id<Agent>,
        mode: &Id<String>,
        net: &Network,
        garage: &Garage,
    ) -> Self {
        let start_link = net.link_ids.get_from_ext(&io_route.start_link);
        let end_link = net.link_ids.get_from_ext(&io_route.end_link);
        let veh_id: Id<Vehicle> = garage.get_veh_id(person_id, mode);

        Route {
            distance: io_route.distance,
            veh_id: veh_id.internal() as u64,
            route: vec![start_link.internal() as u64, end_link.internal() as u64],
        }
    }

    fn from_io_net_route(
        io_route: &IORoute,
        person_id: &Id<Agent>,
        mode: &Id<String>,
        net: &Network,
        garage: &Garage,
    ) -> Self {
        if let Some(veh_id_ext) = &io_route.vehicle {
            // catch this special case because we have "null" as vehicle ids for modes which are
            // routed but not simulated on the network.
            if veh_id_ext.eq("null") {
                Self::from_io_generic(io_route, person_id, mode, net, garage)
            } else {
                let veh_id = garage.vehicle_ids.get_from_ext(veh_id_ext.as_str());
                let link_ids = match &io_route.route {
                    None => Vec::new(),
                    Some(encoded_links) => encoded_links
                        .split(' ')
                        .map(|matsim_id| net.link_ids.get_from_ext(matsim_id).internal() as u64)
                        .collect(),
                };
                Route {
                    distance: io_route.distance,
                    veh_id: veh_id.internal() as u64,
                    route: link_ids,
                }
            }
        } else {
            panic!("vehicle id is expected to be set. ")
        }
    }
}

fn parse_time_opt(value: &Option<String>) -> Option<u32> {
    value.as_ref().map(|value| parse_time(value))
}

fn parse_time(value: &str) -> u32 {
    let split: Vec<&str> = value.split(':').collect();
    assert_eq!(3, split.len());

    let hour: u32 = split.first().unwrap().parse().unwrap();
    let minutes: u32 = split.get(1).unwrap().parse().unwrap();
    let seconds: u32 = split.get(2).unwrap().parse().unwrap();

    hour * 3600 + minutes * 60 + seconds
}
