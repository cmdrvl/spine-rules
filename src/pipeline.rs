//! Pipeline integration helpers for multi-tool spine tests.
//!
//! Provides binary discovery, subprocess execution, piping, and
//! output parsing utilities for chaining spine tools together.

use std::io::Write;
use std::process::{Command, Stdio};

/// Result of running a single tool.
#[derive(Debug)]
pub struct ToolRun {
    /// Process exit code (None if killed by signal).
    pub code: Option<i32>,
    /// Captured stdout bytes.
    pub stdout: Vec<u8>,
    /// Captured stderr bytes.
    pub stderr: Vec<u8>,
}

impl ToolRun {
    /// True when the process exited with code 0.
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }

    /// Stdout interpreted as lossy UTF-8.
    pub fn stdout_text(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    /// Stderr interpreted as lossy UTF-8.
    pub fn stderr_text(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
}

/// Check whether `name` exists on `PATH`. Returns the name itself
/// (not the full path) so callers can pass it straight to `Command::new`.
pub fn find_binary(name: &str) -> Option<String> {
    Command::new("which")
        .arg(name)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()
        .filter(|s| s.success())
        .map(|_| name.to_string())
}

/// Run a tool and capture exit code + stdout + stderr.
pub fn run_tool(binary: &str, args: &[&str]) -> ToolRun {
    let output = Command::new(binary)
        .args(args)
        .env_remove("EPISTEMIC_WITNESS")
        .output()
        .unwrap_or_else(|e| panic!("Failed to run {} {:?}: {}", binary, args, e));

    ToolRun {
        code: output.status.code(),
        stdout: output.stdout,
        stderr: output.stderr,
    }
}

/// Run a tool with `stdin_bytes` piped to its stdin.
pub fn run_tool_with_stdin(binary: &str, args: &[&str], stdin_bytes: &[u8]) -> ToolRun {
    let mut child = Command::new(binary)
        .args(args)
        .env_remove("EPISTEMIC_WITNESS")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("Failed to spawn {} {:?}: {}", binary, args, e));

    child
        .stdin
        .take()
        .expect("stdin must be piped")
        .write_all(stdin_bytes)
        .unwrap_or_else(|e| panic!("Failed to write stdin to {}: {}", binary, e));

    let output = child
        .wait_with_output()
        .unwrap_or_else(|e| panic!("Failed to wait for {}: {}", binary, e));

    ToolRun {
        code: output.status.code(),
        stdout: output.stdout,
        stderr: output.stderr,
    }
}

/// Pipe `tool_a` stdout into `tool_b` stdin (streaming, no intermediate file).
///
/// Returns the [`ToolRun`] for `tool_b`. If `tool_a` fails to start, panics.
pub fn pipe_tools(
    bin_a: &str,
    args_a: &[&str],
    bin_b: &str,
    args_b: &[&str],
) -> ToolRun {
    let child_a = Command::new(bin_a)
        .args(args_a)
        .env_remove("EPISTEMIC_WITNESS")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("Failed to spawn {} {:?}: {}", bin_a, args_a, e));

    let output_b = Command::new(bin_b)
        .args(args_b)
        .env_remove("EPISTEMIC_WITNESS")
        .stdin(child_a.stdout.expect("child_a stdout must be piped"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap_or_else(|e| panic!("Failed to run {} {:?}: {}", bin_b, args_b, e));

    ToolRun {
        code: output_b.status.code(),
        stdout: output_b.stdout,
        stderr: output_b.stderr,
    }
}

/// Parse JSONL bytes (one JSON object per line) into `Vec<Value>`.
///
/// Blank lines are silently skipped.
pub fn parse_jsonl(stdout: &[u8]) -> Vec<serde_json::Value> {
    let text = String::from_utf8_lossy(stdout);
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("Invalid JSON line: {e}\n  line: {line}"))
        })
        .collect()
}

/// Parse a single JSON object from bytes.
pub fn parse_json(stdout: &[u8]) -> serde_json::Value {
    serde_json::from_slice(stdout)
        .unwrap_or_else(|e| {
            let text = String::from_utf8_lossy(stdout);
            panic!("Invalid JSON: {e}\n  input: {text}")
        })
}

/// Early-return macro that skips a test (with a message) if any of the
/// listed binaries are missing from `PATH`.
///
/// ```ignore
/// require_binaries!(vacuum, hashbytes, lock);
/// ```
#[macro_export]
macro_rules! require_binaries {
    ($($bin:ident),+ $(,)?) => {{
        let missing: Vec<&str> = vec![
            $(
                {
                    let name = stringify!($bin);
                    if $crate::pipeline::find_binary(name).is_none() {
                        Some(name)
                    } else {
                        None
                    }
                },
            )+
        ]
        .into_iter()
        .flatten()
        .collect();

        if !missing.is_empty() {
            eprintln!(
                "SKIPPED: binaries not on PATH: {}",
                missing.join(", ")
            );
            return;
        }
    }};
}
