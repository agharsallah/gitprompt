//! Resident memory and CPU of this process, read with `sysinfo` (safe, cross-platform).

use serde::{Deserialize, Serialize};

use super::stats::max_present;

/// One reading; `None` where the platform can't tell.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProcessSample {
    pub rss_bytes: Option<u64>,
    /// Percent of one core (above 100 when several cores are busy).
    pub cpu_percent: Option<f32>,
}

/// The highest readings seen so far.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Peaks {
    pub rss_bytes: Option<u64>,
    pub cpu_percent: Option<f32>,
}

impl Peaks {
    pub fn add(&mut self, sample: ProcessSample) {
        self.rss_bytes = max_present(self.rss_bytes, sample.rss_bytes);
        self.cpu_percent = max_present(self.cpu_percent, sample.cpu_percent);
    }
}

/// Reads this process. CPU is measured between two calls, so the first reading is 0.
pub struct ProcessSampler {
    system: sysinfo::System,
    pid: Option<sysinfo::Pid>,
}

impl std::fmt::Debug for ProcessSampler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProcessSampler").field("pid", &self.pid).finish_non_exhaustive()
    }
}

impl Default for ProcessSampler {
    fn default() -> Self {
        ProcessSampler { system: sysinfo::System::new(), pid: sysinfo::get_current_pid().ok() }
    }
}

impl ProcessSampler {
    pub fn sample(&mut self) -> ProcessSample {
        let Some(pid) = self.pid else { return ProcessSample::default() };
        let kind = sysinfo::ProcessRefreshKind::nothing().with_memory().with_cpu();
        self.system.refresh_processes_specifics(sysinfo::ProcessesToUpdate::Some(&[pid]), true, kind);
        self.system
            .process(pid)
            .map_or_else(ProcessSample::default, |p| ProcessSample { rss_bytes: Some(p.memory()), cpu_percent: Some(p.cpu_usage()) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_process_can_be_sampled() {
        let sample = ProcessSampler::default().sample();
        assert!(sample.rss_bytes.is_some_and(|bytes| bytes > 0));
        assert!(sample.cpu_percent.is_some());
    }

    #[test]
    fn peaks_keep_the_highest_reading_of_each() {
        let mut peaks = Peaks::default();
        peaks.add(ProcessSample { rss_bytes: Some(10), cpu_percent: Some(90.0) });
        peaks.add(ProcessSample { rss_bytes: Some(30), cpu_percent: None });
        assert_eq!(peaks, Peaks { rss_bytes: Some(30), cpu_percent: Some(90.0) });
    }
}
