---
applyTo: "crates/**/*.rs"
description: "Use when editing Rust files in crates/: prevent spaghetti code, enforce module boundaries, and split oversized multi-responsibility files."
---

# Rust Modularity Guardrails

- Keep one primary responsibility per Rust file.
- Keep `main.rs` and `lib.rs` thin: entrypoint wiring, module declarations, and re-exports only.
- Split files when they mix multiple domains, including transport concerns, protocol mapping, runtime orchestration, and device/business behavior.
- If a file is over 150 lines and a change adds a new responsibility, extract a dedicated module instead of appending more logic.
- Prefer named structs/enums for cross-module data over tuple types.
- Keep functions focused. If a function grows beyond roughly 50 lines and performs multiple steps, split into helpers.
- When adding behavior, place code in an existing concern-specific module when possible; otherwise create a new module with a clear name.
- Preserve stable public APIs while refactoring by re-exporting moved types/functions from crate roots when needed.
- After structural refactors, update architecture documentation in `docs/` during the same task.
