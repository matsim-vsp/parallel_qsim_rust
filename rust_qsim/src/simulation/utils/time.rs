pub fn parse_time_opt(value: &Option<String>) -> Option<u32> {
    if let Some(time) = value.as_ref() {
        parse_time(time)
    } else {
        None
    }
}

pub fn parse_time(value: &str) -> Option<u32> {
    let split: Vec<&str> = value.split(':').collect();
    if split.len() == 3 {
        let hour: u32 = split.first().unwrap().parse().unwrap();
        let minutes: u32 = split.get(1).unwrap().parse().unwrap();
        let seconds: u32 = split.get(2).unwrap().parse().unwrap();

        Some(hour * 3600 + minutes * 60 + seconds)
    } else {
        None
    }
}

/// create a string "hh:mm:ss" from a given number of seconds
pub fn write_timestr(time_secs: u32) -> String {
    let hours = time_secs / 3600; // rounds towards zero, i.e., floors the result
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}
