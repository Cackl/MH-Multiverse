//! Classic LoadLibraryW remote-thread injection, by PID, with the minimum
//! process rights. Mirrors what the overlay's own launcher does.

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows::core::{s, w};
use windows::Win32::Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0};
use windows::Win32::System::Diagnostics::Debug::WriteProcessMemory;
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
            Ok(()) => Err(fail("LoadLibrary returned 0 (the overlay DLL failed to load)")),
            Err(e) => Err(InjectError::Failed(format!("GetExitCodeThread failed: {e}"))),
        }
    }
}
