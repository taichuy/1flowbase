use std::{cmp::Ordering, ffi::OsStr, io, os::windows::ffi::OsStrExt};
use windows_sys::Win32::{
    Globalization::CompareStringOrdinal,
    System::Environment::{FreeEnvironmentStringsW, GetEnvironmentStringsW},
};

/// Preserve the OS environment (including per-drive =C: entries), adding invocation-local values.
pub(super) fn with_command(shell: &OsStr, command: &str) -> io::Result<(Vec<u16>, String, String)> {
    let token = uuid::Uuid::now_v7().simple().to_string();
    let shell_key = format!("_1FLOWBASE_SHELL_{token}");
    let command_key = format!("_1FLOWBASE_COMMAND_{token}");
    let raw = unsafe { GetEnvironmentStringsW() };
    if raw.is_null() {
        return Err(io::Error::last_os_error());
    }
    struct Environment(*mut u16);
    impl Drop for Environment {
        fn drop(&mut self) {
            unsafe {
                FreeEnvironmentStringsW(self.0);
            }
        }
    }
    let owned = Environment(raw);
    let mut entries = Vec::<Vec<u16>>::new();
    let mut cursor = owned.0;
    unsafe {
        while *cursor != 0 {
            let mut end = cursor;
            while *end != 0 {
                end = end.add(1);
            }
            entries.push(
                std::slice::from_raw_parts(cursor, end.offset_from(cursor) as usize).to_vec(),
            );
            cursor = end.add(1);
        }
    }
    let mut shell_entry: Vec<u16> = format!("{shell_key}=").encode_utf16().collect();
    shell_entry.extend(shell.encode_wide());
    entries.push(shell_entry);
    entries.push(format!("{command_key}={command}").encode_utf16().collect());
    let name_len = |entry: &[u16]| -> i32 {
        // The leading '=' in drive-current-directory entries belongs to the name.
        entry
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, c)| **c == b'=' as u16)
            .map(|(i, _)| i)
            .unwrap_or(entry.len()) as i32
    };
    let mut comparison_failed = false;
    entries.sort_by(|a, b| {
        match unsafe { CompareStringOrdinal(a.as_ptr(), name_len(a), b.as_ptr(), name_len(b), 1) } {
            1 => Ordering::Less,
            2 => Ordering::Equal,
            3 => Ordering::Greater,
            _ => {
                comparison_failed = true;
                Ordering::Equal
            }
        }
    });
    if comparison_failed {
        return Err(io::Error::other(
            "could not order the Windows process environment",
        ));
    }
    let mut block = Vec::new();
    for entry in entries {
        block.extend(entry);
        block.push(0);
    }
    block.push(0);
    Ok((block, shell_key, command_key))
}
