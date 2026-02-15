use crate::simulation::config::{
    ComputationalSetup, Drt, Output, Partitioning, Routing, Simulation,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SimpleConfig {
    pub modules: Modules,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Modules {
    #[serde(default)]
    pub network: Option<PathBuf>,
    #[serde(default)]
    pub population: Option<PathBuf>,
    #[serde(default)]
    pub vehicles: Option<PathBuf>,
    #[serde(default)]
    pub ids: Option<PathBuf>,
    #[serde(default)]
    pub partitioning: Option<Partitioning>,
    #[serde(default)]
    pub output: Option<Output>,
    #[serde(default)]
    pub routing: Option<Routing>,
    #[serde(default)]
    pub simulation: Option<Simulation>,
    #[serde(default)]
    pub computational_setup: Option<ComputationalSetup>,
    #[serde(default)]
    pub drt: Option<Drt>,
}
