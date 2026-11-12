//! The warning rules over successive samples. Pure: the caller passes each sample and the time, so
//! every rule is tested without waiting.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use gitprompt_store::{SLOW_HOLD, SLOW_WAIT, WriterStats};

use super::SLOW_REQUEST;
use super::process::ProcessSample;

/// When a sample is worth a warning.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Limits {
    /// Resident memory above this is too much for a local daemon.
    pub max_rss_bytes: u64,
    /// Resident memory above this multiple of the settled baseline (the first sample after
    /// `settle`) is a leak or an unbounded cache...
    pub growth_factor: f64,
    /// ...when it is also at least this much above the baseline (small heaps double easily).
    pub growth_min_bytes: u64,
    /// CPU (percent of one core) above this...
    pub max_cpu_percent: f32,
    /// ...for this many samples in a row.
    pub cpu_samples: u32,
    /// Start-up (index rebuild, spool drain) is busy by nature: no baseline or CPU rule before.
    #[serde(with = "seconds")]
    pub settle: Duration,
    /// The same kind of warning is not repeated sooner.
    #[serde(with = "seconds")]
    pub cooldown: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_rss_bytes: 1 << 30,
            growth_factor: 3.0,
            growth_min_bytes: 256 << 20,
            max_cpu_percent: 80.0,
            cpu_samples: 3,
            settle: Duration::from_secs(120),
            cooldown: Duration::from_secs(15 * 60),
        }
    }
}

/// Durations as whole seconds on the wire.
mod seconds {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(value: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(value.as_secs())
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
        u64::deserialize(deserializer).map(Duration::from_secs)
    }
}

/// What a warning is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WarningKind {
    MemoryHigh,
    MemoryGrowth,
    CpuHigh,
    /// Writes waited for the database writer (something kept it too long).
    WriterContention,
    /// Requests answered 5xx.
    Errors,
    /// Requests slower than the access log's limit.
    SlowRequests,
}

/// A warning a rule raised: its kind and its message.
pub type Raised = (WarningKind, String);

/// One sample of the process, and the counters since start.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sample {
    pub process: ProcessSample,
    pub writer: WriterStats,
    /// Requests answered 5xx so far.
    pub errors: u64,
    /// Requests slower than [`SLOW_REQUEST`] so far.
    pub slow: u64,
}

#[derive(Debug)]
pub struct Monitor {
    limits: Limits,
    started: Instant,
    baseline_rss: Option<u64>,
    hot_samples: u32,
    previous: Sample,
    last_warned: HashMap<WarningKind, Instant>,
}

impl Monitor {
    pub fn new(limits: Limits, started: Instant) -> Self {
        Monitor { limits, started, baseline_rss: None, hot_samples: 0, previous: Sample::default(), last_warned: HashMap::new() }
    }

    /// The warnings `sample` raises at `now`, each kind at most once per cooldown.
    pub fn observe(&mut self, sample: Sample, now: Instant) -> Vec<Raised> {
        let settled = now.saturating_duration_since(self.started) >= self.limits.settle;
        let raised = [
            self.memory_high(&sample),
            self.memory_growth(&sample, settled),
            self.cpu_high(&sample, settled),
            self.writer_contention(&sample),
            self.errors(&sample),
            self.slow_requests(&sample),
        ];
        self.previous = sample;
        raised.into_iter().flatten().filter(|(kind, _)| self.quiet(*kind, now)).collect()
    }

    fn memory_high(&self, sample: &Sample) -> Option<Raised> {
        let rss = sample.process.rss_bytes.filter(|rss| *rss > self.limits.max_rss_bytes)?;
        Some((WarningKind::MemoryHigh, format!("resident memory is {} (limit {})", mib(rss), mib(self.limits.max_rss_bytes))))
    }

    /// Measured from the first sample once start-up settled.
    fn memory_growth(&mut self, sample: &Sample, settled: bool) -> Option<Raised> {
        let rss = sample.process.rss_bytes.filter(|_| settled)?;
        let baseline = *self.baseline_rss.get_or_insert(rss);
        #[expect(clippy::cast_precision_loss, clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "a byte threshold")]
        let grown = (baseline as f64 * self.limits.growth_factor) as u64;
        let leaking = rss > grown && rss.saturating_sub(baseline) >= self.limits.growth_min_bytes;
        leaking.then(|| {
            (WarningKind::MemoryGrowth, format!("resident memory grew from {} to {} since start-up settled", mib(baseline), mib(rss)))
        })
    }

    /// Only after `cpu_samples` hot samples in a row: a short burst is normal.
    fn cpu_high(&mut self, sample: &Sample, settled: bool) -> Option<Raised> {
        let cpu = sample.process.cpu_percent.filter(|cpu| settled && *cpu > self.limits.max_cpu_percent);
        self.hot_samples = if cpu.is_some() { self.hot_samples + 1 } else { 0 };
        let cpu = cpu.filter(|_| self.hot_samples >= self.limits.cpu_samples)?;
        Some((WarningKind::CpuHigh, format!("CPU at {cpu:.0}% of a core for {} samples in a row", self.hot_samples)))
    }

