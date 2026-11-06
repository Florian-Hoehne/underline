# Vision: Underline

## Mission

Notes that matter to a project end up scattered: in a local notes app, in the project's git repository or wiki, and
in chats with an AI assistant that forgets them. Underline is a native note-taking desktop app with a plain but
modern interface that keeps them in one place: a workspace directory of plain Markdown or AsciiDoc files on the local
disk. The user edits a note either directly in its markup or in the facade, a rendered editing view that looks and
works the same whatever format the file is stored in. An AI agent reads and writes the same notes through the Model
Context Protocol (MCP), and the user watches the agent's cursor next to their own. When the workspace is a git
repository, Underline shows and drives the usual git workflow. Nothing is locked into a proprietary format.

## Target users

- **User** — a developer, technical writer, or knowledge worker who keeps notes, documentation, and tasks next to
  their projects and wants a fast, keyboard-friendly editor (normal, Vim, or Nano input) with diagrams, code, and
  images rendered in place.
- **AI agent** — an MCP-capable assistant (Claude Code, Claude Desktop, Cursor, VS Code Copilot, Codex, Gemini CLI,
  or another MCP client) that the user connects to a workspace to read, create, and update notes as a shared,
  persistent memory.

## Goals

- A user edits notes in Markdown or AsciiDoc, in the markup or in the facade, without having to know which format a
  note is stored in; the facade always shows it and offers the options that only that format supports.
- The facade engine coordinates every write, so further Markdown flavors and markup formats can be added later
  without changing the editor.
- A note can be exported to Markdown, AsciiDoc, HTML, and PDF.
- No edit is lost: notes are saved to the local disk automatically, and a workspace that is a git repository with a
  remote is synchronized with it at a regular interval.
- An AI agent connected through MCP can read and write notes, its cursor is visible while it is connected, and the
  user can switch the MCP server on and off and install it into their AI tool with one action.
- Tasks are managed next to the notes: a todo has a due date, an urgency, and a date, and points to the notes it is
  about.
- Every setting lives in `underline.yaml`, with user defaults in the home directory that a workspace can override,
  and every change made in the interface is remembered immediately.

## Scope

### In scope

- Workspaces: local directories of notes; when a workspace is created the user chooses its default note format
  (Markdown preselected); a note's own format follows its file extension.
- Interface: note overview on the left (recent notes), notes in tabs in the middle, properties and tools on the right
  (git diff, outline, code index, figure index), and a status bar at the bottom.
- Editing: source editing and facade editing, a formatting toolbar, input modes normal, Vim, and Nano, folding of
  sections, line numbers that can be switched off and placed left or right, a minimap of the note, copy on select
  (can be switched off), and drag and drop of images, files, and links.
- Rendering: syntax-highlighted code blocks, PlantUML and Mermaid diagrams in the colors of the active theme,
  diagrams and images that can be enlarged, local links that open the linked note.
- Color themes: Light, Dark, GitHub Light, GitHub Dark, Solarized Light, Solarized Dark, Night Owl, Ubuntu, Nord,
  Dracula, and Gruvbox.
- Saving and sync: autosave to the local disk; git pull and push with the workspace's remote at a regular interval.
- Git: changes, diff, and current branch for a workspace that is a git repository; create and switch branches,
  commit, push, and pull.
- MCP: a server that lets an AI agent read and write notes, switchable on and off, with an installer for common AI
  tools and a configuration snippet for any other.
- Todo management linked to notes.
- Export to Markdown, AsciiDoc, HTML, and PDF.

### Out of scope

- A hosted cloud service or user accounts of its own; sync goes through the workspace's git remote only.
- Real-time collaborative editing between several people.
- Proprietary note formats and storage other than plain files.
- Mobile apps and a web version.
- An AI model built into the app; Underline works with the agent the user already has, through MCP.
- Markdown flavors and markup formats other than Markdown as GitHub shows it and AsciiDoc in the first release; the
  facade engine is prepared for them.

## Constraints

- Underline is a desktop app for Linux, Windows, and macOS.
- Notes are stored as plain `.md` and `.adoc` files that other editors, git, and the GitHub and GitLab web interfaces
  can open and display the same way.
- Settings are kept in `underline.yaml`: the defaults in the user's home directory, overridable by an
  `underline.yaml` in the workspace.
- Underline uses the access to git remotes the user has already set up and keeps no passwords of its own.
- Open source under the MIT license.

## Success measures

- A user can go from installing Underline to editing an existing repository of notes in under five minutes.
- Opening a note in the facade and leaving it without an edit leaves it unchanged.
- An AI tool set up by the MCP installer completes a read–write–read round trip on a note on the first try.
- No data loss: a crash loses at most the last two seconds of typing, and a sync never silently overwrites a local or
  a remote change.
