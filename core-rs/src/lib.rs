//! Scoot's portable judgment core — the Rust implementation of the contracts
//! in docs/CONTRACTS.md, conformant to the golden vectors in Tests/golden/.
//!
//! Mirrors Sources/ScootCore module-for-module. Pure by design: no clocks, no
//! timezones, no I/O in the reducers — shells feed signals in and render
//! decisions out. Any behavior change here must match a vector regeneration
//! from the Swift reference in the same PR (CONTRACTS.md change control).

pub mod collection;
pub mod experiments;
pub mod movement;
pub mod rng;
pub mod roll;
pub mod scheduler;
pub mod telemetry;
