//! osquery integration in the root helper. Spawns osqueryd with
//! Endpoint Security publishers enabled (only legal as root), pipes
//! stdout to a subscriber task, normalizes rows to `Event`s, publishes
//! to the helper's internal bus.

mod config_gen;
mod normalize;
mod subscriber;
mod supervisor;

pub use supervisor::Supervisor;
