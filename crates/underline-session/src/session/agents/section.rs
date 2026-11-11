//! M-17.2 to M-17.5 (UC-016, ADR-017): the section "Agents" of the
//! settings window. Ported from `agents_ui`, `check_agents` and
//! `agent_action` of `apps/underline-desktop/src/app.rs`.
//!
//! The tools' configuration files are read and written without the session
//! ([`SectionJob::run`]), in the places the caller gives (the home folder of
//! this computer in the app, a scratch folder in tests). The states are
//! checked when the window asks for the section (the settings open, a
//! workspace opened) and after every action (BR-006).

use std::path::PathBuf;

use underline_core::{AgentAccess, SettingKey};
use underline_mcp::install::{self, InstallError, InstallState, Places, Server, Tool};

use super::super::Session;
use crate::agents_view::{
    AgentAction, AgentButtonView, AgentQuestionView, AgentToolName, AgentToolRowView,
    AgentsSectionView,
};
use crate::Scope;

/// A1, A2: what the section asks before installing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Question {
    SwitchOn(Tool),
    Replace(Tool),
}

/// What the section shows besides the tools' states.
#[derive(Debug, Default)]
pub(in crate::session) struct Section {
    states: Vec<(Tool, Result<InstallState, String>)>,
    report: Option<String>,
    asking: Option<Question>,
}

impl Section {
    /// Another workspace: nothing of the section is kept.
    pub(in crate::session) fn forget(&mut self) {
        *self = Self::default();
    }
}

/// What to do in the tools' configurations, then check them all.
#[derive(Debug)]
enum Work {
    Check,
    Install { tool: Tool, replace: bool },
    Remove(Tool),
}

/// The files of the section, read and written without the session.
#[derive(Debug)]
pub struct SectionJob {
    /// `None` without a workspace: nothing to check.
    server: Option<Server>,
    work: Work,
    /// A4: the configuration to copy.
    copy: Option<String>,
}

/// What [`SectionJob::run`] did.
#[derive(Debug)]
pub struct SectionDone {
    server: Option<Server>,
    states: Vec<(Tool, Result<InstallState, String>)>,
    outcome: Option<(Tool, Work, Result<Option<String>, InstallError>)>,
}

impl SectionJob {
    /// A4: the configuration for any other tool, to put on the clipboard;
    /// the report saying so shows once [`Session::agents_section_done`]
    /// took the job back.
    #[must_use]
    pub fn copied(&self) -> Option<&str> {
        self.copy.as_deref()
    }

    /// Installs or removes as asked, then checks how the workspace stands
    /// in each tool, with the configuration files in `places`.
    #[must_use]
    pub fn run(self, places: &Places) -> SectionDone {
        let Some(server) = self.server else {
            return SectionDone {
                server: None,
                states: Vec::new(),
                outcome: None,
            };
        };
        let outcome = match self.work {
            Work::Check => None,
            Work::Install { tool, replace } => Some((
                tool,
                Work::Install { tool, replace },
                install::install(tool, places, &server, replace).map(Some),
            )),
            Work::Remove(tool) => Some((
                tool,
                Work::Remove(tool),
                install::remove(tool, places, &server).map(|()| None),
            )),
        };
        let states = Tool::ALL
            .iter()
            .map(|tool| {
                (
                    *tool,
                    install::state(*tool, places, &server).map_err(|e| e.to_string()),
                )
            })
            .collect();
        SectionDone {
            server: Some(server),
            states,
            outcome,
        }
    }
}

