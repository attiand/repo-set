use crossbeam_channel::unbounded;
use std::thread;

/// Run `task` over every item using a pool of worker threads tuned for blocking
/// I/O. Work is pulled from a shared queue so slow clones don't stall the others.
pub fn for_each_io<T, F>(items: &[T], threads: usize, task: F)
where
    T: Sync,
    F: Fn(&T) + Sync,
{
    if items.is_empty() {
        return;
    }

    let workers = worker_count(threads, items.len());

    let (tx, rx) = unbounded::<&T>();
    for item in items {
        tx.send(item).expect("work queue send failed");
    }
    drop(tx); // let workers exit once the queue is drained

    thread::scope(|scope| {
        for _ in 0..workers {
            let rx = rx.clone();
            let task = &task;
            scope.spawn(move || {
                while let Ok(item) = rx.recv() {
                    task(item);
                }
            });
        }
    });
}

/// Pick a worker count tuned for blocking I/O. With `requested == 0` we use
/// several times the core count, since clone workers spend most of their time
/// waiting on the network and disk rather than the CPU.
pub fn worker_count(requested: usize, items: usize) -> usize {
    let n = if requested == 0 {
        thread::available_parallelism()
            .map(|p| p.get() * 4)
            .unwrap_or(8)
    } else {
        requested
    };
    n.clamp(1, items.max(1))
}
