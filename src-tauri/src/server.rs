use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

#[cfg(target_os = "windows")]
use windows::{
    Win32::Foundation::HANDLE,
    Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    },
};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

// -- Payload types emitted to the frontend --

#[derive(Clone, serde::Serialize)]
pub struct LogLinePayload {
    pub time: String,
    pub level: String,
    pub msg: String,
}

#[derive(Clone, serde::Serialize)]
pub struct ServerStatusPayload {
    pub running: bool,
    pub exit_code: Option<i32>,
}

// -- Job Object wrapper (Windows-only) --

#[cfg(target_os = "windows")]
struct JobObject(HANDLE);

#[cfg(target_os = "windows")]
unsafe impl Send for JobObject {}
#[cfg(target_os = "windows")]
unsafe impl Sync for JobObject {}

#[cfg(target_os = "windows")]
impl Drop for JobObject {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(target_os = "windows")]
fn create_job_object() -> Option<JobObject> {
    unsafe {
        let job = CreateJobObjectW(None, None).ok()?;
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const _,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
        .ok()?;
        Some(JobObject(job))
    }
}

// -- Managed state --

pub struct ServerProcess {
    pub child: Option<Child>,
    pub apache_child: Option<Child>,
    #[cfg(target_os = "windows")]
    _job: Option<JobObject>,
}

impl ServerProcess {
    pub fn empty() -> Self {
        Self {
            child: None,
            apache_child: None,
            #[cfg(target_os = "windows")]
            _job: None,
        }
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(mut apache) = self.apache_child.take() {
            let _ = apache.kill();
            let _ = apache.wait();
        }
    }
}

pub struct ServerState(pub Arc<Mutex<ServerProcess>>);

/// Kill both server and Apache processes. Called from the window close handler.
pub fn kill_child(proc: &mut ServerProcess) {
    if let Some(ref mut child) = proc.child {
        let _ = child.kill();
        let _ = child.wait();
    }
    proc.child = None;
    if let Some(ref mut apache) = proc.apache_child {
        let _ = apache.kill();
        let _ = apache.wait();
    }
    proc.apache_child = None;
}

// -- Parse a raw log line into structured fields --
// MHServerEmu format: [HH:MM:SS] [Level,5] [Category] Message
// Level is one of: Trace, Debug, Info, Warn, Error, Fatal (right-padded to 5 chars)

fn strip_bracket(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start();
    if !s.starts_with('[') { return None; }
    let end = s.find(']')?;
    Some((s[1..end].trim(), s[end + 1..].trim_start()))
}

fn parse_log_line(raw: &str) -> LogLinePayload {
    let raw = raw.trim();

    // Extract [timestamp]
    let (time, rest) = match strip_bracket(raw) {
        Some((t, r)) => (t.to_string(), r),
        None => return LogLinePayload {
            time: String::new(),
            level: "info".to_string(),
            msg: raw.to_string(),
        },
    };

    // Extract [Level]
    let (level_str, rest) = match strip_bracket(rest) {
        Some((l, r)) => (l.trim().to_lowercase(), r),
        None => return LogLinePayload {
            time,
            level: "info".to_string(),
            msg: rest.to_string(),
        },
    };

    let level = match level_str.as_str() {
        "trace" => "trace",
        "debug" => "debug",
        "info" => "info",
        "warn" => "warn",
        "error" => "err",
        "fatal" => "fatal",
        _ => "info",
    };

    // The rest is [Category] Message — keep it as the display message
    LogLinePayload {
        time,
        level: level.to_string(),
        msg: rest.to_string(),
    }
}

// -- Commands --

