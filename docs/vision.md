# Vision: Underline

> Draft written from the current README and code. Review and correct it before running `/requirements`:
> the requirements catalog is only as good as this document.

## Mission

Notes that matter to a project end up scattered: in a local notes app, in the project wiki, and in chats with an AI
assistant that forgets them. Underline is a fast, native note-taking desktop app that keeps all of them in one place:
a plain directory of Markdown pages in exactly the layout of a GitHub or GitLab wiki. The same files are edited by
the user in the app, synced with the project wiki through git, and read and written by an AI agent through the
Model Context Protocol (MCP). Nothing is locked into a proprietary format, and the user and the agent always work on
the same notes.

## Target users

- **Developer or technical writer** — keeps project notes, documentation, and knowledge next to the code and wants
  them in the project's GitHub or GitLab wiki without leaving a desktop editor or touching git by hand.
- **Team member using an AI agent** — works with an MCP-capable agent (Claude Code, Claude Desktop, other MCP
  clients) and wants the agent to read, create, and update notes as a shared, persistent memory.
- **Wiki maintainer** — owns a GitHub or GitLab wiki and wants to edit it offline in a comfortable editor and publish
  the changes later.

## Goals

- A user can create, find, read, and edit Markdown pages in a native window on Linux, Windows, and macOS.
- A notebook can be connected to a GitHub or GitLab project wiki by its project URL, and cloned, pulled, and pushed
  from inside the app, without the user running git commands.
- An AI agent connected through MCP can list, read, create, and update the pages of a notebook, and the user sees the
  agent's changes in the app.
- Notes stay plain Markdown files that any other editor, git, or the wiki web UI can open and change.
- The app starts in under one second and stays responsive with notebooks of at least 1,000 pages.

## Scope

### In scope

- Notebook as a local directory of `<Page>.md` files, compatible with GitHub and GitLab wikis (page names, links,
  `Home` page, sidebar and footer pages).
- Desktop app: page list, Markdown editor, preview, create, rename, and delete pages, search across pages.
- Wiki sync with GitHub (github.com) and GitLab (gitlab.com and self-hosted instances): clone, pull, push, and handling
  of conflicting edits.
- MCP server (`underline mcp <notebook>`) over stdio with tools to list, read, write, and search notes.
- Several notebooks, each with at most one wiki remote.

### Out of scope

- A hosted cloud service or user accounts of its own; sync goes through the wiki's git repository only.
- Real-time collaborative editing between several people.
- Rich-text or WYSIWYG formats other than Markdown, and proprietary note formats.
- Mobile apps and a web version.
- An AI model built into the app; Underline works with the agent the user already has, through MCP.
- Wiki providers other than GitHub and GitLab (for example Gitea or Azure DevOps) in the first release.

## Constraints

- Written in Rust; the desktop UI uses egui/eframe.
- Runs on Linux (X11 and Wayland), Windows, and macOS; CI builds and tests on all three.
- The on-disk format is the GitHub/GitLab wiki repository layout; Underline must not add files to a notebook that
  would show up as pages in the wiki.
- The MCP server speaks the MCP stdio transport (JSON-RPC 2.0, one message per line).
- Authentication against GitHub and GitLab uses the user's existing git credentials; Underline does not store
  passwords itself.
- Open source under the MIT license.

## Success measures

- A user can go from installing Underline to editing an existing GitHub or GitLab wiki in under five minutes.
- Pages edited in Underline and pushed to the wiki render correctly in the wiki web UI, with working links.
- An MCP client configured with `underline mcp <notebook>` can complete a read–write–read round trip on the first try.
- No data loss: a sync never silently overwrites a local or a remote change.
