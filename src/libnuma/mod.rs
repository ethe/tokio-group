use std::io;

#[cfg(target_os = "linux")]
mod sys {
    use std::{fs, io};

    pub(crate) unsafe fn numa_available() -> bool {
        libc::syscall(libc::SYS_get_mempolicy, &mut (), &mut (), 0, 0, 0) < 0
    }

    pub(crate) fn nume_nodes() -> io::Result<Vec<usize>> {
        Ok(fs::read_dir("/sys/devices/system/node")?
            .filter_map(Result::ok)
            .map(|d| d.file_name().to_string_lossy())
            .filter(|d| d.starts_with("node"))
            .map(|d| d[4..].parse::<usize>())
            .count())
    }

    pub(crate) fn numa_node_to_cpus(node: usize) -> io::Result<Vec<usize>> {
        Ok(
            fs::read_dir(format!("/sys/devices/system/node/node{node}"))?
                .filter_map(Result::ok)
                .filter_map(|d| {
                    let name = d.file_name();
                    if name.to_string_lossy().starts_with("cpu") {
                        Some(name.to_string_lossy()[3..].to_string())
                    } else {
                        None
                    }
                })
                .filter_map(|id| id.parse().ok())
                .collect(),
        )
    }
}

#[cfg(not(target_os = "linux"))]
mod sys {
    use std::{error::Error, fmt::Display, io};

    #[derive(Debug)]
    pub struct NotSupport;

    impl Display for NotSupport {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("numa is not supported in target os")
        }
    }

    impl Error for NotSupport {}

    pub(crate) unsafe fn numa_available() -> bool {
        false
    }

    pub(crate) fn numa_max_node() -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::Other, NotSupport))
    }

    pub(crate) fn numa_node_to_cpus(node: usize) -> io::Result<Vec<usize>> {
        let _ = node;
        Err(io::Error::new(io::ErrorKind::Other, NotSupport))
    }
}

pub fn numa_available() -> bool {
    unsafe { sys::numa_available() }
}

pub fn numa_max_node() -> io::Result<usize> {
    sys::numa_max_node()
}

pub fn numa_node_to_cpus(node: usize) -> io::Result<Vec<usize>> {
    sys::numa_node_to_cpus(node)
}

pub fn set_thread_affinity(cpus: &[usize]) {
    #[cfg(all(target_os = "linux", feature = "affinity"))]
    affinity::set_thread_affinity(cpus);
    #[cfg(not(target_os = "linux"))]
    let _ = cpus;
}
