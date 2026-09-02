use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::AppState;

/// A spawned local model server (e.g. a llamafile) used as the embedded repair
/// model. The child handle is kept so we can stop it / kill it on app exit.
pub struct EmbeddedModel {
    pub child: std::process::Child,
    /// The port we asked the server to bind.
    pub requested_port: u16,
    pub model: String,
    /// The port the server ACTUALLY bound, parsed from its startup log. llamafile
    /// silently falls back to another port if the requested one is taken, so
    /// trusting `requested_port` led the app to send repair to a dead port. None
    /// until the "listening on http://…:PORT" line is seen; shared with the
    /// log-reader thread.
    pub bound_port: std::sync::Arc<std::sync::Mutex<Option<u16>>>,
}

impl EmbeddedModel {
    /// The port to actually talk to: the bound port once discovered, else the
    /// requested one (server may still be starting).
    fn effective_port(&self) -> u16 {
        self.bound_port
            .lock()
            .ok()
            .and_then(|guard| *guard)
            .unwrap_or(self.requested_port)
    }
}

/// Pull the bound port out of a llamafile/llama.cpp startup line such as
/// "llama server listening at http://127.0.0.1:8081" or
/// "server is listening on http://127.0.0.1:8082".
pub(crate) fn parse_listening_port(line: &str) -> Option<u16> {
    if !line.to_ascii_lowercase().contains("listening") {
        return None;
    }
    let after_scheme = line.split("http://").nth(1)?;
    let after_colon = after_scheme.split(':').nth(1)?;
    let digits: String = after_colon
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

/// On Windows a `.llamafile` isn't directly executable; it must be a `.exe`.
/// Given a path, return its `.exe` sibling iff it's a `.llamafile`, so the
/// launcher can prefer or produce a runnable file. None for any other extension.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn exe_sibling_for_llamafile(path: &str) -> Option<String> {
    let p = std::path::Path::new(path);
    let is_llamafile = p
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("llamafile"))
        .unwrap_or(false);
    is_llamafile.then(|| p.with_extension("exe").to_string_lossy().into_owned())
}

