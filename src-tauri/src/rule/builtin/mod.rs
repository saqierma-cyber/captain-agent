//! Built-in rule pack — compiled into the binary via `include_str!`.
//! Populated starting Slice 2.

pub const CREDENTIALS_YAML: &str = include_str!("credentials.yaml");
pub const PERSISTENCE_YAML: &str = include_str!("persistence.yaml");
pub const COMMANDS_YAML: &str = include_str!("commands.yaml");
pub const NETWORK_YAML: &str = include_str!("network.yaml");
pub const CORRELATIONS_YAML: &str = include_str!("correlations.yaml");
pub const METRICS_YAML: &str = include_str!("metrics.yaml");