    fn writer_contention(&self, sample: &Sample) -> Option<Raised> {
        let changed = sample.writer.since(self.previous.writer);
        (changed.slow_waits > 0).then(|| {
            let message = format!(
                "{} database writes waited over {} ms for the writer, {} kept it over {} ms (longest stay since start {} ms; the `gitprompt_store::writer` warnings name where)",
                changed.slow_waits,
                SLOW_WAIT.as_millis(),
                changed.slow_holds,
                SLOW_HOLD.as_millis(),
                sample.writer.max_hold_ms
            );
            (WarningKind::WriterContention, message)
        })
    }

    fn errors(&self, sample: &Sample) -> Option<Raised> {
        let new = sample.errors.saturating_sub(self.previous.errors);
        (new > 0).then(|| (WarningKind::Errors, format!("{new} requests failed with a server error")))
    }

    fn slow_requests(&self, sample: &Sample) -> Option<Raised> {
        let new = sample.slow.saturating_sub(self.previous.slow);
        (new > 0).then(|| (WarningKind::SlowRequests, format!("{new} requests took over {} ms", SLOW_REQUEST.as_millis())))
    }

    /// Whether `kind` may warn at `now` (and if so, starts its cooldown).
    fn quiet(&mut self, kind: WarningKind, now: Instant) -> bool {
        let quiet = self.last_warned.get(&kind).is_none_or(|at| now.saturating_duration_since(*at) >= self.limits.cooldown);
        if quiet {
            self.last_warned.insert(kind, now);
        }
        quiet
    }
}

fn mib(bytes: u64) -> String {
    format!("{} MiB", bytes >> 20)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIB: u64 = 1 << 20;

    fn limits() -> Limits {
        Limits { settle: Duration::from_secs(60), cooldown: Duration::from_secs(600), ..Limits::default() }
    }

    fn rss(mib: u64) -> Sample {
        Sample { process: ProcessSample { rss_bytes: Some(mib * MIB), cpu_percent: None }, ..Sample::default() }
    }

    fn cpu(percent: f32) -> Sample {
        Sample { process: ProcessSample { rss_bytes: None, cpu_percent: Some(percent) }, ..Sample::default() }
    }

    fn kinds(found: &[Raised]) -> Vec<WarningKind> {
        found.iter().map(|(kind, _)| *kind).collect()
    }

    #[test]
    fn memory_above_the_limit_warns_once_per_cooldown() {
        let start = Instant::now();
        let mut monitor = Monitor::new(limits(), start);
        assert_eq!(kinds(&monitor.observe(rss(2048), start)), [WarningKind::MemoryHigh]);
        assert_eq!(kinds(&monitor.observe(rss(2048), start + Duration::from_secs(30))), [], "cooling down");
        assert_eq!(kinds(&monitor.observe(rss(2048), start + Duration::from_secs(601))), [WarningKind::MemoryHigh]);
    }

    #[test]
    fn growth_is_measured_from_the_first_sample_after_start_up_settles() {
        let start = Instant::now();
        let mut monitor = Monitor::new(limits(), start);
        let at = |s: u64| start + Duration::from_secs(s);
        assert_eq!(kinds(&monitor.observe(rss(500), at(0))), [], "start-up peak: no baseline yet");
        assert_eq!(kinds(&monitor.observe(rss(100), at(60))), [], "baseline 100 MiB");
        assert_eq!(kinds(&monitor.observe(rss(290), at(90))), [], "under 3x");
        assert_eq!(kinds(&monitor.observe(rss(320), at(120))), [], "3x but less than 256 MiB more");
        assert_eq!(kinds(&monitor.observe(rss(400), at(150))), [WarningKind::MemoryGrowth]);
    }

    #[test]
    fn cpu_warns_only_when_high_for_several_samples_in_a_row() {
        let start = Instant::now();
        let mut monitor = Monitor::new(limits(), start);
        let at = |s: u64| start + Duration::from_secs(60 + s);
        for (s, percent) in [(0, 95.0), (30, 95.0), (60, 10.0), (90, 95.0), (120, 95.0)] {
            assert_eq!(kinds(&monitor.observe(cpu(percent), at(s))), [], "at {s} s");
        }
        assert_eq!(kinds(&monitor.observe(cpu(95.0), at(150))), [WarningKind::CpuHigh]);
    }

    #[test]
    fn counters_warn_on_what_happened_since_the_previous_sample() {
        let start = Instant::now();
        let mut monitor = Monitor::new(Limits { cooldown: Duration::ZERO, ..limits() }, start);
        let mut sample =
            Sample { writer: WriterStats { slow_waits: 2, ..WriterStats::default() }, errors: 1, slow: 3, ..Sample::default() };
        let all = [WarningKind::WriterContention, WarningKind::Errors, WarningKind::SlowRequests];
        assert_eq!(kinds(&monitor.observe(sample, start)), all);
        assert_eq!(kinds(&monitor.observe(sample, start + Duration::from_secs(30))), [], "nothing new");
        sample.errors = 2;
        assert_eq!(kinds(&monitor.observe(sample, start + Duration::from_secs(60))), [WarningKind::Errors]);
    }
}
