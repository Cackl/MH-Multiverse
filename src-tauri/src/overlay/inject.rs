//! Loads the hook by running the overlay's own injector, MhInject.exe. Multiverse never writes
//! into the game's memory itself: antivirus flags a launcher that does (Defender quarantined 1.7.1).

use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, Process32FirstW, Process32NextW, MODULEENTRY32W,
    PROCESSENTRY32W, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Threading::{OpenProcess, WaitForInputIdle, PROCESS_QUERY_INFORMATION, PROCESS_SYNCHRONIZE};

const IDLE_WAIT_MS: u32 = 15000;
const CREATE_NO_WINDOW: u32 = 0x08000000;
/// Longest injector message shown in the app.
const MAX_MESSAGE: usize = 300;

#[derive(Debug)]
pub enum InjectError {
    /// LoadLibrary didn't finish in time (hook still initialising); worth retrying.
    Timeout,
    Failed(String),
}

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { let _ = CloseHandle(self.0); }
    }
}

fn wide_str(s: &[u16]) -> String {
    let len = s.iter().position(|&c| c == 0).unwrap_or(s.len());
    String::from_utf16_lossy(&s[..len])
}

/// Block until the process is waiting for input (or the timeout elapses).
pub fn wait_input_idle(pid: u32) {
    if let Ok(p) = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_SYNCHRONIZE, false, pid) } {
        let p = Handle(p);
        unsafe { WaitForInputIdle(p.0, IDLE_WAIT_MS) };
    }
}

/// True if a module with this file name is loaded in the process.
pub fn module_loaded(pid: u32, name: &str) -> bool {
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) else { return false };
        let snap = Handle(snap);
        let mut me = MODULEENTRY32W { dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32, ..Default::default() };
        let mut ok = Module32FirstW(snap.0, &mut me).is_ok();
        while ok {
            if wide_str(&me.szModule).eq_ignore_ascii_case(name) {
                return true;
            }
            ok = Module32NextW(snap.0, &mut me).is_ok();
        }
        false
    }
}

/// Poll up to `secs` seconds for the module to appear.
pub fn wait_module(pid: u32, name: &str, secs: u64) -> bool {
    for _ in 0..secs {
        if module_loaded(pid, name) {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    module_loaded(pid, name)
}

/// The exe name of `pid` and how many running processes have that name.
fn name_and_count(pid: u32) -> Option<(String, usize)> {
    let mut names = Vec::new();
    let mut ours = None;
    unsafe {
        let snap = Handle(CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).ok()?);
        let mut pe = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        let mut ok = Process32FirstW(snap.0, &mut pe).is_ok();
        while ok {
            let name = wide_str(&pe.szExeFile);
            if pe.th32ProcessID == pid {
                ours = Some(name.clone());
            }
            names.push(name);
            ok = Process32NextW(snap.0, &mut pe).is_ok();
        }
    }
    let ours = ours?;
    let count = names.iter().filter(|n| n.eq_ignore_ascii_case(&ours)).count();
    Some((ours, count))
}

/// Runs `injector` to load `dll` into the game `pid`. MhInject finds the game by process name and
/// takes the first match, so this refuses when another process has the same name.
pub fn inject(pid: u32, injector: &Path, dll: &Path) -> Result<(), InjectError> {
    let fail = |m: String| InjectError::Failed(m);
    let (name, count) = name_and_count(pid).ok_or_else(|| fail("The game process was not found.".into()))?;
    if count > 1 {
        return Err(fail(format!(
            "Close the other {name} first. The overlay injector finds the game by name and could pick the wrong one."
        )));
    }
    let stem = Path::new(&name).file_stem().unwrap_or_default();
    let out = Command::new(injector)
        .arg(dll)
        .arg(stem)
        .current_dir(injector.parent().unwrap_or(Path::new(".")))
        .stdin(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| fail(format!("Cannot run MhInject.exe: {e}")))?;
    let code = out.status.code();
    let stderr = String::from_utf8_lossy(&out.stderr);
    outcome(code, &stderr)
}

/// Maps MhInject's exit code and stderr to a result.
fn outcome(code: Option<i32>, stderr: &str) -> Result<(), InjectError> {
    // The .NET app host fails with an 0x8000_80xx code when the runtime is missing.
    let host_error = code.is_some_and(|c| (c as u32) & 0xFFFF_FF00 == 0x8000_8000);
    match code {
        Some(0) => Ok(()),
        _ if stderr.trim_start().starts_with("TIMEOUT") => Err(InjectError::Timeout),
        _ if host_error || stderr.contains("install or update .NET") => Err(InjectError::Failed(
            "The overlay injector needs the .NET 8 Runtime. Install it from Microsoft and launch again.".into(),
        )),
        _ => {
            let first = stderr.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("no message");
            let clean: String = first.chars().filter(|c| !c.is_control()).take(MAX_MESSAGE).collect();
            let code = code.map_or("none".into(), |c| c.to_string());
            Err(InjectError::Failed(format!("The overlay injector failed (exit {code}): {clean}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcomes() {
        assert!(outcome(Some(0), "").is_ok());
        assert!(matches!(outcome(Some(4), "TIMEOUT: CreateRemoteThread did not finish"), Err(InjectError::Timeout)));
        let msg = |r: Result<(), InjectError>| match r { Err(InjectError::Failed(m)) => m, other => panic!("{other:?}") };
        assert!(msg(outcome(Some(0x8000_8096u32 as i32), "")).contains(".NET 8"));
        assert!(msg(outcome(Some(1), "You must install or update .NET to run this application.")).contains(".NET 8"));
        let m = msg(outcome(Some(4), "\nOpenProcess failed (try Administrator): 5\r\nmore"));
        assert_eq!(m, "The overlay injector failed (exit 4): OpenProcess failed (try Administrator): 5");
        assert!(msg(outcome(None, "")).contains("exit none"));
    }

    #[test]
    fn finds_own_process_by_pid() {
        let (name, count) = name_and_count(std::process::id()).unwrap();
        assert!(name.to_ascii_lowercase().ends_with(".exe"));
        assert!(count >= 1);
        assert!(name_and_count(u32::MAX).is_none());
    }
}
