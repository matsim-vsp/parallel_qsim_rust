use crate::simulation::events::{EventTrait, GeneralEvent};
use crate::simulation::id::Id;
use crate::simulation::scenario::population::InternalPerson;


pub struct BackpackPlan {
    //TODO
}

impl BackpackPlan {
    pub fn new() -> Self {
        Self {}
    }

    // TODO
}

/// Backpacks store the Events as well as a partial plan ([BackpackPlan]) for each agent.
/// The Backpack is not managed by the agent itself but by the [BackpackDataCollector], which exists
/// once for each partition. If an agent leaves the current partition, the Backpack is transmitted
/// to the partition the agent is currently entering.
pub struct Backpack{
    person_id: Id<InternalPerson>,
    events: Vec<Box<dyn EventTrait>>,
    backpack_plan: BackpackPlan,
    starting_partition: i32
}

impl Backpack {
    pub fn new(person_id: Id<InternalPerson>, starting_partition: i32) -> Self {
        Self {
            person_id,
            events: Default::default(),
            backpack_plan: BackpackPlan::new(),
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