# Test Cases: Synchronize with Remote

**Use Case:** [UC-014 Synchronize with Remote](../use_cases/UC-014-synchronize-with-remote.md)  
**Requirements:** [FR-020, FR-045, FR-052, FR-053, NFR-002, NFR-006, NFR-014, NFR-015](../requirements.md)

## Coverage

| Test case | Flow / rule                                           | Requirements             |
|-----------|-------------------------------------------------------|--------------------------|
| TC-014-01 | Main success scenario: Sync now, both sides new       | FR-045, BR-003           |
| TC-014-02 | Changes committed before a sync, marked Auto          | BR-002                   |
| TC-014-03 | Changes left out stay out                             | BR-002                   |
| TC-014-04 | When it syncs                                         | BR-001, FR-045           |
| TC-014-05 | Status bar                                            | BR-004, FR-020           |
| TC-014-06 | Joining as the user's git setup says                  | BR-003                   |
| TC-014-07 | A1 Pull only, push only                               | FR-052, FR-053           |
| TC-014-08 | A2 Nothing to do                                      | —                        |
| TC-014-09 | A3 Conflict                                           | —                        |
| TC-014-10 | A4 No network                                         | NFR-014                  |
| TC-014-11 | A5 Sign-in missing                                    | BR-005                   |
| TC-014-12 | A6 New branch                                         | BR-007                   |
| TC-014-13 | A7 Sync fails otherwise                               | NFR-006                  |
| TC-014-14 | A8 A sync is running                                  | —                        |
| TC-014-15 | A9 Closing                                            | —                        |
| TC-014-16 | A10 No sync possible                                  | —                        |
| TC-014-17 | A11 A change left out is in the way                   | BR-002                   |
| TC-014-18 | Typing goes on, open notes follow                     | BR-008, NFR-002          |
| TC-014-19 | Only the remote receives notes                        | BR-006, NFR-015          |

## TC-014-01: Sync now with new commits on both sides

**Flow:** Main success scenario, BR-003  
**Preconditions:** The workspace `notes` on `main` follows `origin/main`; a local commit changed `Plan.md`; another clone pushed a commit changing `Ideas.md`.

| Step | Action                                   | Expected result                                                                      |
|------|------------------------------------------|--------------------------------------------------------------------------------------|
| 1    | Choose "Sync now" in the status bar.      | The status bar shows "Syncing…", then "Synced just now".                            |
| 2    | Look at "Ideas".                          | It holds the change of the other clone.                                              |
| 3    | Run `git log origin/main` in a terminal.  | It holds both commits.                                                               |

## TC-014-02: Changes committed before a sync, marked Auto

**Flow:** Step 2, BR-002  
**Preconditions:** `Plan.md` and `Ideas.md` changed and are not committed; `Plan.md` has a typed word not yet saved.

| Step | Action               | Expected result                                                                 |
|------|----------------------|---------------------------------------------------------------------------------|
| 1    | Choose "Sync now".    | `git log -1` shows "Auto: Update Plan, Ideas" holding both notes, with the typed word; the remote has it. |

## TC-014-03: Changes left out stay out

**Flow:** BR-002  
**Preconditions:** `Plan.md` and `Ideas.md` changed; "Ideas" is unchecked in the commit field (UC-012).

| Step | Action               | Expected result                                                        |
|------|----------------------|------------------------------------------------------------------------|
| 1    | Choose "Sync now".    | The automatic commit holds only `Plan.md`; "Ideas" is still changed.   |

## TC-014-04: When it syncs

**Flow:** BR-001, FR-045  
**Preconditions:** The sync interval is 1 minute; a second clone can push.

| Step | Action                                          | Expected result                                                  |
|------|-------------------------------------------------|------------------------------------------------------------------|
| 1    | Open the workspace.                              | A sync runs at once.                                             |
| 2    | Push a commit from the other clone, wait 70 s.   | The commit arrives without asking.                               |
| 3    | Commit a change in Underline (UC-012).           | A sync runs right after; the remote has the commit.              |
| 4    | Set the interval to 5 minutes and wait 2 minutes. | No sync runs.                                                   |

## TC-014-05: Status bar

**Flow:** BR-004, FR-020  
**Preconditions:** The last sync succeeded 2 minutes ago; then 2 local commits were made, and the other clone pushed 1.

| Step | Action                                   | Expected result                                                  |
|------|------------------------------------------|------------------------------------------------------------------|
| 1    | Look at the status bar before the next fetch. | "Synced 2 min ago" and "↑2".                                 |
| 2    | Choose Pull in the branch menu.           | After the pull "↑2" stays and nothing waits to be pulled.       |

## TC-014-06: Joining as the user's git setup says

**Flow:** BR-003  
**Preconditions:** Both sides have a new commit changing different notes.

| Step | Action                                                   | Expected result                                          |
|------|----------------------------------------------------------|----------------------------------------------------------|
| 1    | Without `pull.rebase`, sync.                              | The history holds a merge commit.                       |
| 2    | Set `git config pull.rebase true`, repeat with new commits. | The local commit is on top of the remote's; no merge commit. |

## TC-014-07: Pull only, push only

