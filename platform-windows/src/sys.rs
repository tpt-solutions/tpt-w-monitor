// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Thin, safe wrappers over the Win32 and NT calls the backend needs. This is the
//! only module that contains `unsafe`. Each block states the invariant it relies on.

#![allow(non_snake_case)]

use std::collections::HashSet;
use std::ffi::c_void;
use std::mem::size_of;

use tpt_monitor_core::{MonitorError, Pid};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER, FILETIME, GetLastError, HANDLE,
    HWND, INVALID_HANDLE_VALUE, LPARAM,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::ProcessStatus::{
    K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX,
};
use windows_sys::Win32::System::SystemInformation::{
    GetSystemInfo, GlobalMemoryStatusEx, MEMORYSTATUSEX, SYSTEM_INFO,
};
use windows_sys::Win32::System::Threading::{
    GetProcessIoCounters, GetProcessTimes, IO_COUNTERS, OpenProcess, PROCESS_QUERY_INFORMATION,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION, PROCESS_SUSPEND_RESUME,
    PROCESS_TERMINATE, PROCESS_VM_READ, SetPriorityClass, SetProcessAffinityMask, TerminateProcess,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, IsWindowVisible,
};

/// Priority class constants (`winbase.h`), defined here so the mapping is explicit.
pub const IDLE_PRIORITY_CLASS: u32 = 0x0000_0040;
/// See [`IDLE_PRIORITY_CLASS`].
pub const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
/// See [`IDLE_PRIORITY_CLASS`].
pub const NORMAL_PRIORITY_CLASS: u32 = 0x0000_0020;
/// See [`IDLE_PRIORITY_CLASS`].
pub const ABOVE_NORMAL_PRIORITY_CLASS: u32 = 0x0000_8000;
/// See [`IDLE_PRIORITY_CLASS`].
pub const HIGH_PRIORITY_CLASS: u32 = 0x0000_0080;

#[link(name = "ntdll")]
unsafe extern "system" {
    fn NtSuspendProcess(process: HANDLE) -> i32;
    fn NtResumeProcess(process: HANDLE) -> i32;
    fn NtQuerySystemInformation(
        class: u32,
        information: *mut c_void,
        length: u32,
        return_length: *mut u32,
    ) -> i32;
}

/// `SystemProcessorPerformanceInformation`, one record per logical processor.
const SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION: u32 = 8;

/// Layout of `SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION`. Times are 100 ns units;
/// `kernel` includes `idle`.
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct ProcessorPerformance {
    idle: i64,
    kernel: i64,
    user: i64,
    dpc: i64,
    interrupt: i64,
    interrupt_count: u32,
}

/// Owned process handle; closed on drop.
pub struct ProcessHandle {
    raw: HANDLE,
    pid: Pid,
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        // SAFETY: `raw` was returned by OpenProcess and is closed exactly once here.
        unsafe {
            CloseHandle(self.raw);
        }
    }
}

/// Maps the calling thread's last Win32 error to a [`MonitorError`].
fn last_error(pid: Pid) -> MonitorError {
    // SAFETY: GetLastError has no preconditions.
    let code = unsafe { GetLastError() };
    match code {
        ERROR_ACCESS_DENIED => MonitorError::PermissionDenied(pid),
        ERROR_INVALID_PARAMETER => MonitorError::ProcessNotFound(pid),
        other => MonitorError::Os(format!("win32 error {other} for pid {pid}")),
    }
}

/// Opens a process with the given access rights.
pub fn open(pid: Pid, access: u32) -> Result<ProcessHandle, MonitorError> {
    // SAFETY: OpenProcess takes plain values and returns a handle or null.
    let raw = unsafe { OpenProcess(access, 0, pid.0) };
    if raw.is_null() {
        return Err(last_error(pid));
    }
    Ok(ProcessHandle { raw, pid })
}

/// Opens with the richest rights available: full query first, limited query as fallback.
/// Returns `None` when even the limited right is denied (e.g. protected processes).
pub fn open_for_query(pid: Pid) -> Option<ProcessHandle> {
    open(pid, PROCESS_QUERY_INFORMATION | PROCESS_VM_READ)
        .or_else(|_| open(pid, PROCESS_QUERY_LIMITED_INFORMATION))
        .ok()
}

fn filetime_to_u64(ft: FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
}

