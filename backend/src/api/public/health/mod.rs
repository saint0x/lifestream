use super::*;

mod checks;
mod metrics;

#[cfg(test)]
pub(crate) use checks::{
    check_binary_available, check_media_root_writable, check_runtime_dependencies_with_binaries,
};
pub(crate) use checks::{health, health_live, health_ready};
pub(crate) use metrics::metrics;

#[derive(Clone, Debug)]
pub(crate) struct RuntimeHealthStatus {
    pub(crate) ready: bool,
    pub(crate) database: bool,
    pub(crate) dependencies: HealthDependencies,
}
