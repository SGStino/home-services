# Home Services Workspace Instructions

- Keep instructions concise and focused on current development work.
- Do not include project creation, scaffolding, or initialization steps in responses; this repository is already created and actively in development.
- Whenever the user provides new or updated architectural information, immediately update the relevant documentation files in docs/ during the same task.
- Treat architecture documentation synchronization as required work, not optional follow-up.
- If architecture details are ambiguous, ask a clarifying question before finalizing documentation changes.
- At the end of architecture-related changes, summarize which documentation files were updated.
- Keep the codebase organized and modular: split features by responsibility and avoid concentrating large amounts of unrelated logic in a single file (for example, an oversized `lib.rs`).
- In this dev-container, Docker/Podman container CLI commands are not available from inside the workspace runtime (to avoid docker-in-docker or docker-in-podman assumptions); prefer host-side container execution guidance when needed.
- Follow existing project conventions and avoid unrelated refactors.