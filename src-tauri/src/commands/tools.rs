//! Tools commands: the tools' status, and installing the missing ones as jobs.

use std::sync::Arc;

use scrappy_core::jobs::Jobs;
use scrappy_core::tools::{ToolJob, ToolStatus, Toolbox};
use tauri::State;

/// Every tool, where it is, its version and any install running.
#[tauri::command]
pub async fn tools_status(toolbox: State<'_, Toolbox>) -> Result<Vec<ToolStatus>, String> {
    Ok(toolbox.status().await)
}

/// Starts installing every missing or outdated tool that has a download here; returns the jobs to follow.
#[tauri::command]
pub async fn install_tools(toolbox: State<'_, Toolbox>, jobs: State<'_, Arc<Jobs>>) -> Result<Vec<ToolJob>, String> {
    Ok(toolbox.install_needed(&jobs).await)
}
