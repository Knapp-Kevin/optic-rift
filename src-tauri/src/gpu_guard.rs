//! GPU sharing guard. The GPU belongs to the whole computer: games and other apps may be using it.
//! Before Optic Rift loads a model or generates, it takes a per-process snapshot of GPU use (Windows
//! GPU performance counters: dedicated VRAM and 3D engine utilisation per process) and decides:
//! proceed within a VRAM budget that leaves headroom, ask the user first because another app is
//! actively using the GPU, or refuse because it would not fit without squeezing other apps.

use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};

/// Always leave at least this much VRAM free for the rest of the system.
pub const RESERVE_MIN_MB: u64 = 1536;
/// ...or this fraction of total VRAM, whichever is larger.
pub const RESERVE_FRACTION: f64 = 0.10;
/// Another process holding this much dedicated VRAM is treated as an active GPU app.
pub const BUSY_DEDICATED_MB: u64 = 768;
/// Another process using this much of a 3D engine is treated as an active GPU app (games).
pub const BUSY_3D_PERCENT: f64 = 10.0;
/// System compositor processes are counted for memory but never prompt the user.
const SYSTEM_PROCESSES: &[&str] = &["dwm.exe", "csrss.exe", "system", "idle"];

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GpuProcess {
    pub pid: u32,
    pub name: String,
    pub dedicated_mb: u64,
    pub engine_3d_percent: f64,
    pub own: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct GpuSnapshot {
    pub device: Option<String>,
    pub total_mb: Option<u64>,
    pub used_mb: Option<u64>,
    pub free_mb: Option<u64>,
    /// Processes using the discrete GPU, largest memory users first.
    pub processes: Vec<GpuProcess>,
    /// False when per-process counters were unavailable (then only totals are known).
    pub per_process: bool,
}

impl GpuSnapshot {
    pub fn own_dedicated_mb(&self) -> u64 {
        self.processes
            .iter()
            .filter(|p| p.own)
            .map(|p| p.dedicated_mb)
            .sum()
    }

    /// Other applications actively using the GPU (excluding Optic Rift and the compositor).
    pub fn busy_apps(&self) -> Vec<GpuProcess> {
        self.processes
            .iter()
            .filter(|p| !p.own && !SYSTEM_PROCESSES.contains(&p.name.to_ascii_lowercase().as_str()))
            .filter(|p| {
                p.dedicated_mb >= BUSY_DEDICATED_MB || p.engine_3d_percent >= BUSY_3D_PERCENT
            })
            .cloned()
            .collect()
    }
}

pub fn reserve_mb(total_mb: u64) -> u64 {
    RESERVE_MIN_MB.max((total_mb as f64 * RESERVE_FRACTION) as u64)
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Decision {
    /// Go ahead; the worker must stay within `budget_mb` of VRAM.
    Proceed { budget_mb: u64 },
    /// Another app is actively using the GPU. Ask the user; proceed only if they acknowledge these PIDs.
    Confirm {
        budget_mb: u64,
        apps: Vec<GpuProcess>,
        message: String,
    },
    /// Not enough VRAM without squeezing other apps.
    Block { message: String },
}

/// Decide whether a job needing `need_mb` of VRAM in total (including what Optic Rift already holds)
/// may run now. `acknowledged` are PIDs the user already agreed to share the GPU with.
pub fn evaluate(s: &GpuSnapshot, need_mb: u64, acknowledged: &[u32]) -> Decision {
    let (Some(total), Some(free)) = (s.total_mb, s.free_mb) else {
        return Decision::Block { message: "No supported NVIDIA GPU was detected, so Optic Rift can't check whether there is room to run a model safely.".into() };
    };
    let reserve = reserve_mb(total);
    let own = s.own_dedicated_mb();
    // Memory Optic Rift may use in total: what it holds plus what is free beyond the reserve.
    let budget = (free + own).saturating_sub(reserve);
    if need_mb > budget {
        let others = s
            .processes
            .iter()
            .filter(|p| !p.own)
            .map(|p| p.dedicated_mb)
            .sum::<u64>();
        let who = s
            .busy_apps()
            .iter()
            .map(|p| p.name.clone())
            .collect::<Vec<_>>();
        return Decision::Block {
            message: format!(
                "Not enough free graphics memory to run this model without affecting other apps: it needs about {:.1} GB, and {:.1} GB can be used while keeping {:.1} GB free for the rest of the system{}. Close GPU-heavy apps{} and try again.",
                need_mb as f64 / 1024.0,
                budget as f64 / 1024.0,
                reserve as f64 / 1024.0,
                if s.per_process { format!(" (other apps hold {:.1} GB)", others as f64 / 1024.0) } else { String::new() },
                if who.is_empty() { String::new() } else { format!(" such as {}", who.join(", ")) },
            ),
        };
    }
    let busy = s.busy_apps();
    let unacknowledged: Vec<GpuProcess> = busy
        .iter()
        .filter(|p| !acknowledged.contains(&p.pid))
        .cloned()
        .collect();
    if !unacknowledged.is_empty() {
        let list = busy
            .iter()
            .map(|p| {
                format!(
                    "{} ({:.1} GB, {:.0}% 3D)",
                    p.name,
                    p.dedicated_mb as f64 / 1024.0,
                    p.engine_3d_percent
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        return Decision::Confirm {
            budget_mb: budget,
            apps: busy,
            message: format!("Another app is using your GPU right now: {list}. Generating at the same time can slow it down or make it unstable. Optic Rift will stay within {:.1} GB and keep {:.1} GB free.", budget as f64 / 1024.0, reserve as f64 / 1024.0),
        };
    }
    if !s.per_process && s.used_mb.unwrap_or(0) >= BUSY_DEDICATED_MB + own {
        return Decision::Confirm {
            budget_mb: budget,
            apps: Vec::new(),
            message: format!("{:.1} GB of graphics memory is in use by other software, and Optic Rift couldn't identify which apps. Continue only if nothing important is using the GPU.", s.used_mb.unwrap_or(0) as f64 / 1024.0),
        };
    }
    Decision::Proceed { budget_mb: budget }
}

/// Parse a PDH GPU instance name such as `pid_1234_luid_0x00000000_0x0000D1F2_phys_0_eng_0_engtype_3D`.
pub fn parse_instance(name: &str) -> Option<(u32, String, Option<String>)> {
    let pid = name.strip_prefix("pid_")?.split('_').next()?.parse().ok()?;
    let luid_start = name.find("luid_")?;
    let luid: String = name[luid_start..]
        .split('_')
        .take(3)
        .collect::<Vec<_>>()
        .join("_");
    let engine = name.find("engtype_").map(|i| name[i + 8..].to_string());
    Some((pid, luid, engine))
}

/// Pick the adapter LUID whose dedicated usage best matches what nvidia-smi reports in use.
pub fn match_adapter(
    adapters: &BTreeMap<String, u64>,
    nvidia_used_mb: Option<u64>,
) -> Option<String> {
    match nvidia_used_mb {
        Some(used) => adapters
            .iter()
            .min_by_key(|(_, mb)| mb.abs_diff(used))
            .map(|(l, _)| l.clone()),
        None => adapters
            .iter()
            .max_by_key(|(_, mb)| **mb)
            .map(|(l, _)| l.clone()),
    }
}

/// Combine raw counter samples into per-process figures for one adapter.
pub fn build_processes(
    memory: &[(String, f64)],
    engines: &[(String, f64)],
    adapter: &str,
    names: &HashMap<u32, String>,
    own: &HashSet<u32>,
) -> Vec<GpuProcess> {
    let mut by_pid: BTreeMap<u32, GpuProcess> = BTreeMap::new();
    let entry = |by_pid: &mut BTreeMap<u32, GpuProcess>, pid: u32| -> *mut GpuProcess {
        by_pid.entry(pid).or_insert_with(|| GpuProcess {
            pid,
            name: names
                .get(&pid)
                .cloned()
                .unwrap_or_else(|| format!("pid {pid}")),
            dedicated_mb: 0,
            engine_3d_percent: 0.0,
            own: own.contains(&pid),
        }) as *mut _
    };
    for (instance, bytes) in memory {
        if let Some((pid, luid, _)) = parse_instance(instance) {
            if luid == adapter && *bytes > 0.0 {
                // SAFETY: pointer into a map entry that is not moved before this write.
                unsafe { (*entry(&mut by_pid, pid)).dedicated_mb += (*bytes / 1_048_576.0) as u64 };
            }
        }
    }
    for (instance, percent) in engines {
        if let Some((pid, luid, Some(engine))) = parse_instance(instance) {
            if luid == adapter && engine.eq_ignore_ascii_case("3D") && *percent > 0.0 {
                // SAFETY: as above.
                unsafe {
                    let p = entry(&mut by_pid, pid);
                    (*p).engine_3d_percent = ((*p).engine_3d_percent + percent).min(100.0);
                }
            }
        }
    }
    let mut out: Vec<GpuProcess> = by_pid
        .into_values()
        .filter(|p| p.dedicated_mb > 0 || p.engine_3d_percent > 0.5)
        .collect();
    out.sort_by_key(|p| std::cmp::Reverse(p.dedicated_mb));
    out
}

#[cfg(windows)]
mod windows_counters {
    use std::collections::{HashMap, HashSet};
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::Performance::{
        PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
        PdhOpenQueryW, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_MORE_DATA,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    /// Read GPU memory, 3D engine and adapter counters, sampling engines over `interval`.
    pub fn sample(interval: Duration) -> Option<[Vec<(String, f64)>; 3]> {
        // SAFETY: standard PDH usage; handles are closed before returning and buffers are sized by PDH.
        unsafe {
            let mut query = std::mem::zeroed();
            if PdhOpenQueryW(std::ptr::null(), 0, &mut query) != 0 {
                return None;
            }
            let paths = [
                r"\GPU Process Memory(*)\Dedicated Usage",
                r"\GPU Engine(*)\Utilization Percentage",
                r"\GPU Adapter Memory(*)\Dedicated Usage",
            ];
            let mut counters = Vec::new();
            for path in paths {
                let mut c = std::mem::zeroed();
                if PdhAddEnglishCounterW(query, wide(path).as_ptr(), 0, &mut c) != 0 {
                    PdhCloseQuery(query);
                    return None;
                }
                counters.push(c);
            }
            PdhCollectQueryData(query);
            std::thread::sleep(interval);
            if PdhCollectQueryData(query) != 0 {
                PdhCloseQuery(query);
                return None;
            }
            let mut out: [Vec<(String, f64)>; 3] = Default::default();
            for (i, c) in counters.into_iter().enumerate() {
                let (mut size, mut count) = (0u32, 0u32);
                let status = PdhGetFormattedCounterArrayW(
                    c,
                    PDH_FMT_DOUBLE,
                    &mut size,
                    &mut count,
                    std::ptr::null_mut(),
                );
                if status != PDH_MORE_DATA || size == 0 {
                    continue;
                }
                let mut buf = vec![0u8; size as usize];
                let items = buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
                if PdhGetFormattedCounterArrayW(c, PDH_FMT_DOUBLE, &mut size, &mut count, items)
                    != 0
                {
                    continue;
                }
                for k in 0..count as usize {
                    let item = &*items.add(k);
                    if item.FmtValue.CStatus > 1 {
                        continue; // invalid data for this instance
                    }
                    let mut len = 0;
                    while *item.szName.add(len) != 0 {
                        len += 1;
                    }
                    let name =
                        String::from_utf16_lossy(std::slice::from_raw_parts(item.szName, len));
                    out[i].push((name, item.FmtValue.Anonymous.doubleValue));
                }
            }
            PdhCloseQuery(query);
            Some(out)
        }
    }

    /// Process names and the set of this app's own process tree.
    pub fn processes(root: u32) -> (HashMap<u32, String>, HashSet<u32>) {
        let mut names = HashMap::new();
        let mut parents = HashMap::new();
        // SAFETY: toolhelp snapshot iteration with a correctly sized entry; handle closed.
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap != INVALID_HANDLE_VALUE {
                let mut e = PROCESSENTRY32W {
                    dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                    ..Default::default()
                };
                let mut ok = Process32FirstW(snap, &mut e);
                while ok != 0 {
                    let len = e
                        .szExeFile
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(e.szExeFile.len());
                    names.insert(
                        e.th32ProcessID,
                        String::from_utf16_lossy(&e.szExeFile[..len]),
                    );
                    parents.insert(e.th32ProcessID, e.th32ParentProcessID);
                    ok = Process32NextW(snap, &mut e);
                }
                CloseHandle(snap);
            }
        }
        let mut own: HashSet<u32> = HashSet::from([root]);
        loop {
            let before = own.len();
            for (&pid, &parent) in &parents {
                if own.contains(&parent) && pid != parent {
                    own.insert(pid);
                }
            }
            if own.len() == before {
                break;
            }
        }
        (names, own)
    }
}

/// Take a full snapshot (about one second, because engine utilisation is a rate).
pub fn snapshot() -> GpuSnapshot {
    let gpu = crate::generation::gpu_info();
    let mut s = GpuSnapshot {
        device: gpu.name,
        total_mb: gpu.vram_total_mb,
        used_mb: gpu.vram_used_mb,
        free_mb: gpu.vram_free_mb,
        processes: Vec::new(),
        per_process: false,
    };
    #[cfg(windows)]
    if let Some([memory, engines, adapters_raw]) =
        windows_counters::sample(std::time::Duration::from_millis(1000))
    {
        let mut adapters = BTreeMap::new();
        for (instance, bytes) in &adapters_raw {
            if let Some(i) = instance.find("luid_") {
                let luid: String = instance[i..]
                    .split('_')
                    .take(3)
                    .collect::<Vec<_>>()
                    .join("_");
                *adapters.entry(luid).or_insert(0) += (*bytes / 1_048_576.0) as u64;
            }
        }
        if let Some(adapter) = match_adapter(&adapters, s.used_mb) {
            let (names, own) = windows_counters::processes(std::process::id());
            s.processes = build_processes(&memory, &engines, &adapter, &names, &own);
            s.per_process = true;
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(free: u64, procs: Vec<GpuProcess>) -> GpuSnapshot {
        GpuSnapshot {
            device: Some("RTX".into()),
            total_mb: Some(16303),
            used_mb: Some(16303 - free),
            free_mb: Some(free),
            processes: procs,
            per_process: true,
        }
    }
    fn p(pid: u32, name: &str, mb: u64, pct: f64, own: bool) -> GpuProcess {
        GpuProcess {
            pid,
            name: name.into(),
            dedicated_mb: mb,
            engine_3d_percent: pct,
            own,
        }
    }

    #[test]
    fn idle_gpu_proceeds_with_headroom_reserved() {
        let s = snap(
            15000,
            vec![
                p(1, "dwm.exe", 400, 3.0, false),
                p(2, "explorer.exe", 60, 0.0, false),
            ],
        );
        assert_eq!(
            evaluate(&s, 6400, &[]),
            Decision::Proceed {
                budget_mb: 15000 - reserve_mb(16303)
            }
        );
        assert_eq!(reserve_mb(16303), 1630);
    }

    #[test]
    fn a_running_game_requires_confirmation_even_when_memory_fits() {
        let wow = p(4242, "Wow.exe", 4100, 62.0, false);
        let s = snap(10000, vec![wow.clone(), p(1, "dwm.exe", 400, 20.0, false)]);
        match evaluate(&s, 6400, &[]) {
            Decision::Confirm { apps, message, .. } => {
                assert_eq!(apps, vec![wow]);
                assert!(message.contains("Wow.exe"), "{message}");
            }
            other => panic!("{other:?}"),
        }
        assert!(
            matches!(evaluate(&s, 6400, &[4242]), Decision::Proceed { .. }),
            "acknowledged PID proceeds"
        );
        assert!(
            matches!(evaluate(&s, 6400, &[999]), Decision::Confirm { .. }),
            "acknowledging another PID does not count"
        );
    }

    #[test]
    fn a_3d_busy_app_with_little_memory_still_counts() {
        let s = snap(14000, vec![p(7, "Game.exe", 300, 45.0, false)]);
        assert!(matches!(evaluate(&s, 6400, &[]), Decision::Confirm { .. }));
    }

    #[test]
    fn insufficient_memory_blocks_instead_of_squeezing_others() {
        let s = snap(6000, vec![p(4242, "Wow.exe", 9000, 70.0, false)]);
        match evaluate(&s, 6400, &[4242]) {
            Decision::Block { message } => assert!(
                message.contains("Wow.exe") && message.contains("free for the rest"),
                "{message}"
            ),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn memory_already_held_by_optic_rift_counts_toward_its_budget() {
        let s = snap(9000, vec![p(100, "python.exe", 5500, 40.0, true)]);
        assert_eq!(
            evaluate(&s, 6400, &[]),
            Decision::Proceed {
                budget_mb: 9000 + 5500 - 1630
            }
        );
    }

    #[test]
    fn without_per_process_data_unexplained_use_requires_confirmation() {
        let mut s = snap(10000, vec![]);
        s.per_process = false;
        assert!(matches!(evaluate(&s, 6400, &[]), Decision::Confirm { .. }));
        s.used_mb = Some(300);
        s.free_mb = Some(16000);
        assert!(matches!(evaluate(&s, 6400, &[]), Decision::Proceed { .. }));
    }

    #[test]
    fn no_gpu_blocks() {
        assert!(matches!(
            evaluate(&GpuSnapshot::default(), 100, &[]),
            Decision::Block { .. }
        ));
    }

    #[test]
    fn counter_instances_parse_and_combine_per_adapter() {
        assert_eq!(
            parse_instance("pid_4242_luid_0x00000000_0x0000D1F2_phys_0_eng_3_engtype_3D"),
            Some((4242, "luid_0x00000000_0x0000D1F2".into(), Some("3D".into())))
        );
        assert_eq!(
            parse_instance("pid_7_luid_0x00000000_0x0000D1F2_phys_0")
                .unwrap()
                .2,
            None
        );
        assert_eq!(parse_instance("_Total"), None);
        let nv = "luid_0x00000000_0x0000D1F2";
        let amd = "luid_0x00000000_0x0000AAAA";
        let memory = vec![
            (format!("pid_4242_{nv}_phys_0"), 4.0 * 1073741824.0),
            (format!("pid_4242_{amd}_phys_0"), 2.0 * 1073741824.0),
            (format!("pid_100_{nv}_phys_0"), 1073741824.0),
        ];
        let engines = vec![
            (format!("pid_4242_{nv}_phys_0_eng_0_engtype_3D"), 40.0),
            (format!("pid_4242_{nv}_phys_0_eng_1_engtype_3D"), 25.0),
            (format!("pid_4242_{nv}_phys_0_eng_5_engtype_Copy"), 90.0),
        ];
        let names = HashMap::from([
            (4242, "Wow.exe".to_string()),
            (100, "python.exe".to_string()),
        ]);
        let procs = build_processes(&memory, &engines, nv, &names, &HashSet::from([100]));
        assert_eq!(
            procs,
            vec![
                p(4242, "Wow.exe", 4096, 65.0, false),
                p(100, "python.exe", 1024, 0.0, true)
            ]
        );
        let adapters = BTreeMap::from([(nv.to_string(), 5200u64), (amd.to_string(), 300u64)]);
        assert_eq!(match_adapter(&adapters, Some(5000)).as_deref(), Some(nv));
    }

    /// Read-only live snapshot: queries counters and nvidia-smi, puts no load on the GPU.
    #[test]
    #[ignore]
    fn live_read_only_snapshot() {
        let s = snapshot();
        println!("{}", serde_json::to_string_pretty(&s).unwrap());
        assert!(
            s.per_process,
            "per-process GPU counters should be available on Windows 10+"
        );
    }
}
