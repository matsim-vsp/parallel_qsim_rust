use crate::simulation::population::{InternalActivity, InternalPlanElement};
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

/// Extracts trips from a plan, using is_stage_activity to identify stage activities.
pub fn get_trips<F>(plan_elements: &[InternalPlanElement], mut is_stage_activity: F) -> Vec<Trip>
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
        // Use the act_type for stage activity detection
        if is_stage_activity(act) {
            continue;
        }

        if origin_activity_index == -1 {
            // This is the first "full" activity we see, set it as origin. Continue.
            origin_activity_index = current_index;
            continue;
        }

        // It could be the case that we started inside a trip.
        // In this case, current_index = origin_activity_index, thus the following condition is false.
        if current_index - origin_activity_index > 1 {
            // There is at least one leg between activities
            let origin = plan_elements[origin_activity_index as usize]
                .as_activity()
                .unwrap();
            let legs = &plan_elements[(origin_activity_index + 1) as usize..current_index as usize];
            let destination = act;
            trips.push(Trip {
                origin,
                legs,
                destination,
            });
        }
        origin_activity_index = current_index;
    }
    trips
}

/// Extracts trips from a plan, using InternalActivity::is_interaction as the default stage activity detector.
pub fn get_trips_default(plan_elements: &[InternalPlanElement]) -> Vec<Trip> {
    get_trips(plan_elements, |a| a.is_interaction())
}

/// Finds the next trip starting at the given activity index in the plan.
/// Returns Some(Trip) if a trip is found, or None if there are no more trips.
pub fn find_trip_starting_at_activity<F>(
    plan_elements: &[InternalPlanElement],
    start_index: usize,
    mut is_stage_activity: F,
) -> Option<Trip>
where
    F: FnMut(&InternalActivity) -> bool,
{
    let trips = get_trips(&plan_elements[start_index..], &mut is_stage_activity);
    trips.into_iter().next()
}

pub fn find_trip_starting_at_activity_default(
    plan_elements: &[InternalPlanElement],
    start_index: usize,
) -> Option<Trip> {
    find_trip_starting_at_activity(plan_elements, start_index, |a| a.is_interaction())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::simulation::id::Id;
    use crate::simulation::population::{InternalLeg, InternalPlanElement};

    #[test]
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

    #[test]
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

    #[test]
    fn test_identify_main_mode_no_mode() {
        // Not a leg, so should return None and log error
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

    #[test]
    fn test_get_trips_basic() {
        // home --leg1--> work --leg2--> shop
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

    #[test]
    fn test_get_trips_with_stage_activity() {
        // home --leg1--> car interaction (stage) --leg2--> work
        let plan = vec![
            make_activity("home", "1"),
            make_leg("car"),
            make_activity("car interaction", "2"),
            make_leg("car"),
            make_activity("work", "3"),
        ];
        // Mark activities containing "interaction" as stage
        let trips = get_trips_default(&plan);
        assert_eq!(trips.len(), 1);
        assert_eq!(trips[0].origin.act_type.external(), "home");
        assert_eq!(trips[0].destination.act_type.external(), "work");
        assert_eq!(trips[0].legs.len(), 3); // both legs and the stage activity are included
    }

    #[test]
    fn test_get_trips_no_trips() {
        // Only activities, no legs
        let plan = vec![make_activity("home", "1"), make_activity("work", "2")];
        let trips = get_trips_default(&plan);
        assert!(trips.is_empty());
    }

    #[test]
    fn test_get_trips_empty() {
        let plan: Vec<InternalPlanElement> = vec![];
        let trips = get_trips_default(&plan);
        assert!(trips.is_empty());
    }

    #[test]
    fn test_find_trip_starting_at_activity_default_basic() {
        // home --leg1--> work --leg2--> shop
        let plan = vec![
            make_activity("home", "1"),
            make_leg("car"),
            make_activity("work", "2"),
            make_leg("walk"),
            make_activity("shop", "3"),
        ];
        // Start at index 0 (home)
        let trip = find_trip_starting_at_activity_default(&plan, 0).unwrap();
        assert_eq!(trip.origin.act_type.external(), "home");
        assert_eq!(trip.destination.act_type.external(), "work");
        assert_eq!(trip.legs.len(), 1);
        // Start at index 2 (work)
        let trip2 = find_trip_starting_at_activity_default(&plan, 2).unwrap();
        assert_eq!(trip2.origin.act_type.external(), "work");
        assert_eq!(trip2.destination.act_type.external(), "shop");
        assert_eq!(trip2.legs.len(), 1);
        // Start at index 4 (shop, last activity, should return None)
        assert!(find_trip_starting_at_activity_default(&plan, 4).is_none());
    }

    #[test]
    fn test_find_trip_starting_at_activity_default_with_stage() {
        // home --leg1--> car interaction (stage) --leg2--> work
        let plan = vec![
            make_activity("home", "1"),
            make_leg("car"),
            make_activity("car interaction", "2"),
            make_leg("car"),
            make_activity("work", "3"),
        ];
        // Start at index 0 (home)
        let trip = find_trip_starting_at_activity_default(&plan, 0).unwrap();
        assert_eq!(trip.origin.act_type.external(), "home");
        assert_eq!(trip.destination.act_type.external(), "work");
        assert_eq!(trip.legs.len(), 3);
        // Start at index 2 (car interaction, which is a stage activity, should skip to next trip)
        let trip2 = find_trip_starting_at_activity_default(&plan, 2);
        assert!(trip2.is_none());
    }

    #[test]
    fn test_find_trip_starting_at_activity_default_empty() {
        let plan: Vec<InternalPlanElement> = vec![];
        assert!(find_trip_starting_at_activity_default(&plan, 0).is_none());
    }
}
