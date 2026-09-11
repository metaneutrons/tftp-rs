//! Embeddable asynchronous TFTP server.
//!
//! The companion `tftp-rs` binary provides the interactive dashboard. Users
//! who need to embed TFTP in another application should use [`server`].

pub mod server;

mod tftp_protocol;