/// `(creation_time, kernel_plus_user_ticks)` for a process.
pub fn process_times(h: &ProcessHandle) -> Result<(u64, u64), MonitorError> {
    let zero = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    let (mut creation, mut exit, mut kernel, mut user) = (zero, zero, zero, zero);
    // SAFETY: all out-pointers reference valid, live FILETIME locals.
    let ok = unsafe { GetProcessTimes(h.raw, &mut creation, &mut exit, &mut kernel, &mut user) };
    if ok == 0 {
        return Err(last_error(h.pid));
    }
    Ok((filetime_to_u64(creation), filetime_to_u64(kernel) + filetime_to_u64(user)))
}

/// `(working_set, private, commit)` in bytes.
pub fn memory_usage(h: &ProcessHandle) -> Result<(u64, u64, u64), MonitorError> {
    // SAFETY: zeroed is a valid bit pattern for this plain C struct.
    let mut counters: PROCESS_MEMORY_COUNTERS_EX = unsafe { std::mem::zeroed() };
    counters.cb = size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    // SAFETY: `counters` is valid for `cb` bytes; the cast is to the base struct the
    // API documents for the EX variant.
    let ok = unsafe {
        K32GetProcessMemoryInfo(
            h.raw,
            (&mut counters as *mut PROCESS_MEMORY_COUNTERS_EX).cast(),
            counters.cb,
        )
    };
    if ok == 0 {
        return Err(last_error(h.pid));
    }
    Ok((
        counters.WorkingSetSize as u64,
        counters.PrivateUsage as u64,
        counters.PagefileUsage as u64,
    ))
}

/// `(bytes_read, bytes_written)` of all I/O issued by the process.
pub fn io_counters(h: &ProcessHandle) -> Result<(u64, u64), MonitorError> {
    // SAFETY: zeroed is a valid bit pattern for IO_COUNTERS (plain u64 fields).
    let mut io: IO_COUNTERS = unsafe { std::mem::zeroed() };
    // SAFETY: `io` is a valid out-parameter for the call.
    let ok = unsafe { GetProcessIoCounters(h.raw, &mut io) };
    if ok == 0 {
        return Err(last_error(h.pid));
    }
    Ok((io.ReadTransferCount, io.WriteTransferCount))
}

/// Applies a priority class to a process.
pub fn set_priority(h: &ProcessHandle, class: u32) -> Result<(), MonitorError> {
    // SAFETY: the handle is valid for the lifetime of `h`.
    let ok = unsafe { SetPriorityClass(h.raw, class) };
    if ok == 0 { Err(last_error(h.pid)) } else { Ok(()) }
}

/// Restricts a process to the CPUs in `mask`.
pub fn set_affinity(h: &ProcessHandle, mask: usize) -> Result<(), MonitorError> {
    // SAFETY: the handle is valid for the lifetime of `h`.
    let ok = unsafe { SetProcessAffinityMask(h.raw, mask) };
    if ok == 0 { Err(last_error(h.pid)) } else { Ok(()) }
}

/// Terminates a process with exit code 1.
pub fn terminate(h: &ProcessHandle) -> Result<(), MonitorError> {
    // SAFETY: the handle is valid for the lifetime of `h`.
    let ok = unsafe { TerminateProcess(h.raw, 1) };
    if ok == 0 { Err(last_error(h.pid)) } else { Ok(()) }
}

/// Suspends all threads of a process (`NtSuspendProcess`).
pub fn suspend(h: &ProcessHandle) -> Result<(), MonitorError> {
    // SAFETY: the handle is valid; NTSTATUS >= 0 is success.
    let status = unsafe { NtSuspendProcess(h.raw) };
    nt_result(status, h.pid)
}

/// Resumes a suspended process (`NtResumeProcess`).
pub fn resume(h: &ProcessHandle) -> Result<(), MonitorError> {
    // SAFETY: the handle is valid; NTSTATUS >= 0 is success.
    let status = unsafe { NtResumeProcess(h.raw) };
    nt_result(status, h.pid)
}

fn nt_result(status: i32, pid: Pid) -> Result<(), MonitorError> {
    if status >= 0 {
        Ok(())
    } else if status as u32 == 0xC000_0022 {
        // STATUS_ACCESS_DENIED
        Err(MonitorError::PermissionDenied(pid))
    } else {
        Err(MonitorError::Os(format!("NTSTATUS {:#010x} for pid {pid}", status as u32)))
    }
}

/// Every process as `(pid, executable name)`.
pub fn list_processes() -> Result<Vec<(Pid, String)>, MonitorError> {
    // SAFETY: a process snapshot takes no pointers.
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snap == INVALID_HANDLE_VALUE {
        return Err(MonitorError::Os("CreateToolhelp32Snapshot failed".into()));
    }
    let _guard = SnapshotGuard(snap);

    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
    let mut out = Vec::new();
    // SAFETY: `entry` has dwSize set as the API requires and lives for each call.
    if unsafe { Process32FirstW(snap, &mut entry) } != 0 {
        loop {
            out.push((Pid(entry.th32ProcessID), wide_to_string(&entry.szExeFile)));
            // SAFETY: as above.
            if unsafe { Process32NextW(snap, &mut entry) } == 0 {
                break;
            }
        }
    }
    Ok(out)
}

