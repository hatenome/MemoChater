//! 内置维度处理器

mod temporal;

pub use temporal::{compute_temporal_weight, format_time_diff, TemporalProcessor};