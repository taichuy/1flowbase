mod environment;
use super::{collect, Cancellation, Collected, Spool};
use std::{
    ffi::OsString,
    fs::File,
    io,
    mem::{size_of, size_of_val, zeroed},
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle, OwnedHandle},
    },
    ptr::{null, null_mut},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::{SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT, WAIT_FAILED, WAIT_OBJECT_0},
    Security::SECURITY_ATTRIBUTES,
    System::{
        JobObjects::*, Pipes::CreatePipe, SystemInformation::GetSystemDirectoryW, Threading::*,
    },
};

fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
    value.encode_wide().chain(Some(0)).collect()
}
fn handle(value: HANDLE) -> io::Result<OwnedHandle> {
    if value.is_null() || value as isize == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { OwnedHandle::from_raw_handle(value) })
    }
}
fn check(success: i32) -> io::Result<()> {
    if success == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
fn pipe() -> io::Result<(OwnedHandle, OwnedHandle)> {
    let mut read = null_mut();
    let mut write = null_mut();
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: null_mut(),
        bInheritHandle: 1,
    };
    check(unsafe { CreatePipe(&mut read, &mut write, &attributes, 0) })?;
    let read = handle(read)?;
    let write = handle(write)?;
    Ok((read, write))
}
struct Job {
    handle: OwnedHandle,
    stopped: AtomicBool,
}
impl Job {
    fn new() -> io::Result<Self> {
        let job = handle(unsafe { CreateJobObjectW(null(), null()) })?;
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        check(unsafe {
            SetInformationJobObject(
                job.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                size_of_val(&limits) as u32,
            )
        })?;
        Ok(Self {
            handle: job,
            stopped: AtomicBool::new(false),
        })
    }
    fn stop(&self) -> io::Result<()> {
        if self.stopped.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        check(unsafe { TerminateJobObject(self.handle.as_raw_handle(), 1) })
    }
    fn wait_empty(&self) -> io::Result<()> {
        loop {
            let mut info: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { zeroed() };
            check(unsafe {
                QueryInformationJobObject(
                    self.handle.as_raw_handle(),
                    JobObjectBasicAccountingInformation,
                    (&mut info as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                    size_of_val(&info) as u32,
                    null_mut(),
                )
            })?;
            if info.ActiveProcesses == 0 {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

// Stable Win32 startup attributes: only our three stdio handles are inherited, and
// the process belongs to our Job before any user code can create descendants.
struct Attributes {
    storage: Vec<usize>,
}
impl Attributes {
    fn new() -> io::Result<Self> {
        let mut bytes = 0;
        unsafe {
            InitializeProcThreadAttributeList(null_mut(), 2, 0, &mut bytes);
        }
        if bytes == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut storage = vec![0usize; bytes.div_ceil(size_of::<usize>())];
        check(unsafe {
            InitializeProcThreadAttributeList(storage.as_mut_ptr().cast(), 2, 0, &mut bytes)
        })?;
        let value = Self { storage };
        Ok(value)
    }
    fn as_ptr(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.storage.as_mut_ptr().cast()
    }
    fn set(&mut self, key: u32, value: &mut [HANDLE]) -> io::Result<()> {
        check(unsafe {
            UpdateProcThreadAttribute(
                self.as_ptr(),
                0,
                key as usize,
                value.as_mut_ptr().cast(),
                std::mem::size_of_val(value),
                null_mut(),
                null(),
            )
        })
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        unsafe {
            DeleteProcThreadAttributeList(self.as_ptr());
        }
    }
}

fn cmd_path() -> io::Result<OsString> {
    let mut buffer = vec![0u16; 260];
    loop {
        let n = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) };
        if n == 0 {
            return Err(io::Error::last_os_error());
        }
        if n < buffer.len() as u32 {
            use std::os::windows::ffi::OsStringExt;
            let mut value = OsString::from_wide(&buffer[..n as usize]);
            value.push("\\cmd.exe");
            return Ok(value);
        }
        buffer.resize(n as usize + 1, 0);
    }
}

pub(super) fn run(
    command: &str,
    stdout: Spool,
    stderr: Spool,
    cancellation: &Cancellation,
) -> io::Result<Collected> {
    let job = Job::new()?;
    let (out_read, out_write) = pipe()?;
    let (err_read, err_write) = pipe()?;
    let (in_read, in_write) = pipe()?;
    for value in [&out_read, &err_read, &in_write] {
        check(unsafe { SetHandleInformation(value.as_raw_handle(), HANDLE_FLAG_INHERIT, 0) })?;
    }
    let mut attributes = Attributes::new()?;
    let mut inherited = [
        in_read.as_raw_handle(),
        out_write.as_raw_handle(),
        err_write.as_raw_handle(),
    ];
    let mut jobs = [job.handle.as_raw_handle()];
    attributes.set(PROC_THREAD_ATTRIBUTE_HANDLE_LIST, &mut inherited)?;
    attributes.set(PROC_THREAD_ATTRIBUTE_JOB_LIST, &mut jobs)?;
    let executable = cmd_path()?;
    let executable_wide = wide(&executable);
    // cmd caches a redirected-output code page at startup. Initialize a private
    // console first, then start the user interpreter with that UTF-8 code page.
    // Delayed expansion inserts the command only after the bootstrap has parsed
    // its own metacharacters; %, !, ^ and quotes are interpreted once by the inner cmd.
    let (environment, shell_key, command_key) = environment::with_command(&executable, command)?;
    let command_line = format!(
        "\"{}\" /D /V:ON /S /C \"chcp 65001>nul && \"!{}!\" /D /V:OFF /S /C \"!{}!\"\"",
        executable.to_string_lossy(),
        shell_key,
        command_key,
    );
    let mut command_line = wide(std::ffi::OsStr::new(&command_line));
    let mut startup: STARTUPINFOEXW = unsafe { zeroed() };
    startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES | STARTF_USESHOWWINDOW;
    startup.StartupInfo.wShowWindow = 0; // SW_HIDE
    startup.StartupInfo.hStdInput = in_read.as_raw_handle();
    startup.StartupInfo.hStdOutput = out_write.as_raw_handle();
    startup.StartupInfo.hStdError = err_write.as_raw_handle();
    startup.lpAttributeList = attributes.as_ptr();
    let mut info: PROCESS_INFORMATION = unsafe { zeroed() };
    check(unsafe {
        CreateProcessW(
            executable_wide.as_ptr(),
            command_line.as_mut_ptr(),
            null(),
            null(),
            1,
            EXTENDED_STARTUPINFO_PRESENT | CREATE_NEW_CONSOLE | CREATE_UNICODE_ENVIRONMENT,
            environment.as_ptr().cast(),
            null(),
            &startup.StartupInfo,
            &mut info,
        )
    })?;
    let process = handle(info.hProcess)?;
    let thread = handle(info.hThread)?;
    drop(thread);
    drop(attributes);
    drop(in_read);
    drop(in_write);
    drop(out_write);
    drop(err_write);
    let mut result = collect(
        File::from(out_read),
        File::from(err_read),
        stdout,
        stderr,
        &|| job.stop(),
        || loop {
            if cancellation.is_cancelled() {
                job.stop()?;
            }
            match unsafe { WaitForSingleObject(process.as_raw_handle(), 10) } {
                WAIT_OBJECT_0 => {
                    let mut code = 0;
                    check(unsafe { GetExitCodeProcess(process.as_raw_handle(), &mut code) })?;
                    return Ok((Some(i64::from(code)), None));
                }
                WAIT_FAILED => return Err(io::Error::last_os_error()),
                _ => {}
            }
        },
    )?;
    job.wait_empty()?;
    result.cleanup_complete = true;
    Ok(result)
}
