use super::{
    config::Config,
    instance::Instance,
    ports::PortPool,
};
use std::collections::HashMap;

pub struct AppState {
    pub config: Config,
    pub port_pool: PortPool,
    pub instances: HashMap<String, Instance>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let port_pool = PortPool::new(
            config.ports.rdp_start..config.ports.rdp_end,
            config.ports.console_start..config.ports.console_end,
        );

        Self {
            config,
            port_pool,
            instances: HashMap::new(),
        }
    }
}
