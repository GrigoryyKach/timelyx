//! A tiny request/response channel between the two Timelyx processes (the tracker and the window).
//!
//! One line in, one line out. On Windows the endpoint is a named pipe, on Linux a Unix socket. Creating the
//! endpoint doubles as the single-instance lock: `serve` fails when another process of the same role (and the
//! same data folder) already serves it. A test instance with its own `TIMELYX_DATA_DIR` never meets the real one.

use std::time::{Duration, Instant};

/// Stable per-data-folder suffix, so different data folders (and users) never share an endpoint.
fn suffix() -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    crate::config::data_dir().to_string_lossy().to_lowercase().hash(&mut h);
    format!("{:x}", h.finish())
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_PIPE_BUSY, ERROR_PIPE_CONNECTED, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{FlushFileBuffers, ReadFile, WriteFile, FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX};
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
    };

    fn pipe_name(role: &str) -> String {
        format!(r"\\.\pipe\Timelyx.{role}.{}", suffix())
    }

    fn wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    pub fn serve(role: &str, handler: impl Fn(&str) -> String + Send + 'static) -> bool {
        let name = wide(&pipe_name(role));
        // SAFETY: plain Win32 call with a NUL-terminated name.
        let h = unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                1,
                8192,
                8192,
                0,
                std::ptr::null(),
            )
        };
        if h == INVALID_HANDLE_VALUE || h.is_null() {
            return false;
        }
        let h = h as isize;
        std::thread::Builder::new()
            .name(format!("timelyx-ipc-{role}"))
            .spawn(move || {
                let h = h as HANDLE;
                loop {
                    // SAFETY: `h` is a pipe handle that lives as long as the process.
                    unsafe {
                        let ok = ConnectNamedPipe(h, std::ptr::null_mut()) != 0 || GetLastError() == ERROR_PIPE_CONNECTED;
                        if ok {
                            if let Some(line) = read_line(h) {
                                let mut reply = handler(line.trim());
                                reply.push('\n');
                                let mut written = 0u32;
                                WriteFile(h, reply.as_ptr(), reply.len() as u32, &mut written, std::ptr::null_mut());
                                FlushFileBuffers(h);
                            }
                        }
                        DisconnectNamedPipe(h);
                    }
                }
            })
            .is_ok()
    }

    unsafe fn read_line(h: HANDLE) -> Option<String> {
        let mut out = Vec::new();
        let mut buf = [0u8; 512];
        loop {
            let mut n = 0u32;
            // SAFETY: the buffer is valid for `buf.len()` bytes.
            let ok = unsafe { ReadFile(h, buf.as_mut_ptr(), buf.len() as u32, &mut n, std::ptr::null_mut()) };
            if ok == 0 || n == 0 {
                return if out.is_empty() { None } else { Some(String::from_utf8_lossy(&out).into_owned()) };
            }
            out.extend_from_slice(&buf[..n as usize]);
            if out.contains(&b'\n') || out.len() > 16_384 {
                let s = String::from_utf8_lossy(&out).into_owned();
                return Some(s.lines().next().unwrap_or("").to_string());
            }
        }
    }

    pub fn request(role: &str, msg: &str, timeout: Duration) -> Option<String> {
        use std::io::{BufRead, BufReader, Write};
        let path = pipe_name(role);
        let end = Instant::now() + timeout;
        let mut file = loop {
            match std::fs::OpenOptions::new().read(true).write(true).open(&path) {
                Ok(f) => break f,
                Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY as i32) && Instant::now() < end => {
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(_) => return None,
            }
        };
        file.write_all(format!("{msg}\n").as_bytes()).ok()?;
        let mut line = String::new();
        BufReader::new(&mut file).read_line(&mut line).ok()?;
        Some(line.trim().to_string())
    }

    #[allow(dead_code)]
    fn _unused(h: HANDLE) {
        unsafe { CloseHandle(h) };
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::PathBuf;

    fn socket_path(role: &str) -> PathBuf {
        let base = std::env::var_os("XDG_RUNTIME_DIR")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            // SAFETY: getuid has no preconditions.
            .unwrap_or_else(|| PathBuf::from(format!("/tmp/timelyx-{}", unsafe { libc::getuid() })));
        let _ = std::fs::create_dir_all(&base);
        base.join(format!("timelyx-{role}-{}.sock", suffix()))
    }

    pub fn serve(role: &str, handler: impl Fn(&str) -> String + Send + 'static) -> bool {
        let path = socket_path(role);
        if UnixStream::connect(&path).is_ok() {
            return false; // somebody answers: already running
        }
        let _ = std::fs::remove_file(&path); // a stale file from a crash
        let Ok(listener) = UnixListener::bind(&path) else { return false };
        std::thread::Builder::new()
            .name(format!("timelyx-ipc-{role}"))
            .spawn(move || {
                for conn in listener.incoming().flatten() {
                    let mut line = String::new();
                    let mut reader = BufReader::new(&conn);
                    if reader.read_line(&mut line).is_ok() {
                        let mut reply = handler(line.trim());
                        reply.push('\n');
                        let mut w = &conn;
                        let _ = w.write_all(reply.as_bytes());
                    }
                }
            })
            .is_ok()
    }

    pub fn request(role: &str, msg: &str, timeout: Duration) -> Option<String> {
        let mut s = UnixStream::connect(socket_path(role)).ok()?;
        let _ = s.set_read_timeout(Some(timeout));
        s.write_all(format!("{msg}\n").as_bytes()).ok()?;
        let mut line = String::new();
        BufReader::new(&s).read_line(&mut line).ok()?;
        Some(line.trim().to_string())
    }
}

/// Starts serving `role` on a background thread. `false` when the role is already served by another process.
pub fn serve(role: &str, handler: impl Fn(&str) -> String + Send + 'static) -> bool {
    imp::serve(role, handler)
}

/// Sends one line and returns the reply line; `None` when nobody serves the role (or it did not answer in time).
pub fn request(role: &str, msg: &str) -> Option<String> {
    imp::request(role, msg, Duration::from_secs(2))
}

/// Is a process serving this role right now?
pub fn is_running(role: &str) -> bool {
    request(role, "ping").is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serve_request_and_single_instance() {
        // The endpoint name hashes the data folder, which other tests change: hold the shared lock.
        let _env = crate::config::ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // A role name unique to this test run, so parallel tests and real instances never collide.
        let role = format!("test{}", std::process::id());
        assert!(request(&role, "ping").is_none());
        assert!(serve(&role, |m| format!("echo:{m}")));
        assert_eq!(request(&role, "hello").as_deref(), Some("echo:hello"));
        assert_eq!(request(&role, "a b c").as_deref(), Some("echo:a b c"));
        assert!(is_running(&role));
        // The same role cannot be served twice.
        assert!(!serve(&role, |_| String::new()));
    }
}
