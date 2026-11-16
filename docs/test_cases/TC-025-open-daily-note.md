# Test Cases: Open Daily Note

**Use Case:** [UC-025 Open Daily Note](../use_cases/UC-025-open-daily-note.md)  
**Requirements:** [FR-094, FR-095, FR-018, NFR-006, NFR-011, NFR-013](../requirements.md)

## Coverage

| Test case | Flow / rule                                          | Requirements            |
|-----------|------------------------------------------------------|-------------------------|
| TC-025-01 | Main success scenario: create from the template      | FR-095, BR-001 to BR-004 |
| TC-025-02 | Without a daily note template                        | BR-003, BR-004          |
| TC-025-03 | A1 Daily note exists                                 | BR-006                  |
| TC-025-04 | A2 Daily note of another day                         | —                       |
| TC-025-05 | A3 Daily note template not found                     | —                       |
| TC-025-06 | A4 Daily note folder not allowed                     | BR-002                  |
| TC-025-07 | A5 Daily note cannot be created                      | NFR-006                 |
| TC-025-08 | Other folder and template from the workspace settings | BR-002, BR-003         |
| TC-025-09 | A note like any other                                | BR-005                  |
| TC-025-10 | The day changes at midnight                          | BR-001                  |
| TC-025-11 | Keyboard alone                                       | BR-007, NFR-011         |

## TC-025-01: Create the daily note from the template

**Flow:** Main success scenario, BR-001 to BR-004  
**Preconditions:** The Markdown workspace `notes` has no folder `daily`. `templates/Daily.adoc` holds `= {{title}}`, an empty line, `{{cursor}}`, and `== Done`. The date of the computer is 11 November 2026, 08:15.

| Step | Action                                 | Expected result                                                                                       |
|------|----------------------------------------|-------------------------------------------------------------------------------------------------------|
| 1    | Press Ctrl+Shift+J.                    | "2026-11-11" opens in a tab in the facade and is listed first; the status bar says it was created.   |
| 2    | Look at the cursor.                    | It stands on the empty line above "Done".                                                            |
| 3    | Look at `daily/2026-11-11.adoc`.       | It holds `= 2026-11-11`, an empty line, and `== Done`; the folder `daily` was created.               |

## TC-025-02: Without a daily note template

**Flow:** BR-003, BR-004  
**Preconditions:** The Markdown workspace `notes` has no template folder.

| Step | Action                              | Expected result                                                    |
|------|-------------------------------------|--------------------------------------------------------------------|
| 1    | Ask for the daily note.             | `daily/2026-11-11.md` holds only `# 2026-11-11`; the cursor is at its end. |

## TC-025-03: Daily note exists

**Flow:** A1, BR-006  
**Preconditions:** TC-025-01 was done; "today" was typed into the daily note; its tab was closed.

| Step | Action                              | Expected result                                                  |
|------|-------------------------------------|------------------------------------------------------------------|
| 1    | Ask for the daily note.             | "2026-11-11" opens with "today" in it; the cursor is at its end; nothing is created. |
| 2    | Ask again while it is open.         | Its tab becomes the active tab.                                  |

## TC-025-04: Daily note of another day

**Flow:** A2  
**Preconditions:** `daily` holds `2026-11-05.md` and `2026-11-11.md`; "2026-11-11" is in the active tab.

| Step | Action                                      | Expected result                                                  |
|------|---------------------------------------------|------------------------------------------------------------------|
| 1    | Press Ctrl+Shift+PageUp.                    | "2026-11-05" opens; no note is created for the days between.    |
| 2    | Press Ctrl+Shift+PageUp again.              | The status bar says there is no earlier daily note.              |
| 3    | Press Ctrl+Shift+PageDown twice.            | "2026-11-11" opens, then the status bar says there is no later one. |

## TC-025-05: Daily note template not found

**Flow:** A3  
**Preconditions:** The workspace settings name the daily note template `templates/Journal-Day.md`, which does not exist.

| Step | Action                    | Expected result                                                                    |
|------|---------------------------|------------------------------------------------------------------------------------|
| 1    | Ask for the daily note.   | It is created holding only its title; the status bar says the template was not found. |

## TC-025-06: Daily note folder not allowed

**Flow:** A4, BR-002  
**Preconditions:** The workspace settings name the daily note folder `.days`.

| Step | Action                    | Expected result                                                                       |
|------|---------------------------|---------------------------------------------------------------------------------------|
| 1    | Ask for the daily note.   | The system says the folder is hidden, creates nothing, and offers to open the settings. |

## TC-025-07: Daily note cannot be created

**Flow:** A5, NFR-006  
**Preconditions:** The workspace directory may only be read; there is no folder `daily`.

| Step | Action                    | Expected result                                                         |
|------|---------------------------|-------------------------------------------------------------------------|
| 1    | Ask for the daily note.   | The status bar says the daily note could not be created and why.       |
| 2    | Look at the workspace.    | No folder `daily` and no note were created.                             |

## TC-025-08: Folder and template from the workspace settings

**Flow:** BR-002, BR-003  
**Preconditions:** The workspace settings name the daily note folder `journal/days` and the daily note template `templates/Day.md`, which holds `# {{date}}` and `Written at {{time}}`; the time is 08:15.

| Step | Action                    | Expected result                                                                  |
|------|---------------------------|----------------------------------------------------------------------------------|
| 1    | Ask for the daily note.   | `journal/days/2026-11-11.md` holds `# 2026-11-11` and `Written at 08:15`.       |

## TC-025-09: A note like any other

**Flow:** BR-005  
**Preconditions:** TC-025-02 was done.

| Step | Action                                                   | Expected result                                                  |
|------|----------------------------------------------------------|------------------------------------------------------------------|
| 1    | Rename "2026-11-11" to "Kickoff" (UC-006).               | The note is renamed and its links follow.                         |
| 2    | Ask for the daily note.                                  | A new `daily/2026-11-11.md` is created; "Kickoff" is unchanged.  |

## TC-025-10: The day changes at midnight

**Flow:** BR-001  
**Preconditions:** The clock of the computer stands at 23:59 on 11 November 2026 in its time zone; "2026-11-11" exists.

| Step | Action                                     | Expected result                         |
|------|--------------------------------------------|-----------------------------------------|
| 1    | Ask for the daily note.                    | "2026-11-11" opens.                     |
| 2    | Wait until 00:01 and ask again.            | "2026-11-12" is created and opens.      |

## TC-025-11: Keyboard alone

**Flow:** BR-007, NFR-011  
**Preconditions:** As TC-025-04; the focus is in another note.

| Step | Action                                             | Expected result                     |
|------|----------------------------------------------------|-------------------------------------|
| 1    | Press Ctrl+Shift+J, then Ctrl+Shift+PageUp.        | "2026-11-11", then "2026-11-05" opens. |
| 2    | Run "Daily note" from the command palette (UC-023). | "2026-11-11" opens.                 |
