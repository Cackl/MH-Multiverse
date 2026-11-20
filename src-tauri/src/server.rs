use regex::Regex;
use rusqlite::{params, Connection, OpenFlags};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, LazyLock, Mutex};
use tauri::{AppHandle, Emitter, Manager};
use crate::ini;

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

// -- Helper --

fn log_timestamp() -> String {
    chrono::Local::now().format("%Y.%m.%d %H:%M:%S%.3f").to_string()
}

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

#[derive(Clone, serde::Serialize)]
pub struct PlayerSession {
    pub session_id: String,
    pub username: String,
    // Fields populated from Account.db at login time.
    // All optional — the DB lookup is best-effort and falls back gracefully.
    pub email: Option<String>,
    pub user_level: Option<i64>,
    pub flags: Option<i64>,
    pub gazillionite_balance: Option<i64>,
    pub last_logout_time: Option<i64>,
    pub avatar_count: Option<i64>,
    pub guild_name: Option<String>,
}

#[derive(Clone, serde::Serialize)]
pub struct PlayerEventPayload {
    pub kind: String, // "login" | "logout" | "clear"
    pub session_id: Option<String>,
    pub username: Option<String>,
    pub count: usize,
}

#[derive(Debug, Clone)]
enum PlayerLogEvent {
    Login { username: String, session_id: String },
    Logout { username: String, session_id: String },
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
    /// A shutdown was already sent to `child`, so a second Stop doesn't send another.
    stopping: bool,
    pub apache_child: Option<Child>,
    #[cfg(target_os = "windows")]
    _job: Option<JobObject>,
    // Apache's own Job Object (KILL_ON_JOB_CLOSE). Without it, a crash or
    // force-kill of this app leaves httpd.exe running and holding its port.
    #[cfg(target_os = "windows")]
    _apache_job: Option<JobObject>,
}

impl ServerProcess {
    pub fn empty() -> Self {
        Self {
            child: None,
            stopping: false,
            apache_child: None,
            #[cfg(target_os = "windows")]
            _job: None,
            #[cfg(target_os = "windows")]
            _apache_job: None,
        }
    }

    /// Kill Apache (if running) and release its Job Object.
    pub fn kill_apache(&mut self) {
        if let Some(mut apache) = self.apache_child.take() {
            let _ = apache.kill();
            let _ = apache.wait();
        }
        #[cfg(target_os = "windows")]
        { self._apache_job = None; }
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.kill_apache();
    }
}

pub struct ServerState(pub Arc<Mutex<ServerProcess>>);
pub struct PlayerState(pub Arc<Mutex<HashMap<String, PlayerSession>>>);

/// Stores the path to Account.db, derived from server_exe when it is set.
/// Wrapped in Mutex<Option<...>> so it can be updated without restarting.
pub struct DbPath(pub Arc<Mutex<Option<PathBuf>>>);

impl PlayerState {
    pub fn empty() -> Self {
        Self(Arc::new(Mutex::new(HashMap::new())))
    }
}

impl DbPath {
    pub fn empty() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }

    /// Derive and store the Account.db path from a server_exe path.
    /// Layout: <server_exe_dir>/Data/<[SQLiteDBManager] FileName>
    pub fn set_from_server_exe(&self, server_exe: &str) {
        let path = crate::accounts::db_path(server_exe).ok();
        if let Ok(mut guard) = self.0.lock() {
            *guard = path;
        }
    }
}

