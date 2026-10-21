use anyhow::Result;

pub struct Sample {
    pub resident_bytes: u64,
    pub private_bytes: Option<u64>,
    pub physical_footprint_bytes: Option<u64>,
    pub cpu_time_ns: u64,
}

#[cfg(any(target_os = "macos", test))]
fn ticks_to_ns(ticks: u64, numer: u32, denom: u32) -> u64 {
    ((u128::from(ticks) * u128::from(numer)) / u128::from(denom)).min(u128::from(u64::MAX)) as u64
}

#[cfg(windows)]
pub fn sample(pid: u32) -> Result<Sample> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, FILETIME},
        System::{
            ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX},
            Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
        },
    };
    // SAFETY: query-only access to the caller-supplied owned child process.
    let handle = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid) };
    if handle.is_null() {
        return Err(std::io::Error::last_os_error().into());
    }
    let result = (|| {
        let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
        let mut counters = PROCESS_MEMORY_COUNTERS_EX {
            cb: size,
            ..Default::default()
        };
        let mut created = FILETIME::default();
        let mut exited = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        // SAFETY: structures are initialized and buffers match the EX size;
        // the base pointer is the documented layout-compatible API argument.
        if unsafe {
            GetProcessMemoryInfo(
                handle,
                (&mut counters as *mut PROCESS_MEMORY_COUNTERS_EX).cast(),
                size,
            )
        } == 0
            || unsafe { GetProcessTimes(handle, &mut created, &mut exited, &mut kernel, &mut user) }
                == 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
        let ticks =
            |time: FILETIME| (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
        Ok(Sample {
            resident_bytes: counters.WorkingSetSize as u64,
            private_bytes: Some(counters.PrivateUsage as u64),
            physical_footprint_bytes: None,
            cpu_time_ns: (ticks(kernel) + ticks(user)).saturating_mul(100),
        })
    })();
    // SAFETY: OpenProcess returned this uniquely owned handle.
    unsafe {
        CloseHandle(handle);
    }
    result
}

#[cfg(target_os = "macos")]
pub fn sample(pid: u32) -> Result<Sample> {
    // SAFETY: this versioned struct contains only scalar fields/arrays. libproc
    // writes RUSAGE_INFO_V2 bytes into the correctly sized, initialized buffer.
    let mut usage: libc::rusage_info_v2 = unsafe { std::mem::zeroed() };
    let result = unsafe {
        libc::proc_pid_rusage(
            pid.try_into()?,
            libc::RUSAGE_INFO_V2,
            (&mut usage as *mut libc::rusage_info_v2).cast(),
        )
    };
    if result != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let mut timebase = libc::mach_timebase_info { numer: 0, denom: 0 };
    // SAFETY: points to a live, initialized timebase structure.
    anyhow::ensure!(
        unsafe { libc::mach_timebase_info(&mut timebase) } == 0 && timebase.denom != 0,
        "mach_timebase_info failed"
    );
    Ok(Sample {
        resident_bytes: usage.ri_resident_size,
        private_bytes: None,
        physical_footprint_bytes: Some(usage.ri_phys_footprint),
        cpu_time_ns: ticks_to_ns(
            usage.ri_user_time.saturating_add(usage.ri_system_time),
            timebase.numer,
            timebase.denom,
        ),
    })
}

#[cfg(test)]
#[test]
fn apple_silicon_ticks_are_converted_instead_of_labelled_nanoseconds() {
    assert_eq!(ticks_to_ns(24_000_000, 125, 3), 1_000_000_000);
    assert_eq!(ticks_to_ns(1_000_000_000, 1, 1), 1_000_000_000);
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn sample(_pid: u32) -> Result<Sample> {
    anyhow::bail!("This probe supports Windows and macOS")
}
