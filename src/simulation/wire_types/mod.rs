// Include the `messages` module, which is generated from messages.proto
pub mod events {
    include!(concat!(env!("OUT_DIR"), "/events.rs"));
}
pub mod ids {
    include!(concat!(env!("OUT_DIR"), "/ids.rs"));
}
pub mod messages {
    include!(concat!(env!("OUT_DIR"), "/messages.rs"));
}

pub mod network {
    include!(concat!(env!("OUT_DIR"), "/network.rs"));
}
pub mod population {
    include!(concat!(env!("OUT_DIR"), "/population.rs"));
}
pub mod vehicles {
    include!(concat!(env!("OUT_DIR"), "/vehicles.rs"));
}

pub mod general {
    include!(concat!(env!("OUT_DIR"), "/general.rs"));
}