/// The folder name of the workspace, as the reports name it.
fn workspace_name(server: &Server) -> String {
    server
        .workspace
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn button(label: &str, action: AgentAction) -> AgentButtonView {
    AgentButtonView {
        label: label.to_owned(),
        enabled: true,
        action,
    }
}

impl Session {
    /// The MCP server entry of the open workspace: this Underline serving
    /// it (UC-016 BR-002).
    fn mcp_server(&self) -> Option<Server> {
        Some(Server {
            program: self.agents.section_program.clone()?,
            workspace: self.workspace_root()?.to_owned(),
        })
    }

    /// UC-016 steps 3–5, A1–A4, A6: starts `action` in the section
    /// "Agents", or only checks the tools (`None`). A question asked first
    /// and a cancel need no file; the job then only checks.
    pub fn agents_section_start(&mut self, action: Option<AgentAction>) -> SectionJob {
        let server = self.mcp_server();
        let mut job = SectionJob {
            server: server.clone(),
            work: Work::Check,
            copy: None,
        };
        let (Some(action), Some(server)) = (action, server) else {
            return job;
        };
        let section = &mut self.agents.section;
        section.asking = None;
        match action {
            // A1, A2: the question goes, with the report that asked it
            // (A2; egui left its text standing without buttons, A-167).
            AgentAction::Cancel => section.report = None,
            AgentAction::Copy => {
                section.report = Some(format!(
                    "Copied the configuration of {}. Its command: {}",
                    workspace_name(&server),
                    server.command_line()
                ));
                job.copy = Some(server.snippet());
            }
            AgentAction::Remove { tool } => job.work = Work::Remove(tool.into()),
            AgentAction::AllowAndInstall { tool, level } => {
                // A1: the user chose what agents may do (UC-017).
                let level = AgentAccess::from(level);
                self.settings.change(
                    Scope::Me,
                    SettingKey::AgentAccess,
                    Some(level.label().into()),
                );
                self.take_settings_messages();
                job.work = Work::Install {
                    tool: tool.into(),
                    replace: false,
                };
            }
            AgentAction::Install {
                tool,
                replace: false,
            } if self.settings.user_agent_access() == AgentAccess::Off => {
                // A1: asked first; the tools stay as checked.
                section.asking = Some(Question::SwitchOn(tool.into()));
                section.report = None;
                job.server = None;
            }
            AgentAction::Install { tool, replace } => {
                job.work = Work::Install {
                    tool: tool.into(),
                    replace,
                };
            }
            AgentAction::InstallOnly { tool } => {
                job.work = Work::Install {
                    tool: tool.into(),
                    replace: false,
                };
            }
        }
        job
    }

    /// Takes in what [`SectionJob::run`] did and checked; what the section
    /// shows now.
    pub fn agents_section_done(&mut self, done: SectionDone) -> AgentsSectionView {
        let same = done.server.is_some() && done.server == self.mcp_server();
        if same {
            if let Some((tool, work, outcome)) = done.outcome {
                let report = self.install_report(tool, &work, outcome, done.server.as_ref());
                self.agents.section.report = Some(report);
            }
            self.agents.section.states = done.states;
        }
        self.agents_section()
    }

    /// M-17.4: what an install or removal reports (UC-016 step 5, A2, A3,
    /// A6).
    fn install_report(
        &mut self,
        tool: Tool,
        work: &Work,
        outcome: Result<Option<String>, InstallError>,
        server: Option<&Server>,
    ) -> String {
        let workspace = server.map(workspace_name).unwrap_or_default();
        match (work, outcome) {
            (Work::Remove(_), Ok(_)) => format!("Removed {workspace} from {}.", tool.name()),
            (Work::Install { .. }, Ok(name)) => {
                let name = name.unwrap_or_default();
                let mut report = format!(
                    "Installed {workspace} in {} as {name}; restart {} if it does not show it yet.",
                    tool.name(),
                    tool.name()
                );
                if !self.settings.effective().agent_access.reads() {
                    report
                        .push_str(" Agent access is off, so the agent cannot reach the notes yet.");
                }
                report
            }
            // A2: the entry starts another Underline.
            (Work::Install { .. }, Err(InstallError::ForAnotherPlace(_))) => {
                self.agents.section.asking = Some(Question::Replace(tool));
                "An entry for this workspace starts another Underline. Replace it?".into()
            }
            (_, Err(err)) => format!("{} was not changed: {err}", tool.name()),
            (Work::Check, Ok(_)) => String::new(),
        }
    }

    /// M-17.2 to M-17.5: the section "Agents" below its head, as checked
    /// last.
    #[must_use]
    pub fn agents_section(&self) -> AgentsSectionView {
        let section = &self.agents.section;
        let workspace = self.workspace_root().is_some();
        let tools = if workspace {
            section.states.iter().map(tool_row).collect()
        } else {
            Vec::new()
        };
        let question = section.asking.map(|asking| match asking {
            Question::SwitchOn(tool) => {
                let tool = AgentToolName::from(tool);
                AgentQuestionView {
                    text: Some(
                        "Agents reach the notes only while agent access is on. Allow agents to read, or to read and write, for you?"
                            .into(),
                    ),
                    buttons: vec![
                        button(
                            "Allow reading and install",
                            AgentAction::AllowAndInstall {
                                tool,
                                level: crate::AgentLevelName::ReadOnly,
                            },
                        ),
                        button(
                            "Allow reading and writing and install",
                            AgentAction::AllowAndInstall {
                                tool,
                                level: crate::AgentLevelName::ReadWrite,
                            },
                        ),
                        button("Install only", AgentAction::InstallOnly { tool }),
                        button("Cancel", AgentAction::Cancel),
                    ],
                }
            }
            // A2: egui shows the question in the report below its buttons;
            // here it stands above them, as A1's (A-167).
            Question::Replace(tool) => AgentQuestionView {
                text: section.report.clone(),
                buttons: vec![
                    button(
                        "Replace",
                        AgentAction::Install {
                            tool: tool.into(),
                            replace: true,
                        },
                    ),
                    button("Cancel", AgentAction::Cancel),
                ],
            },
        });
        AgentsSectionView {
            tools,
            question: if workspace { question } else { None },
            report: if workspace && !matches!(section.asking, Some(Question::Replace(_))) {
                section.report.clone()
            } else {
                None
            },
            copy: workspace.then(|| button("Copy configuration", AgentAction::Copy)),
            copy_hover: "For any other agent tool".into(),
        }
    }
}

/// M-17.2: one tool's row: its name, its state, "Install" or "Remove".
fn tool_row((tool, state): &(Tool, Result<InstallState, String>)) -> AgentToolRowView {
    let name = AgentToolName::from(*tool);
    let install = |enabled: bool| AgentButtonView {
        label: "Install".into(),
        enabled,
        action: AgentAction::Install {
            tool: name,
            replace: false,
        },
    };
    let row = |state: String, weak: bool, warn: bool| AgentToolRowView {
        tool: name,
        name: tool.name().to_owned(),
        state,
        weak,
        warn,
        hover: None,
        button: None,
    };
    match state {
        Ok(InstallState::NotFound) => AgentToolRowView {
            button: Some(install(false)),
            ..row("not found".into(), true, false)
        },
        Ok(InstallState::NotInstalled) => AgentToolRowView {
            button: Some(install(true)),
            ..row("not installed".into(), false, false)
        },
        Ok(InstallState::Installed(entry)) => AgentToolRowView {
            button: Some(button("Remove", AgentAction::Remove { tool: name })),
            ..row(format!("installed as {entry}"), false, false)
        },
        Ok(InstallState::ForAnotherPlace(entry)) => AgentToolRowView {
            button: Some(install(true)),
            ..row(format!("{entry}: installed for another place"), false, true)
        },
        Err(err) => AgentToolRowView {
            hover: Some(err.clone()),
            ..row("cannot be read".into(), false, true)
        },
    }
}

/// The program MCP install writes into the tools' configurations: this
/// Underline (UC-016 BR-002), `current_exe` as in egui; an AppImage runs
/// from a mount that goes when it ends, so there the AppImage file itself
/// (`$APPIMAGE`, A-174).
pub(in crate::session) fn this_program() -> Option<PathBuf> {
    std::env::var_os("APPIMAGE")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::current_exe().ok())
}
