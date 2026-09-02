//! MCP server for Nits (plan 2.5). Speaks JSON-RPC 2.0 over newline-delimited
//! stdio (the MCP stdio transport) and proxies every tool call to a `nitsd`
//! daemon over its unix socket or WebSocket.
//!
//! Every mutation is attributed to [`nits_protocol::Author::Agent`], built
//! from the MCP client's `initialize` info, so provenance is structural.

pub mod jsonrpc;
pub mod server;
pub mod tools;

pub use server::{Endpoint, Server};
