pub mod libnuma;

use std::{cmp::max, io, mem::MaybeUninit};

use std::future::Future;

pub struct WorkerGroup<Init, Entry> {
    #[cfg(feature = "numa-awareness")]
    numa: bool,
    #[cfg(feature = "numa-awareness")]
    workers_per_numa: usize,
    init: MaybeUninit<Init>,
    entry: Entry,
    #[cfg(not(feature = "numa-awareness"))]
    workers: usize,
}

impl<Init, Entry> WorkerGroup<Init, Entry> {
    pub fn new() -> WorkerGroup<fn(), fn()> {
        WorkerGroup {
            #[cfg(feature = "numa-awareness")]
            numa: false,
            #[cfg(feature = "numa-awareness")]
            workers_per_numa: 1,
            init: MaybeUninit::new(|| {}),
            entry: || {},
            #[cfg(not(feature = "numa-awareness"))]
            workers: 1,
        }
    }

    #[cfg(feature = "numa-awareness")]
    pub fn numa(self, enable: bool) -> Self {
        Self {
            numa: enable,
            workers_per_numa: self.workers_per_numa,
            init: self.init,
            entry: self.entry,
        }
    }

    #[cfg(feature = "numa-awareness")]
    pub fn workers_per_numa(self, n: bool) -> Self {
        Self {
            numa: self.numa,
            workers_per_numa: n,
            init: self.init,
            entry: self.entry,
        }
    }

    pub fn entry<G, E>(self, entry: E) -> WorkerGroup<Init, E>
    where
        E: Fn() -> G,
        G: 'static + Future + Send,
        G::Output: 'static + Send,
    {
        WorkerGroup {
            #[cfg(feature = "numa-awareness")]
            numa: self.numa,
            #[cfg(feature = "numa-awareness")]
            workers_per_numa: self.workers_per_numa,
            init: self.init,
            entry,
            #[cfg(not(feature = "numa-awareness"))]
            workers: self.workers,
        }
    }

    pub fn init<I: Future>(self, init: I) -> WorkerGroup<I, Entry> {
        WorkerGroup {
            #[cfg(feature = "numa-awareness")]
            numa: self.numa,
            #[cfg(feature = "numa-awareness")]
            workers_per_numa: self.workers_per_numa,
            init: MaybeUninit::new(init),
            entry: self.entry,
            #[cfg(not(feature = "numa-awareness"))]
            workers: self.workers,
        }
    }

    #[cfg(not(feature = "numa-awareness"))]
    pub fn worker_num(self, num: usize) -> Self {
        Self {
            init: self.init,
            entry: self.entry,
            workers: num,
        }
    }
}

impl<G, Init, Entry> WorkerGroup<Init, Entry>
where
    Init: Future,
    Entry: Fn() -> G,
    G: 'static + Future + Send,
    G::Output: 'static + Send,
{
    pub fn run(self) -> io::Result<Vec<G::Output>> {
        let init_runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()?;

        let _guard = init_runtime.block_on(unsafe { self.init.assume_init_read() });
        self.run_workers()
    }

    #[cfg(feature = "numa-awareness")]
    fn run_workers(self) -> io::Result<Vec<G::Output>> {
        let nodes = libnuma::numa_max_node()?;

        let mut handles = Vec::new();
        for node in 0..nodes {
            for _ in self.workers_per_numa {
                let mut builder = tokio::runtime::Builder::new_multi_thread();
                builder.enable_all().on_thread_start(move || {
                    if self.numa && libnuma::numa_available() {
                        let cpus = libnuma::numa_node_to_cpus(node).unwrap();
                        libnuma::set_thread_affinity(&cpus);
                    }
                });

                if self.numa && libnuma::numa_available() {
                    builder.worker_threads(max(
                        libnuma::numa_node_to_cpus(node).unwrap().len()
                            / (nodes * self.workers_per_numa),
                        1,
                    ));
                } else {
                    builder.worker_threads(max(num_cpus() / worker_num, 1));
                }

                let runtime = builder.build()?;

                let _rt = runtime.enter();
                let handle = runtime.spawn((self.entry)());
                handles.push(async move {
                    let _runtime = runtime;
                    handle.await
                });
            }
        }

        let mut results = Vec::new();
        for result in futures::executor::block_on(futures::future::join_all(handles)) {
            results.push(result?);
        }

        Ok(results)
    }

    #[cfg(not(feature = "numa-awareness"))]
    fn run_workers(self) -> io::Result<Vec<G::Output>> {
        use futures_util::future;

        let mut handles = Vec::new();
        for _ in 0..self.workers {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .worker_threads(max(num_cpus() / self.workers, 1))
                .build()?;

            let _rt = runtime.enter();
            let handle = runtime.spawn((self.entry)());
            handles.push(async move {
                let _runtime = runtime;
                handle.await
            });
        }

        let mut results = Vec::new();
        for result in spin_on::spin_on(future::join_all(handles)) {
            results.push(result?);
        }

        Ok(results)
    }
}

pub(crate) fn num_cpus() -> usize {
    const ENV_WORKER_THREADS: &str = "TOKIO_WORKER_THREADS";

    match std::env::var(ENV_WORKER_THREADS) {
        Ok(s) => {
            let n = s.parse().unwrap_or_else(|e| {
                panic!(
                    "\"{}\" must be usize, error: {}, value: {}",
                    ENV_WORKER_THREADS, e, s
                )
            });
            assert!(n > 0, "\"{}\" cannot be set to 0", ENV_WORKER_THREADS);
            n
        }
        Err(std::env::VarError::NotPresent) => usize::max(1, num_cpus::get()),
        Err(std::env::VarError::NotUnicode(e)) => {
            panic!(
                "\"{}\" must be valid unicode, error: {:?}",
                ENV_WORKER_THREADS, e
            )
        }
    }
}
