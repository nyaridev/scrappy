//! Jobs: long work that runs in the background with an id, a cancel flag and progress reports, optionally waiting
//! in a queue that limits how many of its kind run at once (ADR-0010).

// ── Imports ───────────────────────────────
use std::collections::HashMap;
use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::Serialize;
use tokio::sync::Semaphore;
use ts_rs::TS;

// ── Constants ─────────────────────────────
/// Progress reports closer together than this are dropped, so a fast download doesn't flood the window.
const REPORT_EVERY: Duration = Duration::from_millis(100);
/// How often a queued job checks whether it was cancelled while it waits.
const QUEUE_CHECK: Duration = Duration::from_millis(200);

// ── Types ─────────────────────────────────
pub type JobId = u32;

/// Where reports go: the shell turns them into `job-progress` and `job-finished` events.
pub type Reporter = Arc<dyn Fn(JobEvent) + Send + Sync>;

/// What a job is doing right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum JobPhase {
    /// Waiting for a free slot in its queue.
    Queued,
    Download,
    Unpack,
    /// Re-encoding with FFmpeg; `done` and `total` are microseconds of the media.
    Encode,
    /// Saving a collection (playlist, profile, post); `done` and `total` are files, `message` the current one.
    Items,
}

/// How far a job has got. Sizes are bytes (microseconds while encoding), speed is per second, eta is seconds.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct JobProgress {
    pub id: JobId,
    pub phase: JobPhase,
    #[ts(type = "number")]
    pub done: u64,
    #[ts(type = "number | null")]
    pub total: Option<u64>,
    #[ts(type = "number | null")]
    pub speed: Option<u64>,
    #[ts(type = "number | null")]
    pub eta: Option<u64>,
    pub message: Option<String>,
}

/// How a job ended.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum JobResult {
    Done,
    Failed { message: String },
    Cancelled,
}

/// A job's end, sent once.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct JobFinished {
    pub id: JobId,
    pub result: JobResult,
}

/// A report from a job.
#[derive(Debug, Clone, PartialEq)]
pub enum JobEvent {
    Progress(JobProgress),
    Finished(JobFinished),
}

/// The running jobs and where their reports go.
pub struct Jobs {
    next: AtomicU32,
    running: Mutex<HashMap<JobId, Arc<AtomicBool>>>,
    report: Reporter,
}

/// A limit on how many jobs of one kind run at once; the rest wait in order.
#[derive(Clone)]
pub struct Queue(Arc<Semaphore>);

/// A running job's handle: its id, whether it was cancelled, and a way to report progress.
#[derive(Clone)]
pub struct Job {
    id: JobId,
    cancelled: Arc<AtomicBool>,
    report: Reporter,
    started: Instant,
    last_report: Arc<Mutex<Option<Instant>>>,
}

// ── Public API ────────────────────────────
impl Jobs {
    /// No jobs yet; reports go to `report`.
    pub fn new(report: Reporter) -> Arc<Self> {
        Arc::new(Self { next: AtomicU32::new(1), running: Mutex::new(HashMap::new()), report })
    }

    /// Starts `work` in the background and returns its id at once. Must be called inside a Tokio runtime (an async
    /// command). A cancelled job reports `Cancelled` whatever `work` returns.
    pub fn start<F, Fut>(self: &Arc<Self>, work: F) -> JobId
    where
        F: FnOnce(Job) -> Fut,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let cancelled = Arc::new(AtomicBool::new(false));
        self.running.lock().unwrap_or_else(PoisonError::into_inner).insert(id, cancelled.clone());
        let job = Job {
            id,
            cancelled: cancelled.clone(),
            report: self.report.clone(),
            started: Instant::now(),
            last_report: Arc::new(Mutex::new(None)),
        };
        let future = work(job);
        let jobs = self.clone();
        tokio::spawn(async move {
            let outcome = future.await;
            jobs.running.lock().unwrap_or_else(PoisonError::into_inner).remove(&id);
            let result = match outcome {
                _ if cancelled.load(Ordering::Relaxed) => JobResult::Cancelled,
                Ok(()) => JobResult::Done,
                Err(message) => JobResult::Failed { message },
            };
            (jobs.report)(JobEvent::Finished(JobFinished { id, result }));
        });
        id
    }

    /// Like `start`, but `work` waits in `queue` until a slot is free. A job cancelled while it waits never runs.
    pub fn start_in<F, Fut>(self: &Arc<Self>, queue: &Queue, work: F) -> JobId
    where
        F: FnOnce(Job) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        let slots = queue.0.clone();
        self.start(move |job| async move {
            job.progress(JobPhase::Queued, 0, None);
            let _slot = loop {
                tokio::select! {
                    slot = slots.clone().acquire_owned() => break slot.map_err(|_| "The queue was closed.")?,
                    () = tokio::time::sleep(QUEUE_CHECK) => if job.cancelled() { return Err("Cancelled.".into()) },
                }
            };
            work(job).await
        })
    }

    /// Asks a running job to stop. False when no job has that id.
    pub fn cancel(&self, id: JobId) -> bool {
        let running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        running.get(&id).inspect(|flag| flag.store(true, Ordering::Relaxed)).is_some()
    }
}

