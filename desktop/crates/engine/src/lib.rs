//! Receiver engine: UDP -> decrypt -> jitter buffer -> decode -> voice chain -> sink.

pub mod codec;
pub mod engine;
pub mod os;
pub mod sink;

pub use engine::{DenoiseKind, Engine, EngineConfig, Snapshot};
pub use sink::{CpalSink, MonitorGate, MonitorSink, NullSink, Sink, TeeSink, WavSink};
