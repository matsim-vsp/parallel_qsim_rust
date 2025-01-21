use crate::simulation::wire_types::population::Person;

trait ActivityReplanningStrategy {
    fn is_responsible(&self, agent: &Person) -> bool;
    fn replan(&mut self, agent: &Person);
}

struct DrtPassengerReplanning {
    // some fields
}

impl ActivityReplanningStrategy for DrtPassengerReplanning {
    fn is_responsible(&self, _agent: &Person) -> bool {
        todo!()
    }

    fn replan(&mut self, _agent: &Person) {
        todo!()
    }
}

struct DrtDriverReplanning {
    // some fields
}

impl ActivityReplanningStrategy for DrtDriverReplanning {
    fn is_responsible(&self, _agent: &Person) -> bool {
        todo!()
    }

    fn replan(&mut self, _agent: &Person) {
        todo!()
    }
}