impl Queue {
    /// A queue running at most `limit` jobs at once.
    pub fn new(limit: usize) -> Self {
        Self(Arc::new(Semaphore::new(limit)))
    }
}

impl Job {
    /// This job's id.
    pub fn id(&self) -> JobId {
        self.id
    }

    /// Whether the job was asked to stop; long work checks this between steps.
    pub fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// Reports `done` of `total` bytes in `phase`, with speed and time left from the job's start. Reports come at
    /// most every 100 ms, except the last one of a phase.
    pub fn progress(&self, phase: JobPhase, done: u64, total: Option<u64>) {
        self.report(phase, done, total, None);
    }

    /// Reports `done` of `total` files of a collection, with the name of the one being saved.
    pub fn items(&self, done: u64, total: Option<u64>, current: Option<String>) {
        self.report(JobPhase::Items, done, total, current);
    }

    fn report(&self, phase: JobPhase, done: u64, total: Option<u64>, message: Option<String>) {
        let now = Instant::now();
        let finished = total == Some(done);
        {
            let mut last = self.last_report.lock().unwrap_or_else(PoisonError::into_inner);
            if !finished && last.is_some_and(|at| now.duration_since(at) < REPORT_EVERY) {
                return;
            }
            *last = Some(now);
        }
        let seconds = now.duration_since(self.started).as_secs_f64();
        let speed = (seconds > 0.0).then(|| (done as f64 / seconds) as u64).filter(|&speed| speed > 0);
        let eta = total.zip(speed).map(|(total, speed)| total.saturating_sub(done) / speed);
        (self.report)(JobEvent::Progress(JobProgress { id: self.id, phase, done, total, speed, eta, message }));
    }
}

#[cfg(test)]
mod tests {
    use super::{JobEvent, JobPhase, JobResult, Jobs};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    fn recorder() -> (Arc<Mutex<Vec<JobEvent>>>, super::Reporter) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let sink = events.clone();
        (events, Arc::new(move |event| sink.lock().unwrap().push(event)))
    }

    async fn finished(events: &Mutex<Vec<JobEvent>>) -> JobResult {
        for _ in 0..100 {
            if let Some(JobEvent::Finished(end)) = events.lock().unwrap().last().cloned() {
                return end.result;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("the job never finished");
    }

    #[tokio::test]
    async fn reports_progress_and_the_end() {
        let (events, report) = recorder();
        let jobs = Jobs::new(report);
        jobs.start(|job| async move {
            job.progress(JobPhase::Download, 5, Some(10));
            job.progress(JobPhase::Download, 10, Some(10));
            Ok(())
        });
        assert_eq!(finished(&events).await, JobResult::Done);
        let progress = events.lock().unwrap().iter().filter(|e| matches!(e, JobEvent::Progress(_))).count();
        assert_eq!(progress, 2);
    }

    #[tokio::test]
    async fn a_queue_runs_one_at_a_time() {
        let (events, report) = recorder();
        let jobs = Jobs::new(report);
        let queue = super::Queue::new(1);
        let running = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let most = Arc::new(std::sync::atomic::AtomicU32::new(0));
        for _ in 0..3 {
            let (running, most) = (running.clone(), most.clone());
            jobs.start_in(&queue, move |_| async move {
                let now = running.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                most.fetch_max(now, std::sync::atomic::Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(20)).await;
                running.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            });
        }
        for _ in 0..200 {
            let ended = events.lock().unwrap().iter().filter(|e| matches!(e, JobEvent::Finished(_))).count();
            if ended == 3 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(most.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn reports_failures_and_cancels() {
        let (events, report) = recorder();
        let jobs = Jobs::new(report);
        jobs.start(|_| async { Err("No network.".to_string()) });
        assert_eq!(finished(&events).await, JobResult::Failed { message: "No network.".into() });

        let (events, report) = recorder();
        let jobs = Jobs::new(report);
        let id = jobs.start(|job| async move {
            while !job.cancelled() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            Err("stopped".to_string())
        });
        assert!(jobs.cancel(id));
        assert_eq!(finished(&events).await, JobResult::Cancelled);
        assert!(!jobs.cancel(id));
    }
}
