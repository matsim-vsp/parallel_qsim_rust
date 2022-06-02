use std::rc::Rc;

#[derive(PartialEq)]
#[derive(Clone)]
enum PlanElement {
    Leg { mode: String },
    // this apparently has more parameters, but let's start simple
    Activity { act_type: String },
}

impl PlanElement {
    fn is_same_variant(&self, other: &PlanElement) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

struct Person {
    id: String,
    plans: Vec<Plan>,
    selected_plan_index: Option<usize>,
}

struct Plan {
    is_selected: bool,
    elements: Vec<PlanElement>,
}

impl Plan {
    fn new() -> Plan {
        Plan { elements: Vec::new(), is_selected: false }
    }

    fn add_element(&mut self, element: PlanElement) {
        let last = self.elements.last();
        if last.is_some() && element.is_same_variant(last.unwrap()) {
            panic!("Activities and Legs mast be inserted in alternating fashion");
        } else {
            self.elements.push(element);
        }
    }
}

impl Person {
    fn new(id: String) -> Person {
        Person {
            id,
            plans: Vec::new(),
            selected_plan_index: None,
        }
    }

    fn add_plan(&mut self, plan: Plan) {
        self.plans.push(plan);
        self.selected_plan_index = Some(self.plans.len() - 1)
    }

    fn get_selected_plan(&self) -> &Plan {
        self.plans.get(self.selected_plan_index.expect("No selected plan yet")).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use crate::population::{Plan, PlanElement};
    use crate::population::PlanElement::{Activity, Leg};

    #[test]
    #[should_panic]
    fn add_plan_element_wrong_order() {
        let act1 = Activity { act_type: String::from("some-type") };
        let act2 = Activity { act_type: String::from("some-other-type") };
        let mut plan = Plan::new();

        plan.add_element(act1);
        plan.add_element(act2);
    }

    #[test]
    fn add_plan_elements_right_order() {
        let elements = vec![
            Activity { act_type: String::from("some-type") },
            Leg { mode: String::from("some-mode") },
            Activity { act_type: String::from("some-other-type") },
        ];
        let mut plan = Plan::new();

        for element in elements.to_vec() {
            plan.add_element(element);
        }

        elements.iter().zip(plan.elements.iter())
            .for_each(|(element, plan_element)| {
                assert!(element.is_same_variant(plan_element));
            });

        print!("end of test");
    }
}