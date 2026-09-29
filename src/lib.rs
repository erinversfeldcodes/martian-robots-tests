//! The contract, and the machinery that grades a program against it.
//!
//! A library rather than one binary because the suite needs a second program
//! to point at: an implementation built to conform, so that a case which
//! fails a *correct* program is caught here rather than by whoever is trying
//! to satisfy the suite. That probe lives in `src/bin/`, and it reads the same
//! contract and reuses the same reference simulation.

pub mod cases;
pub mod contract;
pub mod expect;
pub mod mission;
pub mod modes;
pub mod mutation;
pub mod properties;
pub mod reference;
pub mod rng;
pub mod run;
pub mod spelling;
