use crate::io::matsim_id::MatsimId;
use crate::io::population::{IOActivity, IOLeg, IOPerson, IOPlan, IOPlanElement, IORoute};
use crate::mpi::messages::proto::leg::Route;
use crate::mpi::messages::proto::{
    Activity, Agent, ExperimentalMessage, GenericRoute, Leg, NetworkRoute, Plan, Vehicle,
    VehicleMessage, VehicleType,
};
use crate::mpi::time_queue::EndTime;
use crate::parallel_simulation::id_mapping::{MatsimIdMapping, MatsimIdMappings};
use crate::parallel_simulation::network::node::NodeVehicle;
use prost::Message;
use std::cmp::Ordering;
use std::io::Cursor;

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
            from_process: from as u32,
            to_process: to as u32,
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
    pub fn new(id: u64, veh_type: VehicleType, agent: Agent) -> Vehicle {
        Vehicle {
            id,
            agent: Some(agent),
            curr_route_elem: 0,
            r#type: veh_type as i32,
        }
    }

    fn agent(&self) -> &Agent {
        self.agent.as_ref().unwrap()
    }
}

impl NodeVehicle for Vehicle {
    fn id(&self) -> usize {
        self.id as usize
    }

    fn advance_route_index(&mut self) {
        self.curr_route_elem += 1;
    }

    fn curr_link_id(&self) -> Option<usize> {
        let leg = self.agent().curr_leg();
        let route = leg.route.as_ref().unwrap();
        match route {
            Route::GenericRoute(route) => Some(route.end_link as usize),
            Route::NetworkRoute(route) => {
                let index = self.curr_route_elem as usize;
                route.route.get(index).map(|id| *id as usize)
            }
        }
    }
}

impl EndTime for Vehicle {
    fn end_time(&self, now: u32) -> u32 {
        self.agent().end_time(now)
    }
}

impl Agent {
    pub fn from_io(io_person: &IOPerson, id_mappings: &MatsimIdMappings) -> Agent {
        let plan = Plan::from_io(io_person.selected_plan(), id_mappings);
        let id = *id_mappings.agents.get_internal(io_person.id()).unwrap();
        Agent {
            id: id as u64,
            plan: Some(plan),
            curr_plan_elem: 0,
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
        self.plan
            .as_ref()
            .unwrap()
            .acts
            .get(act_index as usize)
            .unwrap()
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
            let route = self.curr_leg().route.as_ref().unwrap();
            match route {
                Route::GenericRoute(gen_route) => now + gen_route.trav_time,
                Route::NetworkRoute(_) => {
                    panic!("End time not supported for network route")
                }
            }
        };
    }
}

impl Plan {
    fn new() -> Plan {
        Plan {
            acts: Vec::new(),
            legs: Vec::new(),
        }
    }

    fn from_io(io_plan: &IOPlan, id_mappings: &MatsimIdMappings) -> Plan {
        assert!(!io_plan.elements.is_empty());
        if let IOPlanElement::Leg(_leg) = io_plan.elements.get(0).unwrap() {
            panic!("First plan element must be an activity! But was a leg.");
        }

        let mut result = Plan::new();

        for element in &io_plan.elements {
            match element {
                IOPlanElement::Activity(io_act) => {
                    let act = Activity::from_io(io_act, &id_mappings.links);
                    result.acts.push(act);
                }
                IOPlanElement::Leg(io_leg) => {
                    let leg = Leg::from_io(io_leg, id_mappings);
                    result.legs.push(leg);
                }
            }
        }

        result
    }
}

impl Activity {
    fn from_io(io_act: &IOActivity, link_id_mapping: &MatsimIdMapping) -> Self {
        let link_id = *link_id_mapping.get_internal(io_act.link.as_str()).unwrap();
        Activity {
            x: io_act.x,
            y: io_act.y,
            act_type: io_act.r#type.clone(),
            link_id: link_id as u64,
            start_time: parse_time_opt(&io_act.start_time),
            end_time: parse_time_opt(&io_act.end_time),
            max_dur: parse_time_opt(&io_act.max_dur),
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
    fn from_io(io_leg: &IOLeg, id_mappings: &MatsimIdMappings) -> Self {
        let route = Route::from_io(&io_leg.route, id_mappings);
        Self {
            route: Some(route),
            mode: io_leg.mode.clone(),
            trav_time: parse_time_opt(&io_leg.trav_time),
            dep_time: parse_time_opt(&io_leg.dep_time),
        }
    }
}

impl Route {
    fn from_io(io_route: &IORoute, id_mappings: &MatsimIdMappings) -> Self {
        match io_route.r#type.as_str() {
            "generic" => Route::GenericRoute(GenericRoute::from_io(io_route, &id_mappings.links)),
            "links" => Route::NetworkRoute(NetworkRoute::from_io(io_route, id_mappings)),
            _t => panic!("Unsupported route type: '{_t}'"),
        }
    }
}

impl GenericRoute {
    fn from_io(io_route: &IORoute, link_id_mapping: &MatsimIdMapping) -> Self {
        let start_link = *link_id_mapping
            .get_internal(io_route.start_link.as_str())
            .unwrap();
        let end_link = *link_id_mapping
            .get_internal(io_route.end_link.as_str())
            .unwrap();
        let trav_time = parse_time_opt(&io_route.trav_time).unwrap();

        Self {
            start_link: start_link as u64,
            end_link: end_link as u64,
            trav_time,
            distance: io_route.distance,
        }
    }
}

impl NetworkRoute {
    fn from_io(io_route: &IORoute, id_mappings: &MatsimIdMappings) -> Self {
        let matsim_veh_id = io_route.vehicle.as_ref().unwrap();
        let veh_id = id_mappings
            .vehicles
            .get_internal(matsim_veh_id.as_str())
            .unwrap();
        let link_ids = match &io_route.route {
            None => Vec::new(),
            Some(encoded_links) => encoded_links
                .split(' ')
                .map(|matsim_id| *id_mappings.links.get_internal(matsim_id).unwrap() as u64)
                .collect(),
        };
        Self {
            route: link_ids,
            vehicle_id: *veh_id as u64,
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
