use std::path::PathBuf;

pub mod attributes;
pub mod proto;
pub mod proto_events;
pub mod xml;

pub fn resolve_path(config: &String, file: &str) -> PathBuf {
    let file_path = PathBuf::from(file);
    if file_path.is_absolute() || file_path.starts_with("./") {
        return file_path;
    }

    let config_path = PathBuf::from(config);
    if let Some(path) = config_path.parent() {
        path.join(file_path)
    } else {
        file_path
    }
}

pub trait MatsimId {
    fn id(&self) -> &str;
}