/// Kill both server and Apache processes. Called from the window close handler.
pub fn kill_child(proc: &mut ServerProcess) {
    if let Some(ref mut child) = proc.child {
        let _ = child.kill();
        let _ = child.wait();
    }
    proc.child = None;
    proc.kill_apache();
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

// -- Account.db lookup --

/// Extra account fields fetched from Account.db at login time.
struct AccountInfo {
    email: String,
    user_level: i64,
    flags: i64,
    gazillionite_balance: Option<i64>,
    last_logout_time: Option<i64>,
    avatar_count: Option<i64>,
    guild_name: Option<String>,
}

/// Look up account info by PlayerName. Opens a fresh connection to the DB on
/// every call, read-only (so it can neither write nor create the file). Safe
/// to call repeatedly because SQLite handles concurrent connections to the
/// same file fine; the server holds its own separate one.
/// Returns None on any error so the caller always gets a valid PlayerSession.
fn lookup_account(db_path: &PathBuf, username: &str) -> Option<AccountInfo> {
    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;

    let info = conn.query_row(
        "SELECT
             a.Email,
             a.UserLevel,
             a.Flags,
             p.GazillioniteBalance,
             p.LastLogoutTime,
             g.Name
         FROM Account a
         LEFT JOIN Player      p  ON p.DbGuid    = a.Id
         LEFT JOIN GuildMember gm ON gm.PlayerDbGuid = a.Id
         LEFT JOIN Guild       g  ON g.Id         = gm.GuildId
         WHERE a.PlayerName = ?1",
        params![username],
        |row| {
            Ok(AccountInfo {
                email:                 row.get(0)?,
                user_level:            row.get(1)?,
                flags:                 row.get(2)?,
                gazillionite_balance:  row.get(3)?,
                last_logout_time:      row.get(4)?,
                avatar_count:          None, // filled in below
                guild_name:            row.get(5)?,
            })
        },
    ).ok()?;

    // Count avatars separately — cleaner than a subquery in the join above.
    let avatar_count: Option<i64> = conn.query_row(
        "SELECT COUNT(*) FROM Avatar
         JOIN Account a ON a.Id = Avatar.ContainerDbGuid
         WHERE a.PlayerName = ?1",
        params![username],
        |row| row.get(0),
    ).ok();

    Some(AccountInfo { avatar_count, ..info })
}

// -- Player log event parsing --

static LOGIN_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[Account=(.+?)\s+\(.*?\),\s+SessionId=(0x[0-9A-Fa-f]+)\]").unwrap()
});
static LOGOUT_SESSION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"SessionId=(0x[0-9A-Fa-f]+)").unwrap());
static LOGOUT_USERNAME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Account=(.+?)\s+\(").unwrap());

fn parse_player_log_event(raw: &str) -> Option<PlayerLogEvent> {
    if raw.contains("Accepted and registered client") {
        if !raw.contains("SessionId=") || !raw.contains("Account=") {
            return None;
        }

        let caps = LOGIN_RE.captures(raw)?;
        let username = caps.get(1)?.as_str().trim().to_string();
        let session_id = caps.get(2)?.as_str().trim().to_string();

        return Some(PlayerLogEvent::Login { username, session_id });
    }

    if raw.contains("Removed client") {
        let session_caps = LOGOUT_SESSION_RE.captures(raw)?;
        let session_id = session_caps.get(1)?.as_str().trim().to_string();

        let username = LOGOUT_USERNAME_RE
            .captures(raw)
            .and_then(|caps| caps.get(1).map(|m| m.as_str().trim().to_string()))
            .unwrap_or_default();

        return Some(PlayerLogEvent::Logout { username, session_id });
    }

    None
}

fn emit_player_event(
    app: &AppHandle,
    player_state: &Arc<Mutex<HashMap<String, PlayerSession>>>,
    kind: &str,
    session_id: Option<String>,
    username: Option<String>,
) {
    let count = match player_state.lock() {
        Ok(sessions) => sessions.len(),
        Err(_) => return,
    };

    let _ = app.emit(
        "player-event",
        PlayerEventPayload {
            kind: kind.to_string(),
            session_id,
            username,
            count,
        },
    );
}

fn handle_player_log_event(
    app: &AppHandle,
    player_state: &Arc<Mutex<HashMap<String, PlayerSession>>>,
    db_path: &Arc<Mutex<Option<PathBuf>>>,
    event: PlayerLogEvent,
) {
    match player_state.lock() {
        Ok(mut sessions) => match &event {
            PlayerLogEvent::Login { username, session_id } => {
                // Best-effort DB lookup - runs on the stdout reader thread, which
                // is fine because lookup_account opens its own separate connection
                // and only ever issues SELECT queries against it.
                let db_opt = db_path.lock().ok().and_then(|g| g.clone());
                let account = db_opt.as_ref().and_then(|p| lookup_account(p, username));

                sessions.insert(
                    session_id.clone(),
                    PlayerSession {
                        session_id: session_id.clone(),
                        username: username.clone(),
                        email:                account.as_ref().map(|a| a.email.clone()),
                        user_level:           account.as_ref().map(|a| a.user_level),
                        flags:                account.as_ref().map(|a| a.flags),
                        gazillionite_balance: account.as_ref().and_then(|a| a.gazillionite_balance),
                        last_logout_time:     account.as_ref().and_then(|a| a.last_logout_time),
                        avatar_count:         account.as_ref().and_then(|a| a.avatar_count),
                        guild_name:           account.as_ref().and_then(|a| a.guild_name.clone()),
                    },
                );
            }
            PlayerLogEvent::Logout { username, session_id } => {
                let fallback_username = sessions
                    .get(session_id)
                    .map(|s| s.username.clone())
                    .unwrap_or_default();

                sessions.remove(session_id);

                let username = if username.is_empty() {
                    fallback_username
                } else {
                    username.clone()
                };

                drop(sessions);

                emit_player_event(
                    app,
                    player_state,
                    "logout",
                    Some(session_id.clone()),
                    Some(username),
                );
                return;
            }
        },
        Err(_) => return,
    }

    match event {
        PlayerLogEvent::Login { username, session_id } => {
            emit_player_event(app, player_state, "login", Some(session_id), Some(username));
        }
        PlayerLogEvent::Logout { .. } => {}
    }
}

fn clear_player_state(
    app: &AppHandle,
    player_state: &Arc<Mutex<HashMap<String, PlayerSession>>>,
) {
    if let Ok(mut sessions) = player_state.lock() {
        sessions.clear();
    }

    emit_player_event(app, player_state, "clear", None, None);
}

/// Returns true if the MHServerEmu child process is running.
///
/// Reads the watcher-owned `child` slot rather than calling `try_wait`
/// itself: the watcher thread spawned in `start_server` is the single
/// source of truth for exit detection and clears `child` when the process
/// goes away. Commands that change the server's files use `server_running`.
pub fn server_process_is_running(state: &ServerState) -> bool {
    state.0.lock().map(|proc| proc.child.is_some()).unwrap_or(false)
}

/// The app's server, or this same server exe started outside the app. For changes to the
/// server's files: a server the app didn't start writes over them just the same.
pub fn server_running(app: &AppHandle, server_exe: &str) -> bool {
    server_process_is_running(&app.state::<ServerState>()) || exe_is_running(std::path::Path::new(server_exe))
}

/// A process runs this exe. Matched by path, or by name when its path can't be read.
fn exe_is_running(exe: &std::path::Path) -> bool {
    use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};
    let Some(name) = exe.file_name() else { return false };
    let target = std::fs::canonicalize(exe).ok();
    let mut sys = System::new();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
    );
    sys.processes().values().any(|p| {
        p.name().eq_ignore_ascii_case(name)
            && p.exe().is_none_or(|e| std::fs::canonicalize(e).ok() == target)
    })
}

