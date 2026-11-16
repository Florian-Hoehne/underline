# Use Case: Run Command

## Overview

**Use Case ID:** UC-023  
**Use Case Name:** Run Command  
**Primary Actor:** User  
**Goal:** The user finds any command of Underline by typing a part of its name, learns its key, and runs it without the mouse and without knowing the menu it is in.  
**Trigger:** The user opens the command palette.  
**Status:** Reviewed  

**Requirements:** [FR-077, NFR-011, NFR-013, NFR-018](../requirements.md)

## Preconditions

- Underline is running.

## Main Success Scenario

1. The user opens the command palette with Ctrl+Shift+P (Cmd+Shift+P on macOS) or F1 and types a part of the name of a command.
2. While the user types, the system lists the commands that match (BR-002), each with its name, the menu it belongs to, and its key if it has one, the best match first and chosen.
3. The user chooses a command with the arrow keys and Enter, or with a click.
4. The system closes the command palette and runs the command as its own use case describes, for the active note and the pane the user was in before, and returns the focus there.

## Alternative Flows

### A1: No command matches

**Trigger:** No command matches what the user typed (step 2)  
**Flow:**

1. The system says that no command matches.
2. Use case continues at step 1.

### A2: Command not available now

**Trigger:** The user chooses a command that cannot run at the moment, such as a git command in a workspace that is no git repository or a command for the active note while no tab is open (step 3)  
**Flow:**

1. The system keeps the command palette open, runs nothing, and says why the command is not available.
2. Use case continues at step 1.

### A3: Command asks for a choice

**Trigger:** The chosen command needs a choice before it runs, such as the theme, the input mode, or the note format of an export (step 4)  
**Flow:**

1. The system lists the choices in the same field, with the current one marked, and the user narrows them by typing and chooses one.
2. The system runs the command with that choice and closes the command palette.
3. Use case ends.

### A4: Switch to go to note

**Trigger:** The user removes the `>` at the start of the field of the command palette (step 1)  
**Flow:**

1. The system turns the field into go to note (UC-004 A12), keeping the text typed after the `>`.
2. Use case ends.

### A5: User cancels

**Trigger:** The user presses Escape or clicks outside the command palette (step 3)  
**Flow:**

1. The system closes the command palette, runs nothing, and returns the focus to where it was.
2. Use case ends.

## Postconditions

### Success Postconditions

- The chosen command has run, with the result its own use case describes.
- The command is listed first the next time the command palette opens with an empty field.

### Failure Postconditions

- No command runs unless the user chose it and it is available; the command palette itself changes no note and no setting.

## Business Rules

### BR-001: Every command

The command palette offers every command of Underline: those of the menus, the toolbar, the tool panel, and the context menus of the note overview, the active tab, and the active note, and every key of Underline that is no key of typing or moving in the text. A command has the same name as in its menu. A command for a note runs for the active note. The keys of the Vim and Nano input modes are not commands of the palette (UC-005 BR-009, BR-010).

### BR-002: What matches

Every word typed occurs in the name of the command or in the name of its menu, in any order, also as a part of a word; upper and lower case do not matter. Commands whose name starts with what was typed come first, then those with a word that starts with it, then the others; within each group, the most recently run command comes first. A command that is not available now is listed after the others, marked as not available.

### BR-003: Keys shown

Each command shows its key as it is on the platform (`Ctrl` on Linux and Windows, `Cmd` on macOS), so the palette teaches the keys.

### BR-004: Recently run

With an empty field, the command palette lists the five most recently run commands first, the last one on top, followed by all others by name. The commands run are remembered until Underline is closed.

### BR-005: Same as from the menu

A command run from the command palette does exactly what it does from its menu or key, with the same questions and the same reports. The command palette only finds and starts it.

### BR-006: Speed

The matching commands appear within 200 milliseconds of a key (NFR-018).

### BR-007: Keyboard

Ctrl+Shift+P (Cmd+Shift+P on macOS) and F1 open the command palette from anywhere in the window, also in the Vim and Nano input modes. The field starts with `>`; without it, the field is go to note (UC-004 A12), and typing `>` as the first character in go to note turns it into the command palette. The arrow keys move through the list, Enter runs the chosen command, and Escape closes the palette (NFR-011).