/// Drain a child's stdout/stderr, capturing the real bound port from the first
/// "listening" line. Draining to EOF also prevents the pipe filling and blocking
/// the server.
fn spawn_port_log_reader<R: std::io::Read + Send + 'static>(
    reader: Option<R>,
    bound_port: std::sync::Arc<std::sync::Mutex<Option<u16>>>,
) {
    let Some(reader) = reader else {
        return;
    };
    std::thread::spawn(move || {
        use std::io::BufRead;
        let buffered = std::io::BufReader::new(reader);
        for line in buffered.lines().map_while(Result::ok) {
            let already_known = bound_port.lock().map(|g| g.is_some()).unwrap_or(true);
            if !already_known {
                if let Some(found) = parse_listening_port(&line) {
                    if let Ok(mut guard) = bound_port.lock() {
                        *guard = Some(found);
                    }
                }
            }
        }
    });
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedModelStatus {
    /// The process is spawned and hasn't exited.
    pub running: bool,
    /// The server answered its /health endpoint OK (model loaded, accepting requests).
    pub ready: bool,
    pub url: Option<String>,
    pub model: Option<String>,
}

impl EmbeddedModelStatus {
    fn stopped() -> Self {
        Self {
            running: false,
            ready: false,
            url: None,
            model: None,
        }
    }
}

/// Spawn a local model server (a single-file llamafile) and use it as the
/// embedded repair endpoint. Hardcoded/dev-friendly: you pass the path to the
/// file. Returns immediately after launch (the model can take a while to load) —
/// poll `embedded_repair_model_status` for readiness. Any prior instance is
/// stopped first.
#[tauri::command]
pub fn start_embedded_repair_model(
    state: State<'_, AppState>,
    binary_path: String,
    port: Option<u16>,
    model_name: Option<String>,
) -> Result<EmbeddedModelStatus, String> {
    use std::process::{Command, Stdio};

    let port = port.unwrap_or(8080);
    let model = model_name
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "local-model".to_string());
    let url = format!("http://127.0.0.1:{port}/v1");
    // Only the `cfg(windows)` block below reassigns this, so every other target
    // sees an unused `mut`.
    #[cfg_attr(not(windows), allow(unused_mut))]
    let mut path = binary_path.trim().to_string();
    if path.is_empty() {
        return Err("Provide the path to your llamafile (the single model file).".into());
    }
    // Windows can't execute a bare `.llamafile` (it isn't in PATHEXT and the shell
    // tries to "open" it). Transparently resolve to a `.exe`: prefer an existing
    // sibling, otherwise rename the `.llamafile` (instant, same directory). On
    // Unix the file runs directly, so this is a no-op there.
    #[cfg(windows)]
    if let Some(exe) = exe_sibling_for_llamafile(&path) {
        if std::path::Path::new(&exe).exists() {
            path = exe;
        } else if std::path::Path::new(&path).exists() {
            std::fs::rename(&path, &exe).map_err(|err| {
                format!(
                    "Windows needs a .exe to run the model; auto-rename from .llamafile failed: {err}. Rename the file to .exe manually."
                )
            })?;
            path = exe;
        }
    }
    if !std::path::Path::new(&path).exists() {
        return Err(format!("No file found at: {path}"));
    }

    // Stop any previous instance before launching a new one.
    {
        let mut guard = state.local_model.lock().map_err(|err| err.to_string())?;
        if let Some(mut existing) = guard.take() {
            let _ = existing.child.kill();
        }
    }

    // NOTE: no `--nobrowser` — it was removed in newer llamafile (0.10.x rejects
    // it with "invalid argument" and won't start); that build's server does not
    // auto-open a browser anyway. The remaining flags are stable across versions.
    // Pipe stdout+stderr so a reader thread can capture the port the server
    // actually bound (it falls back off a busy port without telling us otherwise).
    let mut child = Command::new(&path)
        .args([
            "--server",
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("Failed to launch '{path}': {err}"))?;

    let bound_port = std::sync::Arc::new(std::sync::Mutex::new(None));
    spawn_port_log_reader(child.stdout.take(), bound_port.clone());
    spawn_port_log_reader(child.stderr.take(), bound_port.clone());

    {
        let mut guard = state.local_model.lock().map_err(|err| err.to_string())?;
        *guard = Some(EmbeddedModel {
            child,
            requested_port: port,
            model: model.clone(),
            bound_port,
        });
    }

    Ok(EmbeddedModelStatus {
        running: true,
        ready: false,
        url: Some(url),
        model: Some(model),
    })
}

/// Stop the embedded repair model if running.
#[tauri::command]
pub fn stop_embedded_repair_model(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.local_model.lock().map_err(|err| err.to_string())?;
    if let Some(mut model) = guard.take() {
        let _ = model.child.kill();
    }
    Ok(())
}

/// Report whether the embedded model is running and ready (one quick /health
/// probe). The frontend polls this after start to know when repair can use it.
#[tauri::command]
pub async fn embedded_repair_model_status(
    state: State<'_, AppState>,
) -> Result<EmbeddedModelStatus, String> {
    let (model, port) = {
        let mut guard = state.local_model.lock().map_err(|err| err.to_string())?;
        match guard.as_mut() {
            None => return Ok(EmbeddedModelStatus::stopped()),
            Some(model) => {
                // Detect a crashed/exited child and clear it.
                if matches!(model.child.try_wait(), Ok(Some(_))) {
                    *guard = None;
                    return Ok(EmbeddedModelStatus::stopped());
                }
                // Use the port the server actually bound, not the one we asked for.
                (model.model.clone(), model.effective_port())
            }
        }
    };
    let url = format!("http://127.0.0.1:{port}/v1");
    // Probe health WITHOUT holding the lock across the await.
    let health = format!("http://127.0.0.1:{port}/health");
    let ready = reqwest::Client::new()
        .get(&health)
        .timeout(Duration::from_secs(2))
        .send()
        .await
        .map(|response| response.status().is_success())
        .unwrap_or(false);
    Ok(EmbeddedModelStatus {
        running: true,
        ready,
        url: Some(url),
        model: Some(model),
    })
}
