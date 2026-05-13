use crate::simulation::io::xml::transit;
use std::path::Path;
use tracing::info;

pub struct TransitSchedule {
    routes: Vec<String>,
    lines: Vec<String>,
}

impl TransitSchedule {
    pub fn from_file(file_path: &Path) -> Self {
        info!("Reading transit schedule and extract id for lines and routes");
        let schedule = transit::load_from_xml(file_path);
        info!(
            "Finished extracting transit ids. Found {} lines and {} routes.",
            schedule.lines.len(),
            schedule.routes.len()
        );
        schedule
    }

    pub(crate) fn new(routes: Vec<String>, lines: Vec<String>) -> TransitSchedule {
        TransitSchedule { routes, lines }
    }
}

#[cfg(test)]
mod tests {
    use crate::simulation::pt::TransitSchedule;

    #[test]
    fn test() {
        let schedule =
            TransitSchedule::from_file("./assets/pt_tutorial/transitschedule.xml".as_ref());

        assert_eq!(schedule.lines.len(), 1);
        assert_eq!(schedule.lines[0], "Blue Line");
        assert_eq!(schedule.routes.len(), 2);
        assert_eq!(schedule.routes[0], "1to3");
        assert_eq!(schedule.routes[1], "3to1");
    }
}
