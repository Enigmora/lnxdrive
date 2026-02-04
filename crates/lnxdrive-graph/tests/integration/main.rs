//! Integration tests for lnxdrive-graph
//!
//! Uses wiremock to simulate the Microsoft Graph API and verifies
//! end-to-end behavior of the GraphClient, delta queries, uploads,
//! and downloads.

mod common;

mod test_user_info;
mod test_delta;
mod test_sync_operations;
