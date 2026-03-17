use crate::simulation::scenario::population::{InternalActivity, InternalPlanElement};
use tracing::error;

pub fn identify_main_mode(trip_elements: &[InternalPlanElement]) -> Option<String> {
    // Try to get the routing mode from the first leg
    let mut mode: Option<String> =
        trip_elements
            .first()
            .and_then(|el| el.as_leg())
            .and_then(|leg| {
                leg.routing_mode
                    .as_ref()
                    .map(|id| id.external().to_string())
            });

    // If not found and only one element, use the mode of that leg
    if mode.is_none() && trip_elements.len() == 1 {
        mode = trip_elements
            .first()
            .and_then(|el| el.as_leg())
            .map(|leg| leg.mode.external().to_string());
    }

    if mode.is_none() {
        error!("Could not find routing mode for trip {:?}", trip_elements);
    }

    mode
}

/// A trip is a sequence of plan elements between two non-stage activities.
#[derive(Debug, PartialEq)]
pub struct Trip<'a> {
    pub origin: &'a InternalActivity,
    pub legs: &'a [InternalPlanElement],
    pub destination: &'a InternalActivity,
}

/// Identifies a trip inside a flat plan by its boundary activities.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct TripSpan {
    pub origin_index: usize,
    pub destination_index: usize,
}

impl TripSpan {
    pub fn origin<'a>(&self, plan_elements: &'a [InternalPlanElement]) -> &'a InternalActivity {
        activity_at(plan_elements, self.origin_index)
    }

    pub fn destination<'a>(
        &self,
        plan_elements: &'a [InternalPlanElement],
    ) -> &'a InternalActivity {
        activity_at(plan_elements, self.destination_index)
    }

    pub fn trip_elements<'a>(
        &self,
        plan_elements: &'a [InternalPlanElement],
    ) -> &'a [InternalPlanElement] {
        &plan_elements[self.origin_index + 1..self.destination_index]
    }

    pub fn origin_mut<'a>(
        &self,
        plan_elements: &'a mut [InternalPlanElement],
    ) -> &'a mut InternalActivity {
        activity_at_mut(plan_elements, self.origin_index)
    }

    pub fn destination_mut<'a>(
        &self,
        plan_elements: &'a mut [InternalPlanElement],
    ) -> &'a mut InternalActivity {
        activity_at_mut(plan_elements, self.destination_index)
    }

    pub fn trip_elements_mut<'a>(
        &self,
        plan_elements: &'a mut [InternalPlanElement],
    ) -> &'a mut [InternalPlanElement] {
        &mut plan_elements[self.origin_index + 1..self.destination_index]
    }

    pub fn replace_trip_elements(
        &self,
        plan_elements: &mut Vec<InternalPlanElement>,
        new_elements: impl IntoIterator<Item = InternalPlanElement>,
    ) {
        plan_elements.splice(self.origin_index + 1..self.destination_index, new_elements);
    }
}

fn activity_at(plan_elements: &[InternalPlanElement], index: usize) -> &InternalActivity {
    plan_elements[index]
        .as_activity()
        .expect("Trip boundary must be an activity")
}

fn activity_at_mut(
    plan_elements: &mut [InternalPlanElement],
    index: usize,
) -> &mut InternalActivity {
    match &mut plan_elements[index] {
        InternalPlanElement::Activity(activity) => activity,
        InternalPlanElement::Leg(_) => panic!("Trip boundary must be an activity"),
    }
}

fn trip_spans<F>(plan_elements: &[InternalPlanElement], mut is_stage_activity: F) -> Vec<TripSpan>
where
    F: FnMut(&InternalActivity) -> bool,
{
    let mut trips = Vec::new();
    let mut origin_activity_index: isize = -1;
    let mut current_index: isize = -1;

    for pe in plan_elements.iter() {
        current_index += 1;
        let act = match pe.as_activity() {
            Some(a) => a,
            None => continue,
        };
        if is_stage_activity(act) {
            continue;
        }

        if origin_activity_index == -1 {
            origin_activity_index = current_index;
            continue;
        }

        if current_index - origin_activity_index > 1 {
            trips.push(TripSpan {
                origin_index: origin_activity_index as usize,
                destination_index: current_index as usize,
            });
        }
        origin_activity_index = current_index;
    }

    trips
}

