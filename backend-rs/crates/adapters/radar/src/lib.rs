//! Radar source adapters: `dump1090` (HTTP polling) and `grpc` (streaming).
//! Both implementations expose the same domain `RadarSource` trait so the
//! application layer never branches on transport.

pub mod dump1090;
pub mod grpc;

/// Generated protobuf + gRPC client/server code for the `adsb` package.
/// `tonic-build` emits this into `OUT_DIR`; we include it from there so the
/// generated artefacts are never committed.
#[allow(
    clippy::pedantic,
    clippy::all,
    clippy::default_trait_access,
    clippy::too_many_lines,
    clippy::derive_partial_eq_without_eq,
    unreachable_pub,
    missing_debug_implementations
)]
pub mod proto {
    tonic::include_proto!("adsb");
}

pub use dump1090::{Dump1090Config, Dump1090Source};
pub use grpc::{GrpcAdsbConfig, GrpcAdsbSource};
