//! LNXDrive Core - Domain logic and business rules
//!
//! This crate contains the hexagonal architecture core with:
//! - **Domain entities** - `SyncItem`, `Conflict`, `Account`, `AuditEntry`, `SyncSession`
//! - **Use cases** - `AuthenticateUseCase`, `SyncFileUseCase`, `QueryDeltaUseCase`, `ExplainFailureUseCase`
//! - **Port definitions** - Traits for adapters: `ICloudProvider`, `IStateRepository`, `ILocalFileSystem`
//! - **State machine** - Files-On-Demand hydration states
//!
//! # Architecture
//!
//! This crate follows the hexagonal (ports & adapters) architecture pattern.
//! The domain module contains pure business logic with no external dependencies.
//! Ports define trait interfaces that adapter crates implement.
//! Use cases orchestrate domain entities through port interfaces.

pub mod config;
pub mod domain;
pub mod ports;
pub mod usecases;