struct SnapshotGuard(HANDLE);
impl Drop for SnapshotGuard {
    fn drop(&mut self) {
        // SAFETY: the snapshot handle is closed exactly once.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn wide_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

/// PIDs of processes owning a visible top-level window.
pub fn pids_with_visible_windows() -> HashSet<u32> {
    unsafe extern "system" fn callback(hwnd: HWND, lparam: LPARAM) -> i32 {
        // SAFETY: lparam is the `*mut HashSet<u32>` passed to EnumWindows below and
        // remains valid for the duration of that synchronous call.
        unsafe {
            if IsWindowVisible(hwnd) != 0 {
                let mut pid: u32 = 0;
                GetWindowThreadProcessId(hwnd, &mut pid);
                (*(lparam as *mut HashSet<u32>)).insert(pid);
            }
        }
        1
    }
    let mut set: HashSet<u32> = HashSet::new();
    // SAFETY: `set` outlives the synchronous EnumWindows call, which is the only
    // place the callback can run.
    unsafe {
        EnumWindows(Some(callback), &mut set as *mut HashSet<u32> as LPARAM);
    }
    set
}

/// Logical processor count, from `GetSystemInfo`.
pub fn logical_processor_count() -> usize {
    // SAFETY: zeroed is valid for SYSTEM_INFO; the call fills it in.
    let mut info: SYSTEM_INFO = unsafe { std::mem::zeroed() };
    unsafe { GetSystemInfo(&mut info) };
    (info.dwNumberOfProcessors as usize).max(1)
}

/// Cumulative `(total_ticks, idle_ticks)` for each logical processor.
pub fn processor_times(cores: usize) -> Result<Vec<(u64, u64)>, MonitorError> {
    let mut buf = vec![ProcessorPerformance::default(); cores];
    let len = (cores * size_of::<ProcessorPerformance>()) as u32;
    let mut returned: u32 = 0;
    // SAFETY: `buf` holds exactly `len` bytes of writable, properly aligned memory.
    let status = unsafe {
        NtQuerySystemInformation(
            SYSTEM_PROCESSOR_PERFORMANCE_INFORMATION,
            buf.as_mut_ptr().cast(),
            len,
            &mut returned,
        )
    };
    if status < 0 {
        return Err(MonitorError::Os(format!("NtQuerySystemInformation status {status:#x}")));
    }
    Ok(buf
        .iter()
        .map(|p| {
            // kernel time includes idle time; user time is separate.
            let total = (p.kernel + p.user).max(0) as u64;
            (total, p.idle.max(0) as u64)
        })
        .collect())
}

/// `(total_physical, available_physical)` memory in bytes.
pub fn physical_memory() -> Result<(u64, u64), MonitorError> {
    // SAFETY: zeroed is valid for MEMORYSTATUSEX; dwLength is set as required.
    let mut status: MEMORYSTATUSEX = unsafe { std::mem::zeroed() };
    status.dwLength = size_of::<MEMORYSTATUSEX>() as u32;
    // SAFETY: `status` is valid for the call.
    if unsafe { GlobalMemoryStatusEx(&mut status) } == 0 {
        return Err(MonitorError::Os("GlobalMemoryStatusEx failed".into()));
    }
    Ok((status.ullTotalPhys, status.ullAvailPhys))
}

/// Maps a [`tpt_monitor_core::ProcessAction::SetPriority`] class to its Win32 constant.
pub fn priority_class(p: tpt_monitor_core::Priority) -> u32 {
    use tpt_monitor_core::Priority::*;
    match p {
        Idle => IDLE_PRIORITY_CLASS,
        BelowNormal => BELOW_NORMAL_PRIORITY_CLASS,
        Normal => NORMAL_PRIORITY_CLASS,
        AboveNormal => ABOVE_NORMAL_PRIORITY_CLASS,
        High => HIGH_PRIORITY_CLASS,
    }
}

/// Access rights needed for each destructive or control action.
pub const ACCESS_TERMINATE: u32 = PROCESS_TERMINATE;
/// See [`ACCESS_TERMINATE`].
pub const ACCESS_SUSPEND_RESUME: u32 = PROCESS_SUSPEND_RESUME;
/// See [`ACCESS_TERMINATE`].
pub const ACCESS_SET_INFORMATION: u32 = PROCESS_SET_INFORMATION;
