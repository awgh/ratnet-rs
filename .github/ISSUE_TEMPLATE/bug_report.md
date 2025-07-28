---
name: Bug report
about: Create a report to help us improve RatNet-rs
title: '[BUG] '
labels: ['bug']
assignees: ''

---

**Describe the bug**
A clear and concise description of what the bug is.

**To Reproduce**
Steps to reproduce the behavior:
1. Run command '...'
2. See error '...'

**Expected behavior**
A clear and concise description of what you expected to happen.

**Environment (please complete the following information):**
 - OS: [e.g. Ubuntu 22.04, macOS 14.0, Windows 11]
 - Rust version: [e.g. 1.75.0]
 - RatNet-rs version: [e.g. 0.1.0]
 - Architecture: [e.g. x86_64, aarch64]

**Additional context**
Add any other context about the problem here, such as:
- Logs or error messages
- Network configuration
- Related issues
- Screenshots if applicable

**Reproducible example**
If possible, provide a minimal example that reproduces the issue:

```rust
use ratnet::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Your minimal example here
    Ok(())
}
``` 