**Flow:** A1, FR-052, FR-053  
**Preconditions:** One local commit; the remote has one new commit.

| Step | Action                         | Expected result                                                                  |
|------|--------------------------------|----------------------------------------------------------------------------------|
| 1    | Choose Push in the branch menu. | The remote refuses; the status bar says so and offers to sync.                 |
| 2    | Choose Pull.                    | The remote's commit arrives; nothing is pushed.                                 |
| 3    | Choose Push.                    | The local commit reaches the remote.                                            |

## TC-014-08: Nothing to do

**Flow:** A2  
**Preconditions:** Both branches hold the same commits; nothing changed.

| Step | Action               | Expected result                                               |
|------|----------------------|---------------------------------------------------------------|
| 1    | Choose "Sync now".    | No commit is made or pushed; the status bar shows "Synced just now". |

## TC-014-09: Conflict

**Flow:** A3  
**Preconditions:** Line 3 of `Plan.md` was changed and committed locally and differently on the remote.

| Step | Action               | Expected result                                                             |
|------|----------------------|-----------------------------------------------------------------------------|
| 1    | Choose "Sync now".    | Nothing is pushed; the status bar names "Plan" as in conflict (UC-015).    |

## TC-014-10: No network

**Flow:** A4, NFR-014  
**Preconditions:** The network is off; a local commit waits.

| Step | Action               | Expected result                                                      |
|------|----------------------|----------------------------------------------------------------------|
| 1    | Choose "Sync now".    | The status bar shows "Offline, will retry" with the last sync's time. |
| 2    | Turn the network on and wait for the interval. | The commit reaches the remote.                      |

## TC-014-11: Sign-in missing

**Flow:** A5, BR-005  
**Preconditions:** The remote is a private HTTPS repository; git has no saved sign-in.

| Step | Action               | Expected result                                                                     |
|------|----------------------|-------------------------------------------------------------------------------------|
| 1    | Choose "Sync now".    | The status bar says signing in to the remote failed and how to set it up in git; no password is asked for. |

## TC-014-12: New branch

**Flow:** A6, BR-007  
**Preconditions:** The branch `trip` was created in Underline (UC-013) and has a commit; the workspace has the remotes `backup` and `origin`.

| Step | Action               | Expected result                                                          |
|------|----------------------|--------------------------------------------------------------------------|
| 1    | Choose "Sync now".    | `origin` has `trip`; `git status -sb` shows `trip...origin/trip`.        |

## TC-014-13: Sync fails otherwise

**Flow:** A7, NFR-006  
**Preconditions:** The remote rejects pushes to `main` (protected branch); a local commit waits.

| Step | Action               | Expected result                                                               |
|------|----------------------|-------------------------------------------------------------------------------|
| 1    | Choose "Sync now".    | "Sync failed" with the remote's reason, in a warning color; the commit stays. |

## TC-014-14: A sync is running

**Flow:** A8  
**Preconditions:** The remote answers slowly.

| Step | Action                                   | Expected result                                                 |
|------|------------------------------------------|-----------------------------------------------------------------|
| 1    | Choose "Sync now" three times quickly.    | One sync runs; after it ends, one more runs.                   |

## TC-014-15: Closing

**Flow:** A9  
**Preconditions:** A change is saved but not committed.

| Step | Action                                           | Expected result                                                     |
|------|--------------------------------------------------|---------------------------------------------------------------------|
| 1    | Close Underline.                                  | It syncs, then closes; the remote has the change.                  |
| 2    | Turn the remote's answers into a 60 s delay and close again. | Underline closes after 10 s; the change is committed locally. |

## TC-014-16: No sync possible

**Flow:** A10  
**Preconditions:** Three workspaces: one without a remote, one not on a branch, one with a merge waiting.

| Step | Action                            | Expected result                                       |
|------|-----------------------------------|-------------------------------------------------------|
| 1    | Open each and look at the status bar. | "Sync now" is not offered; asking says why.       |

## TC-014-17: A change left out is in the way

**Flow:** A11, BR-002  
**Preconditions:** "Plan" is changed and unchecked in the commit field; the remote has a new commit changing `Plan.md`.

| Step | Action               | Expected result                                                             |
|------|----------------------|-----------------------------------------------------------------------------|
| 1    | Choose "Sync now".    | The sync stops, names "Plan", and offers to commit or discard the change.  |

## TC-014-18: Typing goes on, open notes follow

**Flow:** BR-008, NFR-002  
**Preconditions:** "Ideas" is open; the remote changed `Ideas.md`; the remote answers slowly.

| Step | Action                                   | Expected result                                                   |
|------|------------------------------------------|-------------------------------------------------------------------|
| 1    | Choose "Sync now" and type in "Plan".     | Every character appears at once.                                 |
| 2    | Look at "Ideas" after the sync.           | It shows the remote's change.                                    |

## TC-014-19: Only the remote receives notes

**Flow:** BR-006, NFR-015  
**Preconditions:** A network monitor runs.

| Step | Action                                         | Expected result                                               |
|------|------------------------------------------------|---------------------------------------------------------------|
| 1    | Work with notes for 10 minutes with syncs.      | Underline's only connections for git go to the remote's host. |
