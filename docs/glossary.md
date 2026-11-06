# Glossary

| Term                    | Definition                                                                                                                         | Avoid             |
|-------------------------|------------------------------------------------------------------------------------------------------------------------------------|-------------------|
| User                    | The person who works with Underline on their notes.                                                                                |                   |
| Agent                   | An AI tool, such as Claude Code or Cursor, that reads and writes notes through the MCP server on the user's behalf.               | Bot, Bearbeiter   |
| Workspace               | A local directory whose Markdown and AsciiDoc files Underline shows as notes; it may be a git repository.                         | Notebook, Vault   |
| Note                    | One Markdown or AsciiDoc file in a workspace, shown and edited in a tab.                                                           | Notiz             |
| Note format             | The markup a note is stored in, given by its file extension: Markdown (`.md`) or AsciiDoc (`.adoc`).                               |                   |
| Default format          | The note format a workspace gives to new notes, chosen when the workspace is created; Markdown unless the user picks another.     |                   |
| Facade                  | The editing view that shows a note rendered and lets the user edit it the same way whatever its note format is.                  | WYSIWYG, Fassade  |
| Facade engine           | The part of Underline that stores every edit in the note format of the note, whichever way the user or the agent made it. |                   |
| Format indicator        | The mark in the facade that names the note format of the active note.                                                             |                   |
| Source view             | The editing view that shows and edits the markup of a note as stored in its file.                                                 | Raw mode          |
| Editing view            | Either the source view or the facade, chosen per tab.                                                                              |                   |
| Input mode              | The set of keys the editor follows: Normal, Vim, or Nano. Independent of the editing view.                                        | Edit mode         |
| Tab                     | The area in the middle of the window that shows one open note.                                                                    |                   |
| Note overview           | The panel on the left that lists the notes of the workspace, the recently opened first.                                          | Sidebar           |
| Tool panel              | The panel on the right with the note properties and the tools: diff, outline, code index, figure index, todos.                    |                   |
| Status bar              | The line at the bottom of the window with the workspace directory, the git sync state, the input mode commands, and messages.    |                   |
| Outline                 | The list of the headings of the active note, used to jump to a section.                                                            | Table of contents |
| Code index              | The list of the code blocks of the active note, used to jump to one.                                                               |                   |
| Figure index            | The list of the images and diagrams of the active note, used to jump to one.                                                       |                   |
| Minimap                 | The scaled-down picture of the whole active note beside the editor, showing the visible part.                                     |                   |
| Theme                   | The set of colors of the interface, which rendered diagrams and highlighted code follow.                                          | Skin              |
| Diagram                 | A PlantUML or Mermaid code block of a note, shown as a picture.                                                                    |                   |
| Local link              | A link in a note that points to another note or file of the same workspace.                                                        |                   |
| Autosave                | Writing a changed note to the local disk without a command from the user.                                                          |                   |
| Git sync                | Pulling from and pushing to the remote of a workspace that is a git repository, at a regular interval.                            |                   |
| MCP server              | The part of Underline that offers the notes of a workspace to an agent over the Model Context Protocol.                           |                   |
| MCP install             | The action that registers the MCP server in the configuration of an agent's tool.                                                 |                   |
| User cursor             | The text cursor of the user in an open note.                                                                                       |                   |
| Agent cursor            | The second cursor in an open note that marks where the agent is reading or writing.                                              | AI cursor         |
| Copy on select          | Copying selected text, an image, or another element to the clipboard as soon as it is selected.                                   |                   |
| Todo                    | A piece of work the user is heading towards, with a title, a due date, an urgency, a scheduled date, and links to notes.          | Task              |
| Due date                | The date by which a todo has to be done.                                                                                           | Deadline, Frist   |
| Urgency                 | How pressing a todo is: low, normal, high, or critical.                                                                            |                   |
| Scheduled date          | The day the user plans to work on a todo.                                                                                          |                   |
| User settings file      | The `underline.yaml` in the user's home directory with the user's default settings.                                               |                   |
| Workspace settings file | The `underline.yaml` in the root directory of a workspace; its settings override the user settings file for that workspace.      |                   |
| Export                  | Writing a copy of a note in another format (Markdown, AsciiDoc, HTML, or PDF) without changing the note.                          |                   |
