//! `page mcp` — start the MCP server over stdio.
//!
//! This command is designed to be spawned by Claude Code (or other MCP clients)
//! as a subprocess. It communicates via JSON-RPC over stdin/stdout.

use clap::Args;

#[derive(Args)]
pub struct McpArgs;

pub fn run(_args: &McpArgs) -> anyhow::Result<()> {
    crate::mcp::serve()
}
