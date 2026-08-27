//! MCP server for Moor (plan 2.5). Speaks JSON-RPC 2.0 over newline-delimited
//! stdio (the MCP stdio transport) and proxies every tool call to a `moord`
//! daemon over its unix socket or WebSocket.
//!
//! Every mutation is attributed to [`moor_protocol::Author::Agent`], built
//! from the MCP client's `initialize` info, so provenance is structural.

pub mod jsonrpc;
pub mod server;
pub mod text;
pub mod tools;

pub use server::{Endpoint, Server};
