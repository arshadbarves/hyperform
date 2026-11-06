//! SystemOne (laya-serve, Jev): endpoint rules, wire format and client.

mod client;
mod endpoint;
mod wire;
#[cfg(test)]
mod wire_tests;

pub use client::{SystemOneConfig, SystemOneProvider};