#[tauri::command]
pub async fn start_server(
    app: AppHandle,
    server_exe: String,
) -> Result<(), String> {
    let state = app.state::<ServerState>();
    let mut proc = state.0.lock().map_err(|e| e.to_string())?;

    if proc.child.is_some() {
        return Err("Server is already running.".into());
    }

    if server_exe.trim().is_empty() {
        return Err("Server executable path is not set.".into());
    }

    if !std::path::Path::new(&server_exe).is_file() {
        return Err(format!("Server executable not found: {server_exe}"));
    }

    let exe_path = std::path::Path::new(&server_exe);
    let mhserver_dir = exe_path.parent()
        .ok_or("Could not determine server directory")?;
    let working_dir = mhserver_dir;

    // Apache is no longer auto-started with the server.
    // Use the separate start_apache command instead.

    let mut child = Command::new(&server_exe)
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Failed to spawn server: {e}"))?;

    // Assign to Job Object (Windows only)
    #[cfg(target_os = "windows")]
    let job = {
        use std::os::windows::io::AsRawHandle;
        let job = create_job_object();
        if let Some(ref j) = job {
            unsafe {
                let raw_handle = child.as_raw_handle();
                let handle = HANDLE(raw_handle as *mut core::ffi::c_void);
                let _ = AssignProcessToJobObject(j.0, handle);
            }
        }
        job
    };

    // Stream stdout and stderr through a shared channel into a batching thread.
    let (log_tx, log_rx) = std::sync::mpsc::channel::<LogLinePayload>();

    if let Some(stdout) = child.stdout.take() {
        let tx = log_tx.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(raw) => { let _ = tx.send(parse_log_line(&raw)); }
                    Err(_) => break,
                }
            }
        });
    }

    if let Some(stderr) = child.stderr.take() {
        let tx = log_tx;
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                match line {
                    Ok(raw) if !raw.trim().is_empty() => {
                        let _ = tx.send(LogLinePayload {
                            time: String::new(),
                            level: "err".into(),
                            msg: raw.trim().to_string(),
                        });
                    }
                    _ => break,
                }
            }
        });
    }

    // Batcher thread
    {
        let app_clone = app.clone();
        std::thread::spawn(move || {
            const MAX_BATCH: usize = 50;
            const FLUSH_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);
            let mut batch: Vec<LogLinePayload> = Vec::with_capacity(MAX_BATCH);

            loop {
                match log_rx.recv_timeout(FLUSH_INTERVAL) {
                    Ok(line) => {
                        batch.push(line);
                        if batch.len() >= MAX_BATCH {
                            let _ = app_clone.emit("server-log", std::mem::take(&mut batch));
                            batch.reserve(MAX_BATCH);
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        if !batch.is_empty() {
                            let _ = app_clone.emit("server-log", std::mem::take(&mut batch));
                            batch.reserve(MAX_BATCH);
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        if !batch.is_empty() {
                            let _ = app_clone.emit("server-log", batch);
                        }
                        break;
                    }
                }
            }
        });
    }

    // Watch for process exit on a background thread
    {
        let state_arc = state.0.clone();
        let app_clone = app.clone();
        let child_id = child.id();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_millis(150));
                let mut proc = match state_arc.lock() {
                    Ok(g) => g,
                    Err(_) => break,
                };
                if let Some(ref mut c) = proc.child {
                    if c.id() != child_id {
                        break;
                    }
                    match c.try_wait() {
                        Ok(Some(status)) => {
                            let code = status.code();
                            proc.child = None;

                            // Clean up Apache
                            if let Some(mut apache) = proc.apache_child.take() {
                                let _ = apache.kill();
                                let _ = apache.wait();
                            }

                            // Release job object
                            #[cfg(target_os = "windows")]
                            { proc._job = None; }

                            // Drop the lock before emitting
                            drop(proc);

                            let _ = app_clone.emit("server-stopped", ServerStatusPayload {
                                running: false,
                                exit_code: code,
                            });
                            break;
                        }
                        Ok(None) => {}
                        Err(_) => break,
                    }
                } else {
                    break;
                }
            }
        });
    }

    proc.child = Some(child);
    #[cfg(target_os = "windows")]
    { proc._job = job; }

    app.emit("server-started", ServerStatusPayload { running: true, exit_code: None })
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn stop_server(app: AppHandle) -> Result<(), String> {
    use std::io::Write;

    let state = app.state::<ServerState>();
    let state_arc = state.0.clone();

    // Send "!server shutdown" via stdin, holding the lock only briefly.
    {
        let mut proc = state_arc.lock().map_err(|e| e.to_string())?;
        let child = proc.child.as_mut().ok_or("Server is not running.")?;
        if let Some(ref mut stdin) = child.stdin {
            let _ = stdin.write_all(b"!server shutdown\n");
            let _ = stdin.flush();
        }
    }
    // Lock released — returns to frontend immediately.
    // The watcher thread (spawned by start_server) will detect exit via
    // try_wait, clear proc.child, and emit server-stopped.

    // Safety net: if the process hasn't exited after 10 seconds,
    // hard-kill it. This thread does NOT poll or emit — it just
    // sleeps and then checks once.
    let app_clone = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(10));

        let mut proc = match state_arc.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        // If child is already None, the watcher handled it — nothing to do.
        if proc.child.is_none() {
            return;
        }

        // Still running after 10s — hard kill.
        if let Some(ref mut c) = proc.child {
            let _ = c.kill();
            let _ = c.wait();
        }
        proc.child = None;

        if let Some(mut apache) = proc.apache_child.take() {
            let _ = apache.kill();
            let _ = apache.wait();
        }

        #[cfg(target_os = "windows")]
        { proc._job = None; }

        // Watcher thread may have exited its loop when it saw child = None
        // from the kill above, so emit ourselves as a fallback.
        drop(proc);
        let _ = app_clone.emit("server-stopped", ServerStatusPayload {
            running: false,
            exit_code: None,
        });
    });

    Ok(())
}