// The server runs without a console (CREATE_NO_WINDOW), so .NET reads and writes its
// console in the OEM code page (437 on an English Windows), not UTF-8.

/// Text in the OEM code page, for the server's stdin. Characters it lacks become "?".
#[cfg(target_os = "windows")]
fn oem_encode(s: &str) -> Vec<u8> {
    use windows::Win32::Globalization::{WideCharToMultiByte, CP_OEMCP};
    if s.is_ascii() {
        return s.as_bytes().to_vec();
    }
    let wide: Vec<u16> = s.encode_utf16().collect();
    unsafe {
        let len = WideCharToMultiByte(CP_OEMCP, 0, &wide, None, windows::core::PCSTR::null(), None);
        let mut out = vec![0u8; len.max(0) as usize];
        WideCharToMultiByte(CP_OEMCP, 0, &wide, Some(&mut out), windows::core::PCSTR::null(), None);
        out
    }
}

/// A line of the server's output, from the OEM code page.
#[cfg(target_os = "windows")]
fn oem_decode(bytes: &[u8]) -> String {
    use windows::Win32::Globalization::{MultiByteToWideChar, MULTI_BYTE_TO_WIDE_CHAR_FLAGS, CP_OEMCP};
    if bytes.is_ascii() {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    unsafe {
        let len = MultiByteToWideChar(CP_OEMCP, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), bytes, None);
        let mut wide = vec![0u16; len.max(0) as usize];
        MultiByteToWideChar(CP_OEMCP, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), bytes, Some(&mut wide));
        String::from_utf16_lossy(&wide)
    }
}

