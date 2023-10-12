use crate::simulation::routing::router::CustomQueryResult;

pub struct AltRouter {}

impl AltRouter {
    pub fn new() -> Self {
        AltRouter {}
    }

    pub(crate) fn query(
        &mut self,
        from: usize,
        to: usize,
    ) -> CustomQueryResult {
        CustomQueryResult::new()
    }

    fn perform_preprocessing(&mut self) {}

    pub fn query_links(&mut self, from_link: u64, to_link: u64) -> CustomQueryResult {
        CustomQueryResult::new()
    }

    pub fn customize(&mut self) {}
}