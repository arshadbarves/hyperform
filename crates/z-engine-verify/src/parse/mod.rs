//! Test-count parsing: one summary format per file, chosen by the command
//! and by the shape of the output.

mod bun;
mod cargo;
mod ctest;
mod deno;
mod dispatch;
mod dotnet;
mod go;
mod gradle;
mod jest;
mod maven;
mod mocha;
mod pytest;
mod text;
mod vitest;

pub use dispatch::parse_counts;