/// Extracts trips from a plan, using is_stage_activity to identify stage activities.
pub fn get_trips<F>(plan_elements: &[InternalPlanElement], mut is_stage_activity: F) -> Vec<Trip>
where
    F: Fn(&InternalActivity) -> bool,
{
    trip_spans(plan_elements, &mut is_stage_activity)
        .into_iter()
        .map(|span| Trip {
            origin: span.origin(plan_elements),
            legs: span.trip_elements(plan_elements),
            destination: span.destination(plan_elements),
        })
        .collect()
}

/// Extracts trips from a plan, using InternalActivity::is_interaction as the default stage activity detector.
pub fn get_trips_default(plan_elements: &[InternalPlanElement]) -> Vec<Trip> {
    get_trips(plan_elements, |a| a.is_interaction())
}

pub fn get_trip_spans<F>(
    plan_elements: &[InternalPlanElement],
    is_stage_activity: F,
) -> Vec<TripSpan>
where
    F: Fn(&InternalActivity) -> bool,
{
    trip_spans(plan_elements, is_stage_activity)
}

pub fn get_trip_spans_default(plan_elements: &[InternalPlanElement]) -> Vec<TripSpan> {
    get_trip_spans(plan_elements, |a| a.is_interaction())
}

/// Finds the next trip starting at the given activity index in the plan.
/// Returns Some(Trip) if a trip is found, or None if there are no more trips.
pub fn find_trip_starting_at_activity<F>(
    plan_elements: &[InternalPlanElement],
    start_index: usize,
    mut is_stage_activity: F,
) -> Option<Trip>
where
    F: Fn(&InternalActivity) -> bool,
{
    find_trip_span_starting_at_activity(plan_elements, start_index, &mut is_stage_activity).map(
        |span| Trip {
            origin: span.origin(plan_elements),
            legs: span.trip_elements(plan_elements),
            destination: span.destination(plan_elements),
        },
    )
}

pub fn find_trip_starting_at_activity_default(
    plan_elements: &[InternalPlanElement],
    start_index: usize,
) -> Option<Trip> {
    find_trip_starting_at_activity(plan_elements, start_index, |a| a.is_interaction())
}

pub fn find_trip_span_starting_at_activity<F>(
    plan_elements: &[InternalPlanElement],
    start_index: usize,
    mut is_stage_activity: F,
) -> Option<TripSpan>
where
    F: FnMut(&InternalActivity) -> bool,
{
    trip_spans(&plan_elements[start_index..], &mut is_stage_activity)
        .into_iter()
        .next()
        .map(|span| TripSpan {
            origin_index: start_index + span.origin_index,
            destination_index: start_index + span.destination_index,
        })
}