#[cfg(not(target_os = "windows"))]
fn oem_encode(s: &str) -> Vec<u8> { s.as_bytes().to_vec() }

#[cfg(not(target_os = "windows"))]
fn oem_decode(bytes: &[u8]) -> String { String::from_utf8_lossy(bytes).into_owned() }

/// The server's output, line by line. Unlike `lines()`, a byte that isn't UTF-8 doesn't end it.
fn console_lines(r: impl std::io::Read) -> impl Iterator<Item = String> {
    BufReader::new(r).split(b'\n').map_while(Result::ok).map(|mut b| {
        if b.last() == Some(&b'\r') { b.pop(); }
        oem_decode(&b)
    })
}

/// Writes an empty console line, which lets a server waiting in `Console.ReadLine` exit.
fn release_console_wait(state: &Mutex<ServerProcess>) {
    use std::io::Write;
    if let Ok(mut proc) = state.lock() {
        if let Some(stdin) = proc.child.as_mut().and_then(|c| c.stdin.as_mut()) {
            let _ = stdin.write_all(b"\n");
            let _ = stdin.flush();
        }
    }
}

/// The port from httpd's "could not bind to address 0.0.0.0:80" line, or None for other lines.
fn apache_port_in_use(line: &str) -> Option<&str> {
    let addr = line.split("could not bind to address ").nth(1)?;
    addr.trim().rsplit(':').next()
}

/// How long Stop waits for the server to exit before it kills the process.
const STOP_TIMEOUT_SECS: u64 = 60;

// -- Commands --

