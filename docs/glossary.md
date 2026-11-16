# Glossary

| Term                    | Definition                                                                                                                         | Avoid             |
|-------------------------|------------------------------------------------------------------------------------------------------------------------------------|-------------------|
| User                    | The person who works with Underline on their notes.                                                                                |                   |
| Agent                   | An AI tool, such as Claude Code or Cursor, that reads and writes notes through the MCP server on the user's behalf.               | Bot, Bearbeiter   |
| Underline folder        | The hidden folder `.underline` in a workspace in which Underline remembers its open tabs, recently opened notes, deleted notes, and snapshots; git ignores it by itself. |                   |
| Recently opened workspaces | The workspaces the user opened or created before, listed with the most recently opened first when opening a workspace. |                   |
| Workspace               | A local directory whose Markdown and AsciiDoc files Underline shows as notes; it may be a git repository.                         | Notebook, Vault   |
| Note                    | One Markdown or AsciiDoc file in a workspace, shown and edited in a tab.                                                           | Notiz             |
| Deleted notes           | The notes the user deleted in a workspace, kept on this computer for 30 days so they can be restored; git does not see them. | Trash, Papierkorb |
| Home note               | The note titled Home that every new workspace starts with; the start page when the workspace is a GitHub wiki.                   |                   |
| Note format             | The markup a note is stored in, given by its file extension: Markdown (`.md`) or AsciiDoc (`.adoc`).                               |                   |
| Default format          | The note format a workspace gives to new notes: chosen when the workspace is created, otherwise the format most of its notes have; Markdown unless the user picks another. |                   |
| Facade                  | The editing view that shows a note rendered and lets the user edit it the same way whatever its note format is.                  | WYSIWYG, Fassade  |
| Facade engine           | The part of Underline that stores every edit in the note format of the note, whichever way the user or the agent made it. |                   |
| Format indicator        | The mark in the facade that names the note format of the active note.                                                             |                   |
| Source view             | The editing view that shows and edits the markup of a note as stored in its file.                                                 | Raw mode          |
| Editing view            | Either the source view or the facade, chosen per tab.                                                                              |                   |
| Input mode              | The set of keys the editor follows: Normal, Vim, or Nano. Independent of the editing view.                                        | Edit mode         |
| Tab                     | The area in the middle of the window that shows one open note.                                                                    |                   |
| Note overview           | The panel on the left that lists the notes of the workspace, the recently opened first.                                          | Sidebar           |
| Tool panel              | The panel on the right with the note properties and the tools: diff, outline, code index, figure index, backlinks, outgoing links, tags, todos. |                   |
| Status bar              | The line at the bottom of the window with the workspace directory, the git sync state, the input mode commands, and messages.    |                   |
| Outline                 | The list of the headings of the active note, used to jump to a section.                                                            | Table of contents |
| Code index              | The list of the code blocks of the active note, used to jump to one.                                                               |                   |
| Figure index            | The list of the images and diagrams of the active note, used to jump to one.                                                       |                   |
| Minimap                 | The scaled-down picture of the whole active note beside the editor, showing the visible part.                                     |                   |
| Theme                   | The set of colors of the interface, which rendered diagrams and highlighted code follow.                                          | Skin              |
| Diagram                 | A PlantUML or Mermaid code block of a note, shown as a picture.                                                                    |                   |
| Local link              | A link in a note that points to another note or file of the same workspace.                                                        |                   |
| Autosave                | Writing a changed note to the local disk without a command from the user.                                                          |                   |
| Git remote              | The git repository elsewhere, for example on GitHub or GitLab, that a workspace pulls from and pushes to.                         |                   |
| Git sync                | Pulling from and pushing to the remote of a workspace that is a git repository, at a regular interval.                            |                   |
| MCP server              | The part of Underline that offers the notes of a workspace to an agent over the Model Context Protocol.                           |                   |
| MCP install             | The action that registers the MCP server in the configuration of an agent's tool.                                                 |                   |
| User cursor             | The text cursor of the user in an open note.                                                                                       |                   |
| Agent cursor            | The second cursor in an open note that marks where the agent is reading or writing.                                              | AI cursor         |
| Copy on select          | Copying selected text, an image, or another element to the clipboard as soon as it is selected.                                   |                   |
| Todo                    | A piece of work the user is heading towards, with a title, a due date, an urgency, a scheduled date, and links to notes.          | Task              |
| Todo link               | The link from a todo to one note it is about; a note with a todo link cannot be deleted.                                         |                   |
| Due date                | The date by which a todo has to be done.                                                                                           | Deadline, Frist   |
| Urgency                 | How pressing a todo is: low, normal, high, or critical.                                                                            |                   |
| Scheduled date          | The day the user plans to work on a todo.                                                                                          |                   |
| User settings file      | The `underline.yaml` in the user's home directory with the user's default settings.                                               |                   |
| Workspace settings file | The `underline.yaml` in the root directory of a workspace; its settings override the user settings file for that workspace.      |                   |
| Settings                | Where the user changes the user settings ("For me") and the settings of the open workspace ("This workspace").              | Preferences, Options |
| Export                  | Writing a copy of a note in another format (Markdown, AsciiDoc, HTML, or PDF) without changing the note.                          |                   |
| Note properties         | What the tool panel shows about the active note: its place, note format, size, last change, text statistics, and its note attributes. |                   |
| Note attribute          | A named value kept at the head of a note: a field of the front matter of a Markdown note, or a document attribute in the header of an AsciiDoc note. | Metadata          |
| Front matter            | The block at the very start of a Markdown note, between two lines of three dashes, that holds its note attributes as GitHub reads them. |                   |
| Text statistics         | The number of words and characters of a note or a selection, and its reading time.                                                 |                   |
| Reading time            | The minutes an average reader needs for a text, counted at 200 words a minute and rounded up.                                     |                   |
| Tag                     | A word that groups notes across folders: `#` followed by the word in the text of a Markdown note, or a value of the note attribute `tags`. | Keyword           |
| Backlink                | A local link in another note that leads to the active note, seen from the note it leads to.                                       | Inbound link, Linked mention |
| Outgoing link           | A link the active note holds: a local link, a link to a file of the workspace, or a web address.                                  |                   |
| Broken link             | A local link whose note, file, or heading does not exist in the workspace.                                                        | Dead link         |
| Link completion         | The list of notes and headings the editor suggests while the user types a link, which writes the chosen one as a local link.     |                   |
| Find field              | The field at the top of a tab that finds a text in the note of that tab and, opened for replacing, replaces it.                  | Find bar          |
| Search switch           | One of the three switches of a search: match case, whole word, and regular expression.                                                   |                   |
| Regular expression      | A search text written as a pattern, in the usual notation of programming editors, that can match many different texts.          | Regex             |
| Replace in workspace    | Replacing a text in all notes of the workspace at once, after the user has seen and confirmed every place it changes.            | Global replace    |
| Go to note              | The field that opens a note by a part of its title or folder, typed without leaving the keyboard.                                 | Quick switcher    |
| Command                 | An action of Underline the user can run, such as creating a note, exporting, or switching the theme, with its name and its key, if it has one. |                   |
| Command palette         | The field that lists every command of Underline matching what the user types and runs the chosen one.                            |                   |
| Close pairs             | Writing the closing bracket, quote, or inline markup at once when the user types the opening one.                                |                   |
| Current line            | The line of the markup the user cursor is in, highlighted in the source view.                                                     |                   |
| Editor zoom             | The size of the text of the notes in both editing views, relative to its normal size; it does not change the rest of the window. | Font scale        |
| Focus mode              | A state of the window in which only the tabs and the note are shown, without the note overview, the tool panel, and the toolbars. | Zen mode          |
| Soft wrap               | Showing a line that is longer than the editor is wide on several lines on screen, without changing the note.                    | Word wrap         |
| Template                | A note in the template folder of a workspace that a new note can start from, with placeholders that are filled in when it is used. |                   |
| Template folder         | The folder of a workspace that holds its templates, `templates` unless the workspace settings name another.                      |                   |
| Placeholder             | A word in double curly braces in a template, such as `{{title}}` or `{{date}}`, that is replaced when a note is created from it. | Variable          |
| Daily note              | The note of one day, titled with its date, kept in the daily note folder of the workspace.                                        |                   |
| Daily note folder       | The folder of a workspace that holds its daily notes, `daily` unless the workspace settings name another.                        |                   |
| Split view              | The middle of the window divided into two panes side by side, each with tabs of its own.                                         | Split screen      |
| Pane                    | One of the two sides of the split view, with its own tabs and its own active tab; without the split view there is one pane.      | Editor group      |
| Formula                 | Mathematics written in a note in the notation GitHub reads, inline in a line or as a block of its own, shown typeset.            | Equation          |
| Spell check             | Marking the words of a note that the dictionary of the chosen language does not know, with suggestions, without a network connection. |                   |
| User dictionary         | The words the user has added to the spell check, which are never marked in any workspace.                                        |                   |
| Snapshot                | An earlier version of a note, kept with when and why it was taken, on this computer only, that the user can compare and restore. | Revision          |
