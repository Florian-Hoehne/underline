# Test Cases: Restore Snapshot

**Use Case:** [UC-026 Restore Snapshot](../use_cases/UC-026-restore-snapshot.md)  
**Requirements:** [FR-101, FR-102, NFR-005, NFR-006, NFR-011, NFR-014, NFR-015](../requirements.md)

## Coverage

| Test case | Flow / rule                                          | Requirements            |
|-----------|------------------------------------------------------|-------------------------|
| TC-026-01 | Main success scenario: compare and restore           | FR-102, BR-004          |
| TC-026-02 | Snapshots while editing                              | FR-101, BR-001          |
| TC-026-03 | Snapshots for changes from outside, agents, and git  | BR-001                  |
| TC-026-04 | No snapshot of the same content                      | BR-001                  |
| TC-026-05 | A1 No snapshots                                      | —                       |
| TC-026-06 | A2 Look at the whole snapshot                        | —                       |
| TC-026-07 | A3 Undo the restore                                  | BR-004                  |
| TC-026-08 | A4 Note changes while the snapshots are shown        | —                       |
| TC-026-09 | A5 Note cannot be stored                             | UC-005 A14              |
| TC-026-10 | A6 User cancels                                      | —                       |
| TC-026-11 | Kept 30 days, on this computer only                  | BR-002, NFR-014, NFR-015 |
| TC-026-12 | Renamed and deleted notes                            | BR-003                  |
| TC-026-13 | Without git and with git                             | BR-005                  |
| TC-026-14 | Keyboard alone                                       | BR-006, NFR-011         |
| TC-026-15 | A7 Remove all snapshots                              | BR-002, BR-003, BR-007  |

## TC-026-01: Compare and restore a snapshot

**Flow:** Main success scenario, BR-004  
**Preconditions:** The workspace `notes` is no git repository. `Plan.md` held "Version A" at 09:00 and was changed by the user to "Version B" at 09:30, which took a snapshot with the cause Editing holding "Version A". "Plan" is open.

| Step | Action                                              | Expected result                                                                    |
|------|-----------------------------------------------------|------------------------------------------------------------------------------------|
| 1    | Ask for the snapshots of "Plan" from its tab.        | The list shows the snapshot of 09:30, cause Editing, differing in 1 line.          |
| 2    | Choose it.                                          | "Version A" is marked as taken away and "Version B" as added; nothing changes.    |
| 3    | Restore it.                                         | The tab shows "Version A"; the status bar says the note was restored to the snapshot of 09:30 and offers to undo. |
| 4    | Look at `Plan.md` and the list of snapshots.        | `Plan.md` holds "Version A"; a new snapshot with the cause Restore holds "Version B". |
| 5    | Press Ctrl+Z in the tab.                            | "Version B" is back.                                                               |

## TC-026-02: Snapshots while editing

**Flow:** BR-001, FR-101  
**Preconditions:** `Log.md` holds "start" and has no snapshot.

| Step | Action                                                       | Expected result                                                      |
|------|--------------------------------------------------------------|----------------------------------------------------------------------|
| 1    | At 10:00, type "one" into "Log".                              | A snapshot of 10:00 with the cause Editing holds "start".            |
| 2    | At 10:05, type "two".                                         | No new snapshot.                                                     |
| 3    | At 10:20, type "three".                                       | A new snapshot holds the content from before "three".               |

## TC-026-03: Snapshots for changes from outside, agents, and git

**Flow:** BR-001  
**Preconditions:** "Plan" is open; an agent has read and write access (UC-017); the workspace is a git repository with a remote.

| Step | Action                                                         | Expected result                                                    |
|------|----------------------------------------------------------------|--------------------------------------------------------------------|
| 1    | Change `Plan.md` with another program.                         | A snapshot with the cause Outside change holds the content before. |
| 2    | Let the agent change "Plan" (UC-019).                          | A snapshot with the cause Agent change holds the content before.   |
| 3    | Pull a change of `Plan.md` from the remote (UC-014).           | A snapshot with the cause Git holds the content before.            |
| 4    | Replace a word of "Plan" in the workspace (UC-022).            | A snapshot with the cause Replace in workspace holds the content before. |

## TC-026-04: No snapshot of the same content

**Flow:** BR-001  
**Preconditions:** The newest snapshot of "Plan" holds "Version A"; the note holds "Version A".

| Step | Action                                               | Expected result                  |
|------|------------------------------------------------------|----------------------------------|
| 1    | 15 minutes later, type a letter into "Plan".         | No new snapshot is taken.        |

## TC-026-05: No snapshots

**Flow:** A1  
**Preconditions:** `New.md` was never changed in Underline.

| Step | Action                                      | Expected result                                                  |
|------|---------------------------------------------|------------------------------------------------------------------|
| 1    | Ask for the snapshots of "New".             | The system says there is no snapshot yet and when snapshots are taken. |

## TC-026-06: Look at the whole snapshot

**Flow:** A2  
**Preconditions:** As TC-026-01; the snapshot holds a table and a heading.

