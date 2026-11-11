//! What the window shows of the agents (feature F14, UC-016 to UC-019): the
//! agents entry of the status bar with its menu (M-04.4, M-17.1), the
//! section "Agents" of the settings window (M-17.2 to M-17.5), the agent
//! cursor (M-17.6) and the "Open" buttons after an agent changed notes that
//! are not open (M-04.9).

use serde::{Deserialize, Serialize};
use underline_core::AgentAccess;
use underline_mcp::install::Tool;

/// One level of agent access (UC-017 BR-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum AgentLevelName {
    Off,
    ReadOnly,
    ReadWrite,
}

impl From<AgentLevelName> for AgentAccess {
    fn from(level: AgentLevelName) -> Self {
        match level {
            AgentLevelName::Off => AgentAccess::Off,
            AgentLevelName::ReadOnly => AgentAccess::ReadOnly,
            AgentLevelName::ReadWrite => AgentAccess::ReadWrite,
        }
    }
}

impl From<AgentAccess> for AgentLevelName {
    fn from(access: AgentAccess) -> Self {
        match access {
            AgentAccess::Off => AgentLevelName::Off,
            AgentAccess::ReadOnly => AgentLevelName::ReadOnly,
            AgentAccess::ReadWrite => AgentLevelName::ReadWrite,
        }
    }
}

/// An agent tool MCP install knows (UC-016, FR-060).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum AgentToolName {
    ClaudeCode,
    ClaudeDesktop,
    Cursor,
    VsCode,
    Codex,
    GeminiCli,
}

impl From<AgentToolName> for Tool {
    fn from(tool: AgentToolName) -> Self {
        match tool {
            AgentToolName::ClaudeCode => Tool::ClaudeCode,
            AgentToolName::ClaudeDesktop => Tool::ClaudeDesktop,
            AgentToolName::Cursor => Tool::Cursor,
            AgentToolName::VsCode => Tool::VsCode,
            AgentToolName::Codex => Tool::Codex,
            AgentToolName::GeminiCli => Tool::GeminiCli,
        }
    }
}

impl From<Tool> for AgentToolName {
    fn from(tool: Tool) -> Self {
        match tool {
            Tool::ClaudeCode => AgentToolName::ClaudeCode,
            Tool::ClaudeDesktop => AgentToolName::ClaudeDesktop,
            Tool::Cursor => AgentToolName::Cursor,
            Tool::VsCode => AgentToolName::VsCode,
            Tool::Codex => AgentToolName::Codex,
            Tool::GeminiCli => AgentToolName::GeminiCli,
        }
    }
}

/// M-04.4, M-17.1 (UC-017 step 1, A1, A2, A4): the agents entry of the
/// status bar and its menu, while a workspace is open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AgentsMenuView {
    /// "Agents: read only in this workspace · Agent connected: Claude".
    pub text: String,
    /// UC-017 A2: "Claude asked for access" while access is off; egui
    /// appends it to `text` with " · ", the window shows it first, marked,
    /// so a cut text keeps it (A-168).
    pub asked: Option<String>,
    /// "What agents may do with the notes through Underline's MCP server".
    pub hover: String,
    /// The levels up to what the workspace allows, the user's chosen.
    pub levels: Vec<AgentLevelView>,
    /// "Settings of this workspace…".
    pub workspace_settings: String,
}

/// One level in the agents menu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AgentLevelView {
    pub level: AgentLevelName,
    /// "Off", "Read only", "Read and write".
    pub label: String,
    /// The level of the user settings (BR-002).
    pub chosen: bool,
}

/// What the window shows of the agents, sent as the event `agents` when it
/// changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AgentsView {
    /// The agents entry of the status bar; `null` without a workspace.
    pub menu: Option<AgentsMenuView>,
    /// M-17.2: the lines above the tools in the section "Agents" of the
    /// settings: "Open a workspace to connect agents to it." alone, or
    /// what the section does and the agent access in effect.
    pub head: Vec<String>,
    /// A workspace is open: the section shows the tools.
    pub workspace: bool,
}

/// M-17.6 (FR-057, UC-018 BR-006, UC-019 BR-006): the agent cursor of one
/// agent in a note: the lines it read or wrote last.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AgentMarkView {
    /// The first line, counted from 0.
    pub from: u32,
    /// The line after the last, counted from 0; equal to `from` marks the
    /// line `from` alone.
    pub to: u32,
    /// The agent's name.
    pub name: String,
    /// The agent wrote these lines, rather than read them.
    pub wrote: bool,
}

/// The agent cursors in one open note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AgentMarksView {
    pub note: String,
    /// Sorted by their first line.
    pub marks: Vec<AgentMarkView>,
}

/// M-04.9 (UC-019 BR-010): a button "Open" or "Open {title}" of the status
/// bar, for a note an agent created or changed while it was not open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AgentOpenView {
    pub note: String,
    /// "Open" for one note, "Open {title}" for several.
    pub label: String,
    /// The agent created the note, rather than changed it.
    pub created: bool,
}

/// What a button of the section "Agents" does (UC-016 steps 3–5, A1–A4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AgentAction {
    /// "Install"; with `replace` ("Replace", A2) an entry for another place
    /// is replaced.
    Install { tool: AgentToolName, replace: bool },
    /// A1: "Install only", although agent access stays off.
    InstallOnly { tool: AgentToolName },
    /// A1: "Allow reading and install", "Allow reading and writing and
    /// install".
    AllowAndInstall {
        tool: AgentToolName,
        level: AgentLevelName,
    },
    /// A3: "Remove".
    Remove { tool: AgentToolName },
    /// A4: "Copy configuration".
    Copy,
    /// "Cancel" of a question.
    Cancel,
}

/// A button of the section "Agents".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AgentButtonView {
    pub label: String,
    /// Not found tools show "Install" disabled.
    pub enabled: bool,
    pub action: AgentAction,
}

/// M-17.2 (UC-016 BR-006): one tool of the section "Agents" and how the
/// workspace stands in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AgentToolRowView {
    pub tool: AgentToolName,
    /// "Claude Code", "VS Code (Copilot)" …
    pub name: String,
    /// "not found", "not installed", "installed as {name}", "{name}:
    /// installed for another place", "cannot be read".
    pub state: String,
    /// "not found" is weak.
    pub weak: bool,
    /// In the warning colour: installed for another place, cannot be read.
    pub warn: bool,
    /// Why the configuration cannot be read, at the pointer.
    pub hover: Option<String>,
    /// "Install" or "Remove"; none when the configuration cannot be read.
    pub button: Option<AgentButtonView>,
}

/// M-17.3: what the section asks before installing (UC-016 A1, A2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AgentQuestionView {
    /// A1: "Agents reach the notes only while agent access is on. …"; A2
    /// asks in the report instead.
    pub text: Option<String>,
    pub buttons: Vec<AgentButtonView>,
}

/// M-17.2 to M-17.5: the section "Agents" of the settings window below its
/// head (`AgentsView::head`): the tools, a question, the report of the last
/// action and "Copy configuration".
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AgentsSectionView {
    /// Empty without a workspace.
    pub tools: Vec<AgentToolRowView>,
    pub question: Option<AgentQuestionView>,
    /// M-17.4, M-17.5: "Installed …", "Removed …", "{tool} was not
    /// changed: …", "Copied the configuration of …".
    pub report: Option<String>,
    /// "Copy configuration" with its hover "For any other agent tool";
    /// `null` without a workspace.
    pub copy: Option<AgentButtonView>,
    /// "For any other agent tool".
    pub copy_hover: String,
}
