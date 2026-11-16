# Use Case: Edit Note Attributes

## Overview

**Use Case ID:** UC-024  
**Use Case Name:** Edit Note Attributes  
**Primary Actor:** User  
**Goal:** The user keeps data about a note, such as its tags, its status, or a date, as note attributes, seen and changed as fields in the note properties, without writing the markup of the front matter or of AsciiDoc document attributes.  
**Trigger:** The user adds, changes, or removes a note attribute in the note properties of the active note.  
**Status:** Reviewed  

**Requirements:** [FR-091, FR-093, NFR-005, NFR-006, NFR-007, NFR-011, C-003, C-004](../requirements.md)

## Preconditions

- A note is open in the active tab (UC-004).

## Main Success Scenario

1. The user looks at the note properties of the active note in the tool panel.
2. The system shows, below the place, note format, size, last change, and text statistics of the note, its note attributes as fields in the order of the note, each with its name and its value as its kind shows it (BR-002).
3. The user adds a note attribute with a name, a kind, and a value, changes the value of one, or removes one.
4. The system writes the change into the head of the note in the markup of its note format (BR-001), changing nothing else in the note (BR-003), and shows it at once in both editing views and in the note properties.
5. The system stores the note as autosave stores it (UC-005 BR-001).
6. The user goes on; use case continues at step 3 until the user leaves the note properties.

## Alternative Flows

### A1: First note attribute

**Trigger:** The note has no note attribute yet and the user adds one (step 3)  
**Flow:**

1. In a Markdown note, the system adds a front matter at the very start of the note holding that attribute; in an AsciiDoc note, it adds the attribute to the header, below the document title if the note has one.
2. Use case continues at step 5.

### A2: Last note attribute removed

**Trigger:** The user removes the only note attribute of the note (step 3)  
**Flow:**

1. In a Markdown note, the system removes the whole front matter; in an AsciiDoc note, it removes the attribute and keeps the document title.
2. Use case continues at step 5.

### A3: Name not allowed or taken

**Trigger:** The name of a new note attribute is empty, breaks BR-004, or is the name of an attribute the note has already (step 3)  
**Flow:**

1. The system says why the name is not allowed, keeps what the user typed, and writes nothing.
2. Use case continues at step 3.

### A4: Value does not fit its kind

**Trigger:** The value of a note attribute of the kind Number or Date is no number or no date (step 3)  
**Flow:**

1. The system says what the value must look like, keeps what the user typed, and writes nothing.
2. Use case continues at step 3.

### A5: Head of the note cannot be read

**Trigger:** The front matter of a Markdown note breaks the notation GitHub reads, for example a line without a name (step 2)  
**Flow:**

1. The system shows the front matter as its text with the line and the reason of the error instead of fields, offers to show it in the source view, and changes nothing.
2. Use case ends.

### A6: Nested value

**Trigger:** A note attribute holds fields of its own or a list of such fields (step 2)  
**Flow:**

1. The system shows its value as its markup, marked as changeable only in the source view, and offers to show it there.
2. Use case continues at step 3 for the other note attributes.

### A7: Note changed outside

**Trigger:** The note is changed by another program, by git, or by an agent while the note properties are shown (step 3)  
**Flow:**

1. The system shows the note attributes of the new content; a value the user is typing stays in its field and is written into the new content when the user confirms it.
2. Use case continues at step 3.

## Postconditions

### Success Postconditions

- The note holds the added, changed, or removed note attribute in the markup of its note format, and everything else in the note is unchanged, byte for byte.
- The tags of the note follow its note attribute `tags` (UC-004 BR-016).

### Failure Postconditions

- A head of a note that cannot be read is never rewritten.
- A note is never half written.

## Business Rules

### BR-001: Where the note attributes are

In a Markdown note, the note attributes are the front matter: a block at the very start of the note between two lines of three dashes, with one attribute per line as `name: value`, in the notation GitHub reads and shows as a table (C-003). In an AsciiDoc note, they are the document attributes of the header, one per line as `:name: value`, directly below the document title or at the start of a note without one (C-004). Anything else in the note is no note attribute.

### BR-002: Kinds of values

A note attribute is of one of these kinds; the system shows each the way it is written:

| Kind      | Written as                                                        | Shown as                    |
|-----------|-------------------------------------------------------------------|-----------------------------|
| Text      | any text on one line                                              | a text field                |
| List      | items in square brackets, or one item per line starting with a dash; in AsciiDoc, items separated by commas | a field per item |
| Number    | a whole or decimal number                                         | a number field              |
| Date      | year, month, and day as `2026-11-11`                              | a date with a calendar      |
| Yes or no | `true` or `false`                                                 | a switch                    |
| Nested    | fields of its own, indented below the name                        | its markup, read only (A6)  |

An AsciiDoc note holds only the kinds Text and List. When the user adds a note attribute, the kind is Text unless they choose another.

### BR-003: Only the note attribute changes

A change writes only the line or lines of that note attribute; the order of the attributes, comments, blank lines, and the way the other values are written stay byte for byte as they were, as an edit in the facade keeps every other block (NFR-007). A new note attribute is added after the last one.

### BR-004: Names

The name of a note attribute holds letters, digits, `_`, and `-`, starts with a letter, and has at most 100 characters. No two note attributes of a note have the same name. In an AsciiDoc note, names are written in lower case, as AsciiDoc reads them.

### BR-005: Tags as a note attribute

The note attribute `tags` is a list of tags of the note (UC-004 BR-016). In a Markdown note, a tag in the text is shown among the tags of the note but is not part of this note attribute; it is changed in the text.

### BR-006: Like any edit

A change of a note attribute is a change of the note like any edit (UC-005): it is stored within 2 seconds (NFR-005), never half written (NFR-006), and can be undone in the tab like any change (UC-005 BR-004).

### BR-007: Keyboard

The note attributes can be reached in the tool panel, added, changed, and removed by keyboard alone (NFR-011): Tab moves from field to field and to the remove button beside each note attribute, and Enter confirms a value.
