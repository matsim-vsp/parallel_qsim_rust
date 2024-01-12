use crate::simulation::wire_types::vehicles::LevelOfDetail;

impl TryFrom<i32> for LevelOfDetail {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            x if x == LevelOfDetail::Network as i32 => Ok(LevelOfDetail::Network),
            x if x == LevelOfDetail::Teleported as i32 => Ok(LevelOfDetail::Teleported),
            _ => Err(()),
        }
    }
}
