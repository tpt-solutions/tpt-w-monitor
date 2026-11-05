// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 TPT Solutions

//! Thin, safe wrappers over the Win32 and NT calls the backend needs. This is the
//! only module that contains `unsafe`. Each block states the invariant it relies on.

#![allow(non_snake_case)]

use std::collections::HashSet;
use std::ffi::c_void;
use std::mem::size_of;

use crate::net::TcpRow;
use tpt_monitor_core::{MonitorError, Pid};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER, FILETIME, GetLastError, HANDLE,
    HWND, INVALID_HANDLE_VALUE, LPARAM,
};
use windows_sys::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetExtendedTcpTable, GetIfTable2, GetPerTcpConnectionEStats, MIB_IF_ROW2,
    MIB_IF_TABLE2, MIB_TCPROW_LH, SetPerTcpConnectionEStats, TCP_ESTATS_DATA_ROD_v0,
    TCP_ESTATS_DATA_RW_v0, TCP_TABLE_OWNER_PID_ALL, TcpConnectionEstatsData,
};
use windows_sys::Win32::Networking::WinSock::AF_INET;
use windows_sys::Win32::Security::{
    GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Performance::{
    PDH_CSTATUS_NEW_DATA, PDH_CSTATUS_VALID_DATA, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE,
    PDH_HCOUNTER, PDH_HQUERY, PDH_MORE_DATA, PdhAddEnglishCounterW, PdhCloseQuery,
    PdhCollectQueryData, PdhGetFormattedCounterArrayW, PdhOpenQueryW,
};
use windows_sys::Win32::System::ProcessStatus::{
    K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX,
};
use windows_sys::Win32::System::SystemInformation::GetTickCount64;
use windows_sys::Win32::System::SystemInformation::{
    GetSystemInfo, GlobalMemoryStatusEx, MEMORYSTATUSEX, SYSTEM_INFO,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows_sys::Win32::System::Threading::{
    GetProcessIoCounters, GetProcessTimes, IO_COUNTERS, OpenProcess, PROCESS_QUERY_INFORMATION,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION, PROCESS_SUSPEND_RESUME,
    PROCESS_TERMINATE, PROCESS_VM_READ, SetPriorityClass, SetProcessAffinityMask, TerminateProcess,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
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

/// `ERROR_INSUFFICIENT_BUFFER`: the table grew between the size query and the fetch.
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;

/// The raw IPv4 owner-PID TCP table, as `GetExtendedTcpTable` returns it.
pub fn tcp_owner_table() -> Result<Vec<u8>, MonitorError> {
    let mut size: u32 = 0;
    // SAFETY: a null buffer with size 0 asks the API for the size it needs.
    let rc = unsafe {
        GetExtendedTcpTable(
            std::ptr::null_mut(),
            &mut size,
            0,
            u32::from(AF_INET),
            TCP_TABLE_OWNER_PID_ALL,
            0,
        )
    };
    if rc != ERROR_INSUFFICIENT_BUFFER && rc != 0 {
        return Err(MonitorError::Os(format!("GetExtendedTcpTable size query: {rc}")));
    }
    // The table can grow between calls, so retry a few times before giving up.
    for _ in 0..3 {
        let mut buf = vec![0u8; size as usize];
        // SAFETY: `buf` is `size` bytes long, and the API writes at most `size` bytes.
        let rc = unsafe {
            GetExtendedTcpTable(
                buf.as_mut_ptr().cast(),
                &mut size,
                0,
                u32::from(AF_INET),
                TCP_TABLE_OWNER_PID_ALL,
                0,
            )
        };
        if rc == 0 {
            buf.truncate(size as usize);
            return Ok(buf);
        }
        if rc != ERROR_INSUFFICIENT_BUFFER {
            return Err(MonitorError::Os(format!("GetExtendedTcpTable: {rc}")));
        }
    }
    Err(MonitorError::Os("TCP table kept growing".into()))
}

fn to_lh(row: &TcpRow) -> MIB_TCPROW_LH {
    let mut lh = MIB_TCPROW_LH::default();
    lh.Anonymous.dwState = row.state;
    lh.dwLocalAddr = row.local_addr;
    lh.dwLocalPort = u32::from(row.local_port);
    lh.dwRemoteAddr = row.remote_addr;
    lh.dwRemotePort = u32::from(row.remote_port);
    lh
}

/// Turns on byte-counter collection for one connection. Requires elevation; without it
/// the call fails with access denied and the caller reports traffic as unavailable.
pub fn enable_estats(row: &TcpRow) -> Result<(), MonitorError> {
    let lh = to_lh(row);
    let rw = TCP_ESTATS_DATA_RW_v0 { EnableCollection: true };
    // SAFETY: `lh` and `rw` are valid for the call; the sizes match the structs passed.
    let rc = unsafe {
        SetPerTcpConnectionEStats(
            &lh,
            TcpConnectionEstatsData,
            (&rw as *const TCP_ESTATS_DATA_RW_v0).cast(),
            0,
            size_of::<TCP_ESTATS_DATA_RW_v0>() as u32,
            0,
        )
    };
    if rc == 0 {
        Ok(())
    } else if rc == ERROR_ACCESS_DENIED {
        Err(MonitorError::PermissionDenied(Pid(row.pid)))
    } else {
        Err(MonitorError::Os(format!("SetPerTcpConnectionEStats: {rc}")))
    }
}

/// Cumulative `(bytes_received, bytes_sent)` for one connection, once collection is on.
pub fn estats_counters(row: &TcpRow) -> Option<(u64, u64)> {
    let lh = to_lh(row);
    let mut rod = TCP_ESTATS_DATA_ROD_v0::default();
    // SAFETY: `rod` is a valid out-buffer of the size passed; the read-write and
    // read-only-state arguments are null with size 0, which the API allows for a
    // read-only query.
    let rc = unsafe {
        GetPerTcpConnectionEStats(
            &lh,
            TcpConnectionEstatsData,
            std::ptr::null_mut(),
            0,
            0,
            std::ptr::null_mut(),
            0,
            0,
            (&mut rod as *mut TCP_ESTATS_DATA_ROD_v0).cast(),
            0,
            size_of::<TCP_ESTATS_DATA_ROD_v0>() as u32,
        )
    };
    (rc == 0).then_some((rod.DataBytesIn, rod.DataBytesOut))
}

/// `(instance name, value)` pairs from a wildcard PDH counter.
pub type CounterItems = Vec<(String, f64)>;

/// The GPU engine and dedicated-memory counters, opened once and sampled each interval.
pub struct GpuCounters {
    query: PDH_HQUERY,
    engine: PDH_HCOUNTER,
    memory: PDH_HCOUNTER,
}

// SAFETY: PDH query handles are plain pointers with no thread affinity. The struct is
// only used through `&mut self`, so there is never concurrent access to the query.
unsafe impl Send for GpuCounters {}

impl Drop for GpuCounters {
    fn drop(&mut self) {
        // SAFETY: the query handle was opened by PdhOpenQueryW and is closed once here.
        unsafe {
            PdhCloseQuery(self.query);
        }
    }
}

fn wide_nul(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn add_english_counter(query: PDH_HQUERY, path: &str) -> Result<PDH_HCOUNTER, MonitorError> {
    let mut counter: PDH_HCOUNTER = std::ptr::null_mut();
    let path = wide_nul(path);
    // SAFETY: `path` is a valid NUL-terminated wide string; `counter` is a valid out-slot.
    let rc = unsafe { PdhAddEnglishCounterW(query, path.as_ptr(), 0, &mut counter) };
    if rc != 0 || counter.is_null() {
        return Err(MonitorError::Unsupported("GPU performance counters"));
    }
    Ok(counter)
}

/// Opens the GPU engine and dedicated-memory counters. Machines without a WDDM GPU
/// driver lack these counters and report `Unsupported`.
pub fn open_gpu_counters() -> Result<GpuCounters, MonitorError> {
    let mut query: PDH_HQUERY = std::ptr::null_mut();
    // SAFETY: a null data source and zero user data are the documented defaults.
    let rc = unsafe { PdhOpenQueryW(std::ptr::null(), 0, &mut query) };
    if rc != 0 {
        return Err(MonitorError::Unsupported("GPU performance counters"));
    }
    // Owning the query from here means any failure below closes it through Drop.
    let mut gpu = GpuCounters { query, engine: std::ptr::null_mut(), memory: std::ptr::null_mut() };
    gpu.engine = add_english_counter(query, r"\GPU Engine(*)\Utilization Percentage")?;
    gpu.memory = add_english_counter(query, r"\GPU Process Memory(*)\Dedicated Usage")?;
    Ok(gpu)
}

impl GpuCounters {
    /// Collects one sample. Returns `(engine_items, memory_items)` as
    /// `(instance name, value)` pairs, or `None` if the collection failed.
    pub fn sample(&mut self) -> Option<(CounterItems, CounterItems)> {
        // SAFETY: the query handle is valid for the life of `self`.
        if unsafe { PdhCollectQueryData(self.query) } != 0 {
            return None;
        }
        Some((formatted_items(self.engine)?, formatted_items(self.memory)?))
    }
}

/// Reads every instance of a wildcard counter as doubles, skipping invalid values.
fn formatted_items(counter: PDH_HCOUNTER) -> Option<CounterItems> {
    let mut size: u32 = 0;
    let mut count: u32 = 0;
    // SAFETY: a null buffer asks for the required size.
    let rc = unsafe {
        PdhGetFormattedCounterArrayW(
            counter,
            PDH_FMT_DOUBLE,
            &mut size,
            &mut count,
            std::ptr::null_mut(),
        )
    };
    if rc == 0 && size == 0 {
        return Some(Vec::new());
    }
    if rc != PDH_MORE_DATA {
        return None;
    }
    // The item array holds pointers, so the buffer must be 8-byte aligned: use u64 words.
    let mut buf = vec![0u64; (size as usize).div_ceil(8).max(1)];
    // SAFETY: `buf` provides `size` writable bytes, 8-byte aligned as the items require.
    let rc = unsafe {
        PdhGetFormattedCounterArrayW(
            counter,
            PDH_FMT_DOUBLE,
            &mut size,
            &mut count,
            buf.as_mut_ptr().cast(),
        )
    };
    if rc != 0 {
        return None;
    }
    // SAFETY: on success PDH wrote `count` item records at the start of `buf`; their name
    // pointers refer into the same buffer, which stays alive until this function returns.
    let items = unsafe {
        std::slice::from_raw_parts(
            buf.as_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>(),
            count as usize,
        )
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let status = item.FmtValue.CStatus;
        if status != PDH_CSTATUS_VALID_DATA && status != PDH_CSTATUS_NEW_DATA {
            continue;
        }
        // SAFETY: with PDH_FMT_DOUBLE the value union holds a double.
        let value = unsafe { item.FmtValue.Anonymous.doubleValue };
        out.push((wide_ptr_to_string(item.szName), value));
    }
    Some(out)
}

fn wide_ptr_to_string(p: *const u16) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    // SAFETY: PDH instance names are NUL-terminated and live in the buffer read above.
    unsafe {
        while *p.add(len) != 0 {
            len += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
    }
}

/// Seconds since the last keyboard or mouse input, from `GetLastInputInfo`.
pub fn user_idle_seconds() -> Option<u64> {
    let mut info = LASTINPUTINFO { cbSize: size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
    // SAFETY: `info` is a valid LASTINPUTINFO whose cbSize is set as the API requires.
    if unsafe { GetLastInputInfo(&mut info) } == 0 {
        return None;
    }
    // SAFETY: GetTickCount64 takes no arguments and has no preconditions.
    let now = unsafe { GetTickCount64() };
    // The input timestamp is a 32-bit tick count that wraps every 49.7 days, so the
    // difference is taken in 32-bit arithmetic.
    let elapsed_ms = (now as u32).wrapping_sub(info.dwTime);
    Some(u64::from(elapsed_ms / 1000))
}

/// Whether this process runs elevated (administrator token). Most per-process metrics and
/// actions on Windows depend on it.
pub fn is_elevated() -> bool {
    let mut token: HANDLE = std::ptr::null_mut();
    // SAFETY: the current-process pseudo-handle needs no closing; `token` is a valid out-slot.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return false;
    }
    let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
    let mut returned: u32 = 0;
    // SAFETY: `elevation` is a valid TOKEN_ELEVATION of the size passed; `token` is open.
    let ok = unsafe {
        GetTokenInformation(
            token,
            TokenElevation,
            (&mut elevation as *mut TOKEN_ELEVATION).cast(),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
    };
    // SAFETY: the token handle was opened above and is closed exactly once here.
    unsafe {
        CloseHandle(token);
    }
    ok != 0 && elevation.TokenIsElevated != 0
}

/// Cumulative `(interface type, in octets, out octets)` for every network interface.
pub fn interface_octets() -> Result<Vec<(u32, u64, u64)>, MonitorError> {
    let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
    // SAFETY: on success GetIfTable2 allocates the table and stores its pointer in `table`.
    let rc = unsafe { GetIfTable2(&mut table) };
    if rc != 0 || table.is_null() {
        return Err(MonitorError::Os(format!("GetIfTable2: {rc}")));
    }
    // SAFETY: the table holds NumEntries rows starting at `Table`; they stay valid until
    // FreeMibTable, which runs below after the rows are copied out.
    let rows: Vec<(u32, u64, u64)> = unsafe {
        let count = (*table).NumEntries as usize;
        let first = std::ptr::addr_of!((*table).Table).cast::<MIB_IF_ROW2>();
        std::slice::from_raw_parts(first, count)
            .iter()
            .map(|row| (row.Type, row.InOctets, row.OutOctets))
            .collect()
    };
    // SAFETY: `table` came from GetIfTable2 and is freed exactly once.
    unsafe {
        FreeMibTable(table.cast());
    }
    Ok(rows)
}