pub fn find_trip_span_starting_at_activity_default(
    plan_elements: &[InternalPlanElement],
    start_index: usize,
) -> Option<TripSpan> {
    find_trip_span_starting_at_activity(plan_elements, start_index, |a| a.is_interaction())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::id::Id;
    use crate::simulation::scenario::population::{InternalLeg, InternalPlanElement};
    use macros::integration_test;

    #[integration_test]
    fn test_identify_main_mode_routing_mode() {
        let leg = InternalLeg {
            mode: Id::create("car"),
            routing_mode: Some(Id::create("car")),
            dep_time: None,
            trav_time: None,
            route: None,
            attributes: Default::default(),
        };
        let trip = vec![InternalPlanElement::Leg(leg)];
        let mode = identify_main_mode(&trip);
        assert_eq!(mode, Some("car".to_string()));
    }

    #[integration_test]
    fn test_identify_main_mode_single_leg_mode() {
        let leg = InternalLeg {
            mode: Id::create("bike"),
            routing_mode: None,
            dep_time: None,
            trav_time: None,
            route: None,
            attributes: Default::default(),
        };
        let trip = vec![InternalPlanElement::Leg(leg)];
        let mode = identify_main_mode(&trip);
        assert_eq!(mode, Some("bike".to_string()));
    }

    #[integration_test]
    fn test_identify_main_mode_no_mode() {
        let trip = vec![];
        let mode = identify_main_mode(&trip);
        assert_eq!(mode, None);
    }

    fn make_activity(act_type: &str, link: &str) -> InternalPlanElement {
        InternalPlanElement::Activity(InternalActivity {
            act_type: Id::create(act_type),
            link_id: Id::create(link),
            x: 0.0,
            y: 0.0,
            start_time: None,
            end_time: None,
            max_dur: None,
            attributes: Default::default(),
        })
    }

    fn make_leg(mode: &str) -> InternalPlanElement {
        InternalPlanElement::Leg(InternalLeg {
            mode: Id::create(mode),
            routing_mode: Some(Id::create(mode)),
            dep_time: None,
            trav_time: None,
            route: None,
            attributes: Default::default(),
        })
    }

    #[integration_test]
    fn test_get_trips_basic() {
        let plan = vec![
            make_activity("home", "1"),
            make_leg("car"),
            make_activity("work", "2"),
            make_leg("walk"),
            make_activity("shop", "3"),
        ];
        let trips = get_trips_default(&plan);
        assert_eq!(trips.len(), 2);
        assert_eq!(trips[0].origin.act_type.external(), "home");
        assert_eq!(trips[0].destination.act_type.external(), "work");
        assert_eq!(trips[0].legs.len(), 1);
        assert_eq!(trips[1].origin.act_type.external(), "work");
        assert_eq!(trips[1].destination.act_type.external(), "shop");
        assert_eq!(trips[1].legs.len(), 1);
    }

    #[integration_test]
    fn test_get_trips_with_stage_activity() {
        let plan = vec![
            make_activity("home", "1"),
            make_leg("car"),
            make_activity("car interaction", "2"),
            make_leg("car"),
            make_activity("work", "3"),
        ];
        let trips = get_trips_default(&plan);
        assert_eq!(trips.len(), 1);
        assert_eq!(trips[0].origin.act_type.external(), "home");
        assert_eq!(trips[0].destination.act_type.external(), "work");
        assert_eq!(trips[0].legs.len(), 3);
    }

    #[integration_test]
    fn test_get_trips_no_trips() {
        let plan = vec![make_activity("home", "1"), make_activity("work", "2")];
        let trips = get_trips_default(&plan);
        assert!(trips.is_empty());
    }

    #[integration_test]
    fn test_get_trips_empty() {
        let plan: Vec<InternalPlanElement> = vec![];
        let trips = get_trips_default(&plan);
        assert!(trips.is_empty());
    }

    #[integration_test]
    fn test_get_trip_spans_default_basic() {
        let plan = vec![
            make_activity("home", "1"),
            make_leg("car"),
            make_activity("work", "2"),
            make_leg("walk"),
            make_activity("shop", "3"),
        ];

        let spans = get_trip_spans_default(&plan);
        assert_eq!(spans.len(), 2);
        assert_eq!(
            spans,
            vec![
                TripSpan {
                    origin_index: 0,
                    destination_index: 2,
                },
                TripSpan {
                    origin_index: 2,
                    destination_index: 4,
                },
            ]
        );
        assert_eq!(spans[0].origin(&plan).act_type.external(), "home");
        assert_eq!(spans[1].destination(&plan).act_type.external(), "shop");
    }

    #[integration_test]
    fn test_trip_span_mutation_helpers() {
        let mut plan = vec![
            make_activity("home", "1"),
            make_leg("car"),
            make_activity("work", "2"),
            make_leg("walk"),
            make_activity("shop", "3"),
        ];

        let span = find_trip_span_starting_at_activity_default(&plan, 2).unwrap();
        span.origin_mut(&mut plan).act_type = Id::create("office");
        span.trip_elements_mut(&mut plan)[0] = make_leg("bike");
        span.destination_mut(&mut plan).act_type = Id::create("mall");

        assert_eq!(plan[2].as_activity().unwrap().act_type.external(), "office");
        assert_eq!(plan[3].as_leg().unwrap().mode.external(), "bike");
        assert_eq!(plan[4].as_activity().unwrap().act_type.external(), "mall");
    }

    #[integration_test]
    fn test_trip_span_replace_middle() {
        let mut plan = vec![
            make_activity("home", "1"),
            make_leg("car"),
            make_activity("work", "2"),
            make_leg("walk"),
            make_activity("shop", "3"),
        ];

        let span = find_trip_span_starting_at_activity_default(&plan, 0).unwrap();
        span.replace_trip_elements(
            &mut plan,
            vec![
                make_leg("pt"),
                make_activity("pt interaction", "99"),
                make_leg("pt"),
            ],
        );

        assert_eq!(plan.len(), 7);
        assert_eq!(plan[0].as_activity().unwrap().act_type.external(), "home");
        assert_eq!(plan[1].as_leg().unwrap().mode.external(), "pt");
        assert_eq!(
            plan[2].as_activity().unwrap().act_type.external(),
            "pt interaction"
        );
        assert_eq!(plan[3].as_leg().unwrap().mode.external(), "pt");
        assert_eq!(plan[4].as_activity().unwrap().act_type.external(), "work");
    }

    #[integration_test]
    fn test_find_trip_starting_at_activity_default_basic() {
        let plan = vec![
            make_activity("home", "1"),
            make_leg("car"),
            make_activity("work", "2"),
            make_leg("walk"),
            make_activity("shop", "3"),
        ];
        let trip = find_trip_starting_at_activity_default(&plan, 0).unwrap();
        assert_eq!(trip.origin.act_type.external(), "home");
        assert_eq!(trip.destination.act_type.external(), "work");
        assert_eq!(trip.legs.len(), 1);

        let trip2 = find_trip_starting_at_activity_default(&plan, 2).unwrap();
        assert_eq!(trip2.origin.act_type.external(), "work");
        assert_eq!(trip2.destination.act_type.external(), "shop");
        assert_eq!(trip2.legs.len(), 1);

        assert!(find_trip_starting_at_activity_default(&plan, 4).is_none());
    }

    #[integration_test]
    fn test_find_trip_starting_at_activity_default_with_stage() {
        let plan = vec![
            make_activity("home", "1"),
            make_leg("car"),
            make_activity("car interaction", "2"),
            make_leg("car"),
            make_activity("work", "3"),
        ];
        let trip = find_trip_starting_at_activity_default(&plan, 0).unwrap();
        assert_eq!(trip.origin.act_type.external(), "home");
        assert_eq!(trip.destination.act_type.external(), "work");
        assert_eq!(trip.legs.len(), 3);
        assert!(find_trip_starting_at_activity_default(&plan, 2).is_none());
    }

    #[integration_test]
    fn test_find_trip_starting_at_activity_default_empty() {
        let plan: Vec<InternalPlanElement> = vec![];
        assert!(find_trip_starting_at_activity_default(&plan, 0).is_none());
    }

    #[integration_test]
    fn test_find_trip_span_starting_at_activity_default_basic() {
        let plan = vec![
            make_activity("home", "1"),
            make_leg("car"),
            make_activity("work", "2"),
            make_leg("walk"),
            make_activity("shop", "3"),
        ];

        let span = find_trip_span_starting_at_activity_default(&plan, 2).unwrap();
        assert_eq!(
            span,
            TripSpan {
                origin_index: 2,
                destination_index: 4,
            }
        );
        assert_eq!(span.origin(&plan).act_type.external(), "work");
        assert_eq!(span.destination(&plan).act_type.external(), "shop");
    }
}
