use crate::simulation::id::Id;
use flate2::bufread::GzDecoder;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use tracing::info;

pub struct TransitSchedule {
    routes: Vec<String>,
    lines: Vec<String>,
}

impl TransitSchedule {
    pub fn from_file(file_path: &Path) -> Self {
        info!("Reading transit schedule and extract id for lines and routes");
        let schedule = Self::create_schedule(file_path);
        Self::create_ids(&schedule);
        schedule
    }

    fn create_schedule(file_path: &Path) -> TransitSchedule {
        let mut reader = if file_path.extension().unwrap() == "gz" {
            let file = File::open(file_path)
                .unwrap_or_else(|_| panic!("Could not open file at {}", file_path.display()));
            let buf_reader = BufReader::new(file);
            let decoder: Box<dyn Read> = Box::new(GzDecoder::new(buf_reader));
            let gz_reader = BufReader::new(decoder);
            Reader::from_reader(gz_reader)
        } else {
            let file: Box<dyn Read> = Box::new(File::open(file_path).unwrap());
            let buf_reader = BufReader::new(file);
            Reader::from_reader(buf_reader)
        };

        let mut lines = Vec::new();
        let mut routes = Vec::new();

        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Err(e) => {
                    panic!("Error at position {}: {:?}", reader.error_position(), e)
                }
                Ok(Event::Eof) => break,
                Ok(Event::Start(e)) => match e.name().as_ref() {
                    b"transitLine" => {
                        Self::find_id(&mut lines, e);
                    }
                    b"transitRoute" => {
                        Self::find_id(&mut routes, e);
                    }
                    _ => {}
                },
                Ok(_) => {}
            }
        }

        TransitSchedule { routes, lines }
    }

    fn find_id(lines: &mut Vec<String>, e: BytesStart) {
        'attr: for attr in e.attributes().flatten() {
            if attr.key.as_ref() == b"id" {
                let id_value = attr.unescape_value().unwrap();
                // println!("id = {}", id_value);
                lines.push(id_value.to_string());
                break 'attr;
            }
        }
    }

    fn create_ids(schedule: &TransitSchedule) {
        for line in &schedule.lines {
            Id::<String>::create(line);
        }

        for route in &schedule.routes {
            Id::<String>::create(route);
        }
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
