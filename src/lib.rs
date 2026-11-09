// keep-sorted start
pub mod app;
pub mod cli;
pub mod config;
pub mod dsl;
pub mod pipeline;
pub mod state;
// keep-sorted end

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