| Step | Action                                          | Expected result                                                       |
|------|-------------------------------------------------|-----------------------------------------------------------------------|
| 1    | Choose the snapshot and ask to see it whole.     | It is shown rendered, read only, with its table and heading.          |
| 2    | Select the heading and copy it.                 | The clipboard holds it; the snapshot and the note are unchanged.     |

## TC-026-07: Undo the restore

**Flow:** A3, BR-004  
**Preconditions:** TC-026-01 step 3 was just done.

| Step | Action                                  | Expected result                     |
|------|-----------------------------------------|-------------------------------------|
| 1    | Choose undo in the status bar.          | `Plan.md` holds "Version B" again.  |

## TC-026-08: Note changes while the snapshots are shown

**Flow:** A4  
**Preconditions:** As TC-026-01; the snapshot of 09:30 is chosen.

| Step | Action                                                  | Expected result                                                 |
|------|---------------------------------------------------------|-----------------------------------------------------------------|
| 1    | Change `Plan.md` to "Version C" with another program.   | The differences show "Version C" as added; a snapshot with the cause Outside change is listed. |

## TC-026-09: Note cannot be stored

**Flow:** A5  
**Preconditions:** As TC-026-01; `Plan.md` may only be read.

| Step | Action                                  | Expected result                                                                  |
|------|-----------------------------------------|----------------------------------------------------------------------------------|
| 1    | Restore the snapshot of 09:30.          | The tab shows "Version A", marked as not stored; the status bar says why.        |
| 2    | Look at `Plan.md`.                      | It still holds "Version B", whole.                                               |

## TC-026-10: Cancel

**Flow:** A6  
**Preconditions:** As TC-026-01.

| Step | Action                                                  | Expected result                    |
|------|---------------------------------------------------------|------------------------------------|
| 1    | Open the snapshots, choose one, and close them.         | The note and its snapshots are unchanged. |

## TC-026-11: Kept 30 days, on this computer only

**Flow:** BR-002, NFR-014, NFR-015  
**Preconditions:** "Plan" has a snapshot of 1 October 2026 and one of 1 November 2026; today is 11 November 2026; the network is off; the workspace is a git repository.

| Step | Action                                              | Expected result                                                  |
|------|-----------------------------------------------------|------------------------------------------------------------------|
| 1    | Ask for the snapshots of "Plan".                    | Only the snapshot of 1 November is listed.                       |
| 2    | Look at the changes of the workspace (UC-011).      | No snapshot is shown as a change.                                |
| 3    | Search the workspace for a word only a snapshot holds (UC-008). | Nothing is found.                                  |

## TC-026-12: Renamed and deleted notes

**Flow:** BR-003  
**Preconditions:** "Plan" has two snapshots.

| Step | Action                                         | Expected result                                       |
|------|------------------------------------------------|-------------------------------------------------------|
| 1    | Rename "Plan" to "Roadmap" (UC-006).           | "Roadmap" lists the two snapshots.                    |
| 2    | Delete "Roadmap" and restore it (UC-007).      | "Roadmap" lists the two snapshots again.              |

## TC-026-13: Without git and with git

**Flow:** BR-005  
**Preconditions:** One workspace that is no git repository and one that is, each with an open note.

| Step | Action                                                         | Expected result                                  |
|------|----------------------------------------------------------------|--------------------------------------------------|
| 1    | Edit each note at 10:00 and at 10:20.                          | Both notes have two snapshots with the cause Editing. |
| 2    | In the git repository, commit (UC-012) and look at the snapshots. | The snapshots are unchanged by the commit.   |

## TC-026-14: Keyboard alone

**Flow:** BR-006, NFR-011  
**Preconditions:** As TC-026-01; the focus is in "Plan".

| Step | Action                                                                 | Expected result                         |
|------|------------------------------------------------------------------------|-----------------------------------------|
| 1    | Run the command for the snapshots from the command palette (UC-023).    | The list of snapshots opens.            |
| 2    | Choose a snapshot with the arrow keys, see its differences, restore it.  | The note is restored.                   |

## TC-026-15: Remove all snapshots

**Flow:** A7, BR-002, BR-003, BR-007  
**Preconditions:** The workspace `notes` keeps 2 snapshots of "Plan" and "Trip" taking 20 bytes; the deleted note "Old" has a snapshot.

| Step | Action                                                         | Expected result                                                                      |
|------|----------------------------------------------------------------|--------------------------------------------------------------------------------------|
| 1    | Open the settings of a workspace without snapshots.            | "Snapshots: None"; removing them all cannot be chosen.                               |
| 2    | Open the settings of `notes`, part "This workspace".           | "20 bytes in 2 snapshots" and "Kept on this computer for 30 days".                   |
| 3    | Ask to remove all snapshots, then press Escape.                | The question names "20 bytes in 2 snapshots" and says it cannot be undone; after Escape nothing is removed and the settings stay open. |
| 4    | Ask again and confirm.                                         | The status bar says all snapshots were removed, 20 bytes in 2 snapshots; the settings show "None". |
| 5    | Ask for the snapshots of "Plan"; restore "Old" and ask for its snapshots. | "Plan" has no snapshot (A1); "Old" lists its snapshot.                     |
| 6    | Change "Plan".                                                 | A new snapshot of "Plan" is taken (BR-001).                                          |
