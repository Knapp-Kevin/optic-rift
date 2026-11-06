//! Ties a child process and everything it spawns to Optic Rift's lifetime.
//!
//! The worker is started through the virtual environment's `python.exe` launcher, which starts the
//! real interpreter as a second process; killing the launcher alone leaves the interpreter (and its
//! GPU memory) behind. A Windows Job Object with kill-on-close ends the whole tree when the job is
//! terminated, when this handle is dropped, or when Optic Rift itself exits or crashes.

use std::process::Child;

#[cfg(windows)]
pub struct ProcessTree {
    job: windows_sys::Win32::Foundation::HANDLE,
}

// SAFETY: a job handle may be used and closed from any thread.
#[cfg(windows)]
unsafe impl Send for ProcessTree {}
#[cfg(windows)]
unsafe impl Sync for ProcessTree {}

#[cfg(windows)]
impl ProcessTree {
    /// Place `child` (and every process it later creates) in a new kill-on-close job.
    pub fn adopt(child: &Child) -> Result<Self, String> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        // SAFETY: plain Win32 calls with valid arguments; the handle is owned by the returned value.
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return Err("Could not create a job object for the worker.".into());
            }
            let tree = ProcessTree { job };
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            if ok == 0 {
                return Err("Could not configure the worker job object.".into());
            }
            if AssignProcessToJobObject(job, child.as_raw_handle() as _) == 0 {
                return Err("Could not attach the worker to its job object.".into());
            }
            Ok(tree)
        }
    }

    /// End every process in the tree now.
    pub fn terminate(&self) {
        // SAFETY: valid job handle owned by self.
        unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.job, 1);
        }
    }
}

#[cfg(windows)]
impl Drop for ProcessTree {
    fn drop(&mut self) {
        // Closing the last handle to a kill-on-close job ends every process still in it.
        // SAFETY: valid handle owned by self, closed exactly once.
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.job);
        }
    }
}

#[cfg(not(windows))]
pub struct ProcessTree;

#[cfg(not(windows))]
impl ProcessTree {
    pub fn adopt(_child: &Child) -> Result<Self, String> {
        Ok(ProcessTree)
    }
    pub fn terminate(&self) {}
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    fn alive(pid: u32) -> bool {
        let out = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).contains(&pid.to_string())
    }

    fn grandchild_of(parent: u32) -> Option<u32> {
        let out = Command::new("powershell")
            .args(["-NoProfile", "-Command", &format!("(Get-CimInstance Win32_Process -Filter 'ParentProcessId={parent}' | Where-Object Name -eq 'PING.EXE').ProcessId")])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().parse().ok()
    }

    fn spawn_tree() -> (std::process::Child, ProcessTree, u32) {
        // cmd.exe starts ping.exe, like the venv launcher starts the real interpreter.
        let child = Command::new("cmd")
            .args(["/C", "ping -n 60 127.0.0.1 >NUL"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let tree = ProcessTree::adopt(&child).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let grandchild = loop {
            if let Some(pid) = grandchild_of(child.id()) {
                break pid;
            }
            assert!(Instant::now() < deadline, "grandchild never started");
            std::thread::sleep(Duration::from_millis(100));
        };
        (child, tree, grandchild)
    }

    #[test]
    fn terminate_ends_grandchildren_too() {
        let (mut child, tree, grandchild) = spawn_tree();
        assert!(alive(grandchild));
        tree.terminate();
        std::thread::sleep(Duration::from_millis(500));
        assert!(child.try_wait().unwrap().is_some());
        assert!(!alive(grandchild), "grandchild survived terminate");
    }

    #[test]
    fn dropping_the_handle_ends_the_tree() {
        let (mut child, tree, grandchild) = spawn_tree();
        drop(tree);
        std::thread::sleep(Duration::from_millis(500));
        assert!(child.try_wait().unwrap().is_some());
        assert!(!alive(grandchild), "grandchild survived handle close");
    }

    #[test]
    fn killing_only_the_direct_child_would_orphan_the_grandchild() {
        // Documents the defect this module exists to prevent.
        let (mut child, tree, grandchild) = spawn_tree();
        child.kill().unwrap();
        child.wait().unwrap();
        std::thread::sleep(Duration::from_millis(300));
        assert!(
            alive(grandchild),
            "expected the grandchild to outlive a plain kill"
        );
        tree.terminate();
    }
}
