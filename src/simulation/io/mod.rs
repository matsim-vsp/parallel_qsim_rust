use std::path::PathBuf;

pub mod proto;
pub mod proto_events;
pub mod xml;

pub fn resolve_path(config_path: &Option<PathBuf>, file_path: &PathBuf) -> PathBuf {
    // This is a bit hacky, but tests rely on that. Paul, jul'25
    if file_path.is_absolute() || file_path.starts_with("./") {
        return file_path.clone();
    }

    if let Some(path) = config_path.as_ref().and_then(|c| c.parent()) {
        path.join(file_path)
    } else {
        file_path.clone()
    }
}