#[tauri::command]
pub async fn start_server(
    app: AppHandle,
    server_exe: String,
) -> Result<(), String> {
    let state = app.state::<ServerState>();
    let player_state = app.state::<PlayerState>().0.clone();
    let db_path = app.state::<DbPath>().0.clone();
    let mut proc = state.0.lock().map_err(|e| e.to_string())?;

    if proc.child.is_some() {
        return Err("Server is already running.".into());
    }
    // An install mid-way would fail on the dlls the server locks.
    if crate::updater::update_running() {
        return Err("Wait for the update to finish".into());
    }

    if server_exe.trim().is_empty() {
        return Err("Server executable path is not set.".into());
    }

    if !std::path::Path::new(&server_exe).is_file() {
        return Err(format!("Server executable not found: {server_exe}"));
    }

    // Pre-check: verify the WebFrontend port is available before attempting to spawn.
    // TcpStream::connect_timeout is used as a probe — a successful connection means
    // something is already listening on the port and MHServerEmu will fail to bind it.
    {
        let port = ini::read_merged_value(&server_exe, "WebFrontend", "Port", "8080");
        let addr = format!("127.0.0.1:{port}");
        if std::net::TcpStream::connect_timeout(
            &addr.parse().map_err(|e| format!("Invalid address: {e}"))?,
            std::time::Duration::from_millis(200),
        ).is_ok() {
            let msg = format!("Port {port} is already in use. WebFrontend cannot start. Free the port or change WebFrontend.Port in Config.ini.");
            let _ = app.emit("server-log", vec![LogLinePayload {
                time: log_timestamp(),
                level: "fatal".into(),
                msg: msg.clone(),
            }]);
            return Err(format!("Port {port} already in use."));
        }
    }

    let exe_path = std::path::Path::new(&server_exe);
    let mhserver_dir = exe_path.parent()
        .ok_or("Could not determine server directory")?;
    let working_dir = mhserver_dir;

    // Keep DbPath in sync with the exe we're actually launching.
    app.state::<DbPath>().set_from_server_exe(&server_exe);

    // Apache is no longer auto-started with the server.
    // Use the separate start_apache command instead.

    let mut cmd = Command::new(&server_exe);
    cmd.current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::piped());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let mut child = cmd
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
        let app_for_players = app.clone();
        let player_state_for_thread = player_state.clone();
        let db_path_for_thread = db_path.clone();
        let state_for_reader = state.0.clone();

        std::thread::Builder::new()
            .name("mhserver-stdout-reader".into())
            .spawn(move || {
                for raw in console_lines(stdout) {
                    // After shutdown, and after a Fatal error or crash, the server still
                    // waits on a console line before it exits (ServerApp.cs), so give it one.
                    // Every Fatal it logs is terminal, and an empty line does nothing otherwise.
                    let parsed = parse_log_line(&raw);
                    if parsed.level == "fatal" || raw.contains("All services shut down") {
                        release_console_wait(&state_for_reader);
                    }
                    let _ = tx.send(parsed);

                    if let Some(event) = parse_player_log_event(&raw) {
                        handle_player_log_event(
                            &app_for_players,
                            &player_state_for_thread,
                            &db_path_for_thread,
                            event,
                        );
                    }
                }
            })
            .expect("failed to spawn stdout reader thread");
    }

    if let Some(stderr) = child.stderr.take() {
        let tx = log_tx;
        std::thread::Builder::new()
            .name("mhserver-stderr-reader".into())
            .spawn(move || {
                for raw in console_lines(stderr) {
                    if raw.trim().is_empty() { continue; }
                    let _ = tx.send(LogLinePayload {
                        time: String::new(),
                        level: "err".into(),
                        msg: raw.trim().to_string(),
                    });
                }
            })
            .expect("failed to spawn stderr reader thread");
    }

    // Batcher thread
    {
        let app_clone = app.clone();
        std::thread::Builder::new()
            .name("mhserver-log-batcher".into())
            .spawn(move || {
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
            })
            .expect("failed to spawn log batcher thread");
    }

    // Watch for process exit on a background thread
    {
        let state_arc = state.0.clone();
        let app_clone = app.clone();
        let player_state_for_watcher = player_state.clone();
        let child_id = child.id();
        std::thread::Builder::new()
            .name("mhserver-watcher".into())
            .spawn(move || {
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
                                proc.kill_apache();

                                // Release job object
                                #[cfg(target_os = "windows")]
                                { proc._job = None; }

                                // Drop the lock before emitting
                                drop(proc);

                                clear_player_state(&app_clone, &player_state_for_watcher);

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
            })
            .expect("failed to spawn watcher thread");
    }

    proc.child = Some(child);
    proc.stopping = false;
    #[cfg(target_os = "windows")]
    { proc._job = job; }

    app.emit("server-started", ServerStatusPayload { running: true, exit_code: None })
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn stop_server(app: AppHandle) -> Result<(), String> {
    stop_gracefully(&app)
}

/// Asks the server to shut down, then waits for it to exit and stops the app.
/// Shared by the window close (`exit_app`) and its no-reply fallback.
pub fn exit_gracefully(app: &AppHandle) {
    let state = app.state::<ServerState>();
    if server_process_is_running(&state) && stop_gracefully(app).is_ok() {
        // The stop timer force-closes it at STOP_TIMEOUT_SECS, so this ends.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(STOP_TIMEOUT_SECS + 5);
        while server_process_is_running(&state) && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
    }
    crate::shutdown(app);
}

