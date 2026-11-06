# ADR-006: Run git through the installed git program

**Status:** Accepted
**Date:** 2026-11-06
**Requirements:** C-007, NFR-015, FR-072

## Context

Underline uses the access to git remotes the user has already set up and keeps no passwords of its own (C-007).
Note content leaves the machine only through git push to the workspace's remote (NFR-015). A new workspace can be
created as a git repository with a first commit on the user's default branch (FR-072, UC-001 BR-007).

## Decision

- All git operations (init, status, diff, branch, commit, pull, push, git sync) run the `git` program installed on
  the machine, found on the `PATH`, in `crates/underline-core` behind one typed interface.
- git therefore applies the user's own configuration: the credential helper for HTTPS remotes, the SSH agent and SSH
  configuration for SSH remotes, `init.defaultBranch`, and `user.name` / `user.email`.
- Underline stores no tokens, passwords, or commit identity. When git is missing, authentication fails, or git has no
  user name and email, the status bar says so and how to fix it in git; the operation is not done (UC-001 A5).
- Output that Underline parses is requested in git's machine-readable forms (`--porcelain`, `-z`); git is run with
  `GIT_TERMINAL_PROMPT=0` so it never waits for input.

## Consequences

- A workspace works in Underline exactly when it works with `git` on the command line.
- git must be installed; this is common for the target users but not guaranteed on Windows and macOS, so the message
  for a missing git has to be clear.
- **Alternatives considered:** `git2` (libgit2) needs credential helper and SSH agent support wired up by hand and
  ignores parts of the git configuration; `gitoxide` does not support push completely yet.
