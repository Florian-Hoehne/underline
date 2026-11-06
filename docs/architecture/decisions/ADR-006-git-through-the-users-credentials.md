# ADR-006: Use the user's git credential helper and SSH agent

**Status:** Proposed
**Date:** 2026-11-06
**Requirements:** C-007, NFR-015

## Context

Underline uses the access to git remotes the user has already set up and keeps no passwords of its own (C-007).
Note content leaves the machine only through git push to the workspace's remote (NFR-015).

## Decision

- Git operations (status, diff, branch, commit, pull, push, git sync) use the user's git configuration: the
  configured credential helper for HTTPS remotes and the SSH agent for SSH remotes.
- Underline stores no tokens or passwords; when authentication fails, it reports the error in the status bar.

## Consequences

- No credential storage to secure; a workspace works in Underline exactly when it works with `git` on the command line.
- Whether to call the `git` binary or a library such as gitoxide or libgit2 is open; a library must honour the same
  credential helpers.
