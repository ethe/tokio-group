# Tokio-Group
A tool that helps user to create sharding tokio runtime instances, supports NUMA awareness.

## Install
Only sharding, no affinity feature.
```toml
tokio-group = { git = "https://github.com/ethe/tokio-group.git", branch = "main" }
```

### Core Affinity Mode
Just bind each runtime to cores without NUMA info.
```toml
tokio-group = { git = "https://github.com/ethe/tokio-group.git", branch = "main", features = ["affinity"] }
```

### Numa Awareness Mode
Bind each runtime to NUMA nodes.
```toml
tokio-group = { git = "https://github.com/ethe/tokio-group.git", branch = "main", features = ["numa-awareness"] }
```

## Usage
```rust
fn main() {
    let results: std::io::Result<Vec<_>> = tokio_group::new()
      // switch on NUMA mode, relies on numa-awareness feature.
      .numa(true)
      // tokio-group supports two-level affinity strategies, several tokio runtimes could share one NUMA node.
      .workers_per_numa(1)
      .init(async {
          // some initializations before forking tokio runtimes.
      })
      .entry(async move {
          // server entry here.
      })
      .run();
}
```
