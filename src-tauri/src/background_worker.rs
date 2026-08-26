use std::{
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub(crate) const WORKER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);
const WORKER_JOIN_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// 在有限时间内等待后台线程退出；外部系统调用卡住时放弃等待，避免应用无法关闭。
pub(crate) fn join_with_timeout(
    worker: JoinHandle<()>,
    worker_name: &str,
    timeout: Duration,
) -> bool {
    let deadline = Instant::now() + timeout;
    while !worker.is_finished() && Instant::now() < deadline {
        thread::sleep(WORKER_JOIN_POLL_INTERVAL);
    }

    if !worker.is_finished() {
        log::warn!(
            "后台线程 {worker_name} 未在 {} ms 内退出，将由进程结束回收",
            timeout.as_millis()
        );
        return false;
    }
    if worker.join().is_err() {
        log::warn!("后台线程 {worker_name} 退出时发生 panic");
    }
    true
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use super::join_with_timeout;

    #[test]
    fn join_with_timeout_should_join_completed_worker() {
        let worker = thread::spawn(|| {});

        assert!(join_with_timeout(
            worker,
            "completed-test-worker",
            Duration::from_millis(50)
        ));
    }

    #[test]
    fn join_with_timeout_should_stop_waiting_for_blocked_worker() {
        let worker = thread::spawn(|| thread::sleep(Duration::from_millis(30)));

        assert!(!join_with_timeout(
            worker,
            "blocked-test-worker",
            Duration::from_millis(1)
        ));
    }
}
