//! LNXDrive Core - Domain logic and business rules
//!
//! This crate contains the hexagonal architecture core with:
//! - Domain entities (SyncItem, Conflict, Account)
//! - Use cases (Interactors)
//! - Port definitions (traits for adapters)
//! - State machine for file synchronization

pub mod domain;
pub mod ports;
pub mod usecases;