/// Sends "!server shutdown" once, and starts the force-close timer.
fn stop_gracefully(app: &AppHandle) -> Result<(), String> {
    use std::io::Write;

    let state = app.state::<ServerState>();
    let state_arc = state.0.clone();
    let player_state = app.state::<PlayerState>().0.clone();

    // Send "!server shutdown" via stdin, holding the lock only briefly.
    let pid = {
        let mut proc = state_arc.lock().map_err(|e| e.to_string())?;
        if proc.child.is_some() && proc.stopping {
            return Ok(());
        }
        let child = proc.child.as_mut().ok_or("Server is not running.")?;
        if let Some(ref mut stdin) = child.stdin {
            let _ = stdin.write_all(b"!server shutdown\n");
            let _ = stdin.flush();
        }
        let pid = child.id();
        proc.stopping = true;
        pid
    };
    // Lock released — returns to frontend immediately.
    // The watcher thread (spawned by start_server) will detect exit via
    // try_wait, clear proc.child, and emit server-stopped.

    // Safety net: the stdout reader releases the server once its services are
    // down, so it normally exits in a second or two. If it is still running
    // after STOP_TIMEOUT_SECS, hard-kill it. This thread does NOT poll, it
    // just sleeps and then checks once.
    let app_clone = app.clone();
    std::thread::Builder::new()
        .name("mhserver-stop-timeout".into())
        .spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(STOP_TIMEOUT_SECS));

            let mut proc = match state_arc.lock() {
                Ok(g) => g,
                Err(_) => return,
            };

            // Nothing to do if the watcher already saw it exit, or if the
            // server running now is a new one started after this Stop.
            if proc.child.as_ref().map(|c| c.id()) != Some(pid) {
                return;
            }

            // Still running: hard kill.
            if let Some(ref mut c) = proc.child {
                let _ = c.kill();
                let _ = c.wait();
            }
            proc.child = None;

            proc.kill_apache();

            #[cfg(target_os = "windows")]
            { proc._job = None; }

            // Watcher thread may have exited its loop when it saw child = None
            // from the kill above, so emit ourselves as a fallback.
            drop(proc);

            clear_player_state(&app_clone, &player_state);

            let _ = app_clone.emit("server-log", vec![LogLinePayload {
                time: log_timestamp(),
                level: "warn".into(),
                msg: format!("[MH Multiverse] The server didn't stop within {STOP_TIMEOUT_SECS} s and was force-closed"),
            }]);
            let _ = app_clone.emit("server-stopped", ServerStatusPayload {
                running: false,
                exit_code: None,
            });
        })
        .expect("failed to spawn stop-timeout thread");

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
    let mut cmd = Command::new(&apache_exe);
    cmd.current_dir(apache_working)
        .env("APACHE_SERVER_ROOT", root_dir.join("Apache24"))
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start Apache: {e}"))?;

    // httpd reports startup errors, like a port already in use, on stderr.
    if let Some(stderr) = child.stderr.take() {
        let app_clone = app.clone();
        std::thread::spawn(move || {
            let mut port_in_use = false;
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let line = line.trim();
                // httpd exits after a port error, and what it prints after it only says it gave up.
                if line.is_empty() || port_in_use {
                    continue;
                }
                let msg = match apache_port_in_use(line) {
                    Some(port) => {
                        port_in_use = true;
                        format!("[MH Multiverse] Apache can't start. Port {port} is already in use.")
                    }
                    None => format!("[Apache] {line}"),
                };
                let _ = app_clone.emit("server-log", vec![LogLinePayload {
                    time: log_timestamp(),
                    level: "err".into(),
                    msg,
                }]);
            }
        });
    }

    // Watch for httpd exiting on its own. Stop takes the child first, so this only reports
    // a failed start or a crash.
    {
        let state_arc = state.0.clone();
        let app_clone = app.clone();
        let pid = child.id();
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let Ok(mut proc) = state_arc.lock() else { break };
            let Some(apache) = proc.apache_child.as_mut().filter(|c| c.id() == pid) else { break };
            match apache.try_wait() {
                Ok(None) => {}
                Ok(Some(status)) => {
                    proc.apache_child = None;
                    #[cfg(target_os = "windows")]
                    { proc._apache_job = None; }
                    drop(proc);
                    let code = status.code().map_or("unknown".to_string(), |c| c.to_string());
                    let _ = app_clone.emit("server-log", vec![LogLinePayload {
                        time: log_timestamp(),
                        level: "err".into(),
                        msg: format!("[MH Multiverse] Apache stopped unexpectedly (code {code})"),
                    }]);
                    let _ = app_clone.emit("apache-stopped", ());
                    break;
                }
                Err(_) => break,
            }
        });
    }

    // Windows: put Apache in its own Job Object so it dies with this app.
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::io::AsRawHandle;
        let job = create_job_object();
        if let Some(ref j) = job {
            unsafe {
                let handle = HANDLE(child.as_raw_handle() as *mut core::ffi::c_void);
                let _ = AssignProcessToJobObject(j.0, handle);
            }
        }
        proc._apache_job = job;
    }

    proc.apache_child = Some(child);

    let _ = app.emit("server-log", vec![LogLinePayload {
        time: log_timestamp(),
        level: "ok".into(),
        msg: "Apache started".into(),
    }]);

    Ok(())
}

