//! Classic LoadLibraryW remote-thread injection, by PID, with the minimum
//! process rights. Mirrors what the overlay's own launcher does.

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows::core::{s, w};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Diagnostics::Debug::WriteProcessMemory;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, MODULEENTRY32W, TH32CS_SNAPMODULE,
    TH32CS_SNAPMODULE32,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Memory::{
    VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE,
};
use windows::Win32::System::Threading::{
    CreateRemoteThread, GetExitCodeThread, OpenProcess, WaitForInputIdle, WaitForSingleObject,
    PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_VM_OPERATION,
    PROCESS_VM_WRITE,
};

const LOAD_WAIT_MS: u32 = 5000;
const IDLE_WAIT_MS: u32 = 15000;

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

fn open(pid: u32) -> Result<Handle, InjectError> {
    let rights = PROCESS_CREATE_THREAD
        | PROCESS_QUERY_INFORMATION
        | PROCESS_VM_OPERATION
        | PROCESS_VM_WRITE
        | PROCESS_SYNCHRONIZE;
    unsafe { OpenProcess(rights, false, pid) }
        .map(Handle)
        .map_err(|e| InjectError::Failed(format!("OpenProcess failed: {e}")))
}

/// Block until the process is waiting for input (or the timeout elapses).
pub fn wait_input_idle(pid: u32) -> Result<(), InjectError> {
    let p = open(pid)?;
    unsafe { WaitForInputIdle(p.0, IDLE_WAIT_MS) };
    Ok(())
}

/// True if a module with this file name is loaded in the process.
pub fn module_loaded(pid: u32, name: &str) -> bool {
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) else { return false };
        let snap = Handle(snap);
        let mut me = MODULEENTRY32W { dwSize: std::mem::size_of::<MODULEENTRY32W>() as u32, ..Default::default() };
        let mut ok = Module32FirstW(snap.0, &mut me).is_ok();
        while ok {
            let len = me.szModule.iter().position(|&c| c == 0).unwrap_or(me.szModule.len());
            if String::from_utf16_lossy(&me.szModule[..len]).eq_ignore_ascii_case(name) {
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

pub fn inject(pid: u32, dll: &Path) -> Result<(), InjectError> {
    let fail = |m: &str| InjectError::Failed(m.to_string());
    let process = open(pid)?;

    let mut path: Vec<u16> = dll.as_os_str().encode_wide().collect();
    path.push(0);
    let bytes = path.len() * 2;

    unsafe {
        let load_library = GetModuleHandleW(w!("kernel32.dll"))
            .ok()
            .and_then(|k| GetProcAddress(k, s!("LoadLibraryW")))
            .ok_or_else(|| fail("LoadLibraryW not found"))?;

        let remote = VirtualAllocEx(process.0, None, bytes, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
        if remote.is_null() {
            return Err(fail("VirtualAllocEx failed"));
        }
        let free = || { let _ = VirtualFreeEx(process.0, remote, 0, MEM_RELEASE); };

        if let Err(e) = WriteProcessMemory(process.0, remote, path.as_ptr() as *const c_void, bytes, None) {
            free();
            return Err(InjectError::Failed(format!("WriteProcessMemory failed: {e}")));
        }

        let thread = match CreateRemoteThread(
            process.0,
            None,
            0,
            Some(std::mem::transmute::<_, unsafe extern "system" fn(*mut c_void) -> u32>(load_library)),
            Some(remote),
            0,
            None,
        ) {
            Ok(t) => Handle(t),
            Err(e) => {
                free();
                return Err(InjectError::Failed(format!("CreateRemoteThread failed: {e}")));
            }
        };

        if WaitForSingleObject(thread.0, LOAD_WAIT_MS) != WAIT_OBJECT_0 {
            // thread may still be reading `remote`; leak the few bytes rather than free under it
            return Err(InjectError::Timeout);
        }
        let mut code = 0u32;
        let got = GetExitCodeThread(thread.0, &mut code);
        free();
        match got {
            Ok(()) if code != 0 => Ok(()),
            // the exit code is only the low 32 bits of the module handle, which can
            // legitimately be 0 for a 4 GB-aligned base: confirm by looking for the module
            Ok(()) if dll.file_name().and_then(|n| n.to_str()).is_some_and(|n| module_loaded(pid, n)) => Ok(()),
            Ok(()) => Err(fail("LoadLibrary returned 0 (the overlay DLL failed to load)")),
            Err(e) => Err(InjectError::Failed(format!("GetExitCodeThread failed: {e}"))),
        }
    }
}