#[tauri::command]
pub async fn start_apache(app: AppHandle, server_exe: String) -> Result<(), String> {
    let state = app.state::<ServerState>();
    let mut proc = state.0.lock().map_err(|e| e.to_string())?;

    if proc.apache_child.is_some() {
        return Err("Apache is already running.".into());
    }

    if server_exe.trim().is_empty() {
        return Err("Server executable path is not set.".into());
    }

    // Derive Apache path: server_exe = .../MHServerEmu/MHServerEmu.exe
    // Apache = .../Apache24/bin/httpd.exe (sibling of MHServerEmu dir)
    let exe_path = std::path::Path::new(&server_exe);
    let mhserver_dir = exe_path.parent()
        .ok_or("Could not determine server directory")?;
    let root_dir = mhserver_dir.parent()
        .ok_or("Could not determine root directory")?;
    let apache_exe = root_dir.join("Apache24").join("bin").join("httpd.exe");

    if !apache_exe.is_file() {
        return Err(format!("Apache not found at {}", apache_exe.display()));
    }

    let apache_working = apache_exe.parent().unwrap();
    let child = Command::new(&apache_exe)
        .current_dir(apache_working)
        .env("APACHE_SERVER_ROOT", root_dir.join("Apache24"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map_err(|e| format!("Failed to start Apache: {e}"))?;

    proc.apache_child = Some(child);

    let _ = app.emit("server-log", vec![LogLinePayload {
        time: String::new(),
        level: "ok".into(),
        msg: "Apache started".into(),
    }]);

    Ok(())
}

#[tauri::command]
pub async fn stop_apache(app: AppHandle) -> Result<(), String> {
    let state = app.state::<ServerState>();
    let mut proc = state.0.lock().map_err(|e| e.to_string())?;

    if let Some(mut apache) = proc.apache_child.take() {
        let _ = apache.kill();
        let _ = apache.wait();

        let _ = app.emit("server-log", vec![LogLinePayload {
            time: String::new(),
            level: "info".into(),
            msg: "Apache stopped".into(),
        }]);
    } else {
        return Err("Apache is not running.".into());
    }

    Ok(())
}

#[tauri::command]
pub fn send_command(app: AppHandle, cmd: String) -> Result<(), String> {
    use std::io::Write;

    let state = app.state::<ServerState>();
    let mut proc = state.0.lock().map_err(|e| e.to_string())?;

    let child = proc.child.as_mut().ok_or("Server is not running.")?;
    let stdin = child.stdin.as_mut().ok_or("Server stdin is not available.")?;

    let line = format!("{}\n", cmd.trim());
    stdin.write_all(line.as_bytes())
        .map_err(|e| format!("Failed to send command: {e}"))?;
    stdin.flush()
        .map_err(|e| format!("Failed to flush stdin: {e}"))?;

    Ok(())
}

#[tauri::command]
pub fn server_is_running(app: AppHandle) -> bool {
    let state = app.state::<ServerState>();
    let mut proc = match state.0.lock() {
        Ok(g) => g,
        Err(_) => return false,
    };
    if let Some(ref mut child) = proc.child {
        match child.try_wait() {
            Ok(None) => true,
            _ => false,
        }
    } else {
        false
    }
}

#[tauri::command]
pub fn apache_is_running(app: AppHandle) -> bool {
    let state = app.state::<ServerState>();
    let mut proc = match state.0.lock() {
        Ok(g) => g,
        Err(_) => return false,
    };
    if let Some(ref mut apache) = proc.apache_child {
        match apache.try_wait() {
            Ok(None) => true,
            _ => false,
        }
    } else {
        false
    }
}