#[tauri::command]
pub async fn stop_apache(app: AppHandle) -> Result<(), String> {
    let state = app.state::<ServerState>();
    let mut proc = state.0.lock().map_err(|e| e.to_string())?;

    if proc.apache_child.is_some() {
        proc.kill_apache();

        let _ = app.emit("server-log", vec![LogLinePayload {
            time: log_timestamp(),
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
    stdin.write_all(&oem_encode(&line))
        .map_err(|e| format!("Failed to send command: {e}"))?;
    stdin.flush()
        .map_err(|e| format!("Failed to flush stdin: {e}"))?;

    Ok(())
}

/// Exit detection for the server process is owned entirely by the watcher
/// thread spawned in `start_server`, which clears `proc.child` once
/// `try_wait` reports it has exited. This command just reads that state
/// rather than calling `try_wait` itself, so there's a single source of
/// truth for "has the process exited" instead of two independent pollers
/// racing to observe the same one-shot exit status.
#[tauri::command]
pub fn server_is_running(app: AppHandle) -> bool {
    server_process_is_running(&app.state::<ServerState>())
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

#[tauri::command]
pub fn get_players(app: AppHandle) -> Result<Vec<PlayerSession>, String> {
    let state = app.state::<PlayerState>();
    let mut players: Vec<PlayerSession> =
        state.0.lock().map_err(|e| e.to_string())?.values().cloned().collect();

    // Re-read the account fields, so a ban, whitelist or level change since login shows.
    let db = app.state::<DbPath>().0.lock().ok().and_then(|g| g.clone());
    if let Some(db) = db {
        for p in &mut players {
            if let Some(a) = lookup_account(&db, &p.username) {
                p.user_level = Some(a.user_level);
                p.flags = Some(a.flags);
            }
        }
    }

    players.sort_by(|a, b| {
        a.username
            .to_lowercase()
            .cmp(&b.username.to_lowercase())
            .then_with(|| a.session_id.cmp(&b.session_id))
    });

    Ok(players)
}

#[cfg(test)]
mod tests {
    use super::{apache_port_in_use, console_lines, exe_is_running, oem_encode};

    #[test]
    fn a_running_exe_is_found_by_its_path() {
        assert!(exe_is_running(&std::env::current_exe().unwrap()));
        assert!(!exe_is_running(std::path::Path::new("C:/nowhere/NotRunning123.exe")));
    }

    #[test]
    fn console_text_round_trips_through_the_oem_code_page() {
        let sent = oem_encode("!server broadcast Café\n");
        assert!(sent.starts_with(b"!server broadcast Caf"));
        // A byte that isn't UTF-8 (é in the OEM code page) must not end the output.
        let read: Vec<String> = console_lines(&[sent.as_slice(), b"next\r\n"].concat()[..]).collect();
        assert_eq!(read, ["!server broadcast Café", "next"]);
    }

    #[test]
    fn apache_port_in_use_reads_the_port() {
        let line = "(OS 10048)Only one usage of each socket address (protocol/network address/port) is normally permitted. : AH00072: make_sock: could not bind to address 0.0.0.0:80";
        assert_eq!(apache_port_in_use(line), Some("80"));
        assert_eq!(apache_port_in_use("[::]:8080 could not bind to address [::]:8080"), Some("8080"));
        assert_eq!(apache_port_in_use("AH00451: no listening sockets available, shutting down"), None);
    }
}
