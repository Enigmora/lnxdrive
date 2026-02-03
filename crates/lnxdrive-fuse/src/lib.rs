//! LNXDrive FUSE - Files-on-Demand filesystem
//!
//! Implements a FUSE filesystem that provides:
//! - Placeholder files (sparse files with metadata)
//! - On-demand hydration when files are accessed
//! - Automatic dehydration for space management
//! - Extended attributes for file state
