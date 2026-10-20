//! 进度/日志上报抽象：GUI 用事件实现，CLI 用控制台实现。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub trait Reporter: Send + Sync + 'static {
    fn progress(&self, task: &str, phase: &str, done: u64, total: u64, message: &str);
    fn log(&self, task: &str, line: &str);
}

pub struct NullReporter;

impl Reporter for NullReporter {
    fn progress(&self, _: &str, _: &str, _: u64, _: u64, _: &str) {}
    fn log(&self, _: &str, _: &str) {}
}

struct Inner {
    task: String,
    phase: String,
    done: AtomicU64,
    total: AtomicU64,
    reporter: Arc<dyn Reporter>,
    last: Mutex<Instant>,
}

/// 一次安装/下载任务的进度计数器（Arc 共享，可克隆进并发任务）。
#[derive(Clone)]
pub struct Progress {
    inner: Arc<Inner>,
}

impl Progress {
    pub fn new(
        reporter: Arc<dyn Reporter>,
        task: impl Into<String>,
        phase: impl Into<String>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                task: task.into(),
                phase: phase.into(),
                done: AtomicU64::new(0),
                total: AtomicU64::new(0),
                reporter,
                last: Mutex::new(Instant::now() - Duration::from_secs(1)),
            }),
        }
    }

    pub fn set_total(&self, total: u64) {
        self.inner.total.store(total, Ordering::Relaxed);
        self.force("");
    }

    pub fn add_total(&self, n: u64) {
        self.inner.total.fetch_add(n, Ordering::Relaxed);
    }

    pub fn inc(&self, n: u64) {
        let done = self.inner.done.fetch_add(n, Ordering::Relaxed) + n;
        let mut last = self.inner.last.lock().unwrap_or_else(|e| e.into_inner());
        if last.elapsed() >= Duration::from_millis(120) {
            *last = Instant::now();
            self.inner.reporter.progress(
                &self.inner.task,
                &self.inner.phase,
                done,
                self.inner.total.load(Ordering::Relaxed),
                "",
            );
        }
    }

    pub fn set_done(&self, n: u64) {
        self.inner.done.store(n, Ordering::Relaxed);
    }

    pub fn done(&self) -> u64 {
        self.inner.done.load(Ordering::Relaxed)
    }

    pub fn total(&self) -> u64 {
        self.inner.total.load(Ordering::Relaxed)
    }

    pub fn force(&self, message: &str) {
        let mut last = self.inner.last.lock().unwrap_or_else(|e| e.into_inner());
        *last = Instant::now();
        self.inner.reporter.progress(
            &self.inner.task,
            &self.inner.phase,
            self.inner.done.load(Ordering::Relaxed),
            self.inner.total.load(Ordering::Relaxed),
            message,
        );
    }

    pub fn log(&self, line: impl AsRef<str>) {
        self.inner.reporter.log(&self.inner.task, line.as_ref());
    }

    pub fn reporter(&self) -> Arc<dyn Reporter> {
        self.inner.reporter.clone()
    }

    pub fn task(&self) -> &str {
        &self.inner.task
    }
}
