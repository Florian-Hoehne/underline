//! Feature F14 (UC-017 to UC-019, ADR-018, ADR-025): the agents connected to
//! the open workspace, their changes handed to the window, the agent cursor
//! and the agents entry of the status bar.
//!
//! Ported from `poll_agents`, `take_agent_requests`, `agent_request`,
//! `report_agent_change`, `show_agent_cursors` and `agents_status` of
//! `apps/underline-desktop/src/app.rs`. The files are read and written
//! without the session: [`Session::agents_job`] says what to do,
//! [`AgentsJob::run`] does it on the tick's thread, and
//! [`Session::agents_done`] takes in what it found.
//!
//! - About once a second the window renews its presence file and looks
//!   which agents are connected (UC-017 BR-004); the requests of the MCP
//!   servers are taken every 100 ms (ADR-018, NFR-008).
//! - A change of a note that is not open is made in its file
//!   (`apply_to_file`), and the status says "{agent} created A and changed
//!   B" with "Open" buttons (UC-019 BR-010, M-04.9).
//! - A change of an open note first asks the window for its edits
//!   (`ed-flush`, ADR-025 section 3.6) and waits for them at most
//!   [`FLUSH_AT_MOST`]; then it is planned on the mirror, made as an undo
//!   step of its own (`backend_edit`, sealed) and stored at once; the agent
//!   is answered once it is stored.

mod section;
#[cfg(test)]
mod tests;

pub use self::section::{SectionDone, SectionJob};

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use underline_core::{
    answer_for_tab, answer_request, apply_to_file, connected_agents, find_note, plan_for_tab,
    remove_window_presence, renew_window_presence, stored_agent_access, take_requests, AgentAccess,
    AgentAnswer, AgentChange, AgentRequest, NoteEntry, NoteId, Presence, SaveJob, SettingKey,
    Workspace, WriteProblem, PRESENCE_RENEW, READING_SHOWN, UNNAMED_AGENT,
};

use super::Session;
use crate::agents_view::{
    AgentLevelName, AgentLevelView, AgentMarkView, AgentMarksView, AgentOpenView, AgentsMenuView,
    AgentsView,
};
use crate::{Event, Scope};

/// ADR-018: how often the requests of the MCP servers are taken while an
/// agent is connected.
pub const REQUESTS_EVERY: Duration = Duration::from_millis(100);

/// ADR-025 section 3.6: how long an agent's change of an open note waits
/// for the window to send its edits.
pub const FLUSH_AT_MOST: Duration = Duration::from_millis(300);

/// egui: a reading of the same lines within this time keeps the agent
/// cursor's start, so a series of reads does not restart it.
const SAME_PLACE: Duration = Duration::from_millis(1500);

/// "Underline allows agents only to read the notes of this workspace".
const ONLY_READING: &str = "Underline allows agents only to read the notes of this workspace";

/// UC-019 BR-006: where an agent read or wrote last.
#[derive(Debug, Clone)]
struct AgentCursor {
    note: NoteId,
    /// Counted from 1, the last included.
    lines: (usize, usize),
    since: Instant,
    wrote: bool,
    /// The time of the reading it came from; 0 for a change made here.
    reading_at: u64,
}

/// UC-019 BR-010: an agent created or changed notes that are not open; the
/// report while the status shows it.
#[derive(Debug, Clone)]
struct AgentReport {
    status: String,
    agent: String,
    items: Vec<(&'static str, NoteId)>,
}

/// ADR-018: a request taken from the workspace at `root`.
#[derive(Debug, Clone)]
pub(super) struct Taken {
    root: PathBuf,
    id: String,
    agent: String,
    request: AgentRequest,
}

/// A request for an open note, waiting for the window's edits (ADR-025).
#[derive(Debug)]
struct Waiting {
    taken: Taken,
    note: NoteId,
    entry: NoteEntry,
    asked: Instant,
    /// The tab's number of syncs when the window was asked.
    syncs: u64,
}

/// The answer to give once the open note an agent changed is stored.
#[derive(Debug)]
pub(super) struct AnswerAfterSave {
    root: PathBuf,
    id: String,
    note: NoteId,
    answer: AgentAnswer,
}

impl AnswerAfterSave {
    /// The answer for the save's outcome: the change, or why the note
    /// could not be written (UC-019 A9; the tab keeps the change).
    pub(super) fn answer(self, stored: Result<(), &str>) -> (PathBuf, String, AgentAnswer) {
        let answer = match stored {
            Ok(()) => self.answer,
            Err(reason) => AgentAnswer::refused(&WriteProblem::Write {
                note: self.note.as_str().to_owned(),
                reason: reason.to_owned(),
            }),
        };
        (self.root, self.id, answer)
    }
}

/// The agents of the open workspace (UC-017 to UC-019).
#[derive(Debug, Default)]
pub(super) struct Agents {
    /// UC-017 BR-004: the agents connected, looked at about once a second.
    connected: Vec<Presence>,
    looked: Option<Instant>,
    /// ADR-018: when the requests were taken last.
    requests_taken: Option<Instant>,
    /// UC-018 BR-006, UC-019 BR-006: the agent cursor of each agent.
    marks: HashMap<String, AgentCursor>,
    /// UC-019 BR-010: the report of changes to notes not open.
    report: Option<AgentReport>,
    /// UC-020 A8: the last todo change of each agent that was reported.
    todo_seen: HashMap<String, u64>,
    /// Requests taken, to be found among the notes without the session.
    taken: Vec<Taken>,
    /// Requests for open notes waiting for the window's edits.
    waiting: Vec<Waiting>,
    /// Answers to write, with the workspace they go to.
    pub(super) answers: Vec<(PathBuf, String, AgentAnswer)>,
    /// UC-019: changes of open notes being stored, by the note's file; the
    /// agent is answered when the save ends, whether its tab is still
    /// there or not.
    after_save: Vec<(PathBuf, AnswerAfterSave)>,
    /// The window presence could not be written; said once in the log.
    presence_failed: bool,
    /// Inventory D-05 (fixed): workspaces left, whose window presence goes.
    left: Vec<PathBuf>,
    /// The section "Agents" of the settings window.
    pub(super) section: section::Section,
    /// The program MCP install writes into the tools' configurations.
    section_program: Option<PathBuf>,
    /// What the window was told last.
    told: Option<AgentsView>,
    told_marks: Vec<AgentMarksView>,
}

impl Agents {
    /// Nothing known yet; MCP install names this Underline (UC-016 BR-002).
    pub(super) fn new() -> Self {
        Self {
            section_program: section::this_program(),
            ..Self::default()
        }
    }

    /// Tests name another program than the test binary.
    #[cfg(test)]
    pub(super) fn set_program(&mut self, program: PathBuf) {
        self.section_program = Some(program);
    }
}

/// What the agents' files ask for, done without the session
/// ([`AgentsJob::run`]).
#[derive(Debug)]
pub struct AgentsJob {
    root: Option<PathBuf>,
    left: Vec<PathBuf>,
    answers: Vec<(PathBuf, String, AgentAnswer)>,
    /// Renew the window presence and look who is connected.
    look: bool,
    /// Take the requests.
    take: bool,
    resolve: Vec<Taken>,
    /// The open workspace, to find and change notes in; only with requests
    /// to resolve.
    workspace: Option<Workspace>,
    /// The files of the open tabs, with their notes.
    open: Vec<(PathBuf, NoteId)>,
    /// UC-017 BR-003: agents may write when the job was made; with the
    /// stored access read again right before a note's file is changed.
    writes: bool,
    /// The user settings file, for reading the stored access.
    user_settings: Option<PathBuf>,
}

/// What [`AgentsJob::run`] found and did.
#[derive(Debug)]
pub struct AgentsDone {
    root: Option<PathBuf>,
    looked: Option<(SystemTime, Vec<Presence>)>,
    taken: Vec<(String, AgentRequest)>,
    /// Notes not open an agent created or changed: the agent, the verb.
    applied: Vec<(String, &'static str, NoteId)>,
    /// Requests for open notes, found.
    for_tabs: Vec<(Taken, NoteEntry, NoteId)>,
    /// Requests whose note was opened meanwhile or closed; found again.
    again: Vec<Taken>,
    /// "An agent's change could not be answered: {err}".
    problems: Vec<String>,
    /// Why the window presence could not be written, if it could not.
    presence: Option<String>,
}

impl AgentsJob {
    /// Removes the presence of workspaces left, writes the answers, renews
    /// the window's presence and looks who is connected, finds the notes
    /// of the requests and changes those not open, and takes the new
    /// requests; at `now`.
    #[must_use]
    pub fn run(mut self, now: SystemTime) -> AgentsDone {
        for root in &self.left {
            remove_window_presence(root);
        }
        let mut problems = Vec::new();
        let mut write_answer = |root: &Path, id: &str, answer: &AgentAnswer| {
            if let Err(err) = answer_request(root, id, answer) {
                problems.push(format!("An agent's change could not be answered: {err}"));
            }
        };
        for (root, id, answer) in &self.answers {
            write_answer(root, id, answer);
        }
        let mut done = AgentsDone {
            root: self.root.clone(),
            looked: None,
            taken: Vec::new(),
            applied: Vec::new(),
            for_tabs: Vec::new(),
            again: Vec::new(),
            problems: Vec::new(),
            presence: None,
        };
        let Some(root) = self.root.clone() else {
            done.problems = problems;
            return done;
        };
        if self.look {
            // ADR-018: the servers hand their changes to this window.
            done.presence = renew_window_presence(&root).err().map(|e| e.to_string());
            done.looked = Some((now, connected_agents(&root, now)));
        }
        // UC-017 BR-003: as stored now, as the MCP server reads it; a file
        // that cannot be read refuses (NFR-015).
        let writes = self.resolve.is_empty()
            || (self.writes
                && stored_agent_access(&root, self.user_settings.as_deref())
                    .is_ok_and(AgentAccess::writes));
        if let Some(workspace) = self.workspace.as_mut() {
            for taken in std::mem::take(&mut self.resolve) {
                if taken.root != root {
                    write_answer(&taken.root, &taken.id, &busy());
                    continue;
                }
                let note = &taken.request.note;
                if !writes {
                    // UC-017 BR-003: switched off since it was taken.
                    write_answer(&root, &taken.id, &only_reading(note));
                    continue;
                }
                let entry = match &taken.request.change {
                    AgentChange::Create { .. } => None,
                    _ => match find_note(&workspace.notes_now().notes, note) {
                        Ok(entry) => Some(entry),
                        Err(err) => {
                            let problem = WriteProblem::from(err);
                            write_answer(&root, &taken.id, &AgentAnswer::refused(&problem));
                            continue;
                        }
                    },
                };
                if let Some(entry) = entry {
                    let path = workspace.file_of(&entry);
                    if let Some((_, tab)) = self.open.iter().find(|(p, _)| *p == path) {
                        let tab = tab.clone();
                        done.for_tabs.push((taken, entry, tab));
                        continue;
                    }
                }
                let (answer, changed) = apply_to_file(workspace, note, &taken.request.change);
                write_answer(&root, &taken.id, &answer);
                if let Some(changed) = changed {
                    let verb = if matches!(taken.request.change, AgentChange::Create { .. }) {
                        "created"
                    } else {
                        "changed"
                    };
                    done.applied.push((taken.agent.clone(), verb, changed));
                }
            }
        }
        // Requests left without a workspace to find them in (none expected).
        done.again = std::mem::take(&mut self.resolve);
        if self.take {
            done.taken = take_requests(&root, now);
        }
        done.problems = problems;
        done
    }
}

/// UC-017 BR-002: the refusal while agents may only read.
fn only_reading(note: &str) -> AgentAnswer {
    AgentAnswer::refused(&WriteProblem::Write {
        note: note.to_owned(),
        reason: ONLY_READING.into(),
    })
}

/// ADR-018: "Underline is busy; try again".
fn busy() -> AgentAnswer {
    AgentAnswer::refused(&WriteProblem::Busy)
}

fn since(now: Instant, then: Instant) -> Duration {
    now.saturating_duration_since(then)
}

impl Session {
    /// What the agents' files ask for at `now`, or `None` when nothing is
    /// due: the presence and the connected agents about once a second
    /// (UC-017 BR-004), the requests every 100 ms (ADR-018, A-173), the notes of the requests taken, the answers to
    /// write and the presence of workspaces left.
    pub fn agents_job(&mut self, now: Instant) -> Option<AgentsJob> {
        let root = self.workspace_root().map(Path::to_owned);
        let agents = &mut self.agents;
        let look = root.is_some()
            && agents
                .looked
                .is_none_or(|t| since(now, t) >= PRESENCE_RENEW);
        // A-173: every 100 ms while a workspace is open, not only once an
        // agent was seen connected (egui), which is looked at once a second
        // and would hold a first request up to a second (NFR-008).
        let take = root.is_some()
            && agents
                .requests_taken
                .is_none_or(|t| since(now, t) >= REQUESTS_EVERY);
        if !look
            && !take
            && agents.left.is_empty()
            && agents.answers.is_empty()
            && agents.taken.is_empty()
        {
            return None;
        }
        if look {
            agents.looked = Some(now);
        }
        if take {
            agents.requests_taken = Some(now);
        }
        let resolve = std::mem::take(&mut agents.taken);
        let open = self.open.as_ref();
        let workspace = if resolve.is_empty() {
            None
        } else {
            open.map(|open| open.workspace.clone())
        };
        let files = open
            .map(|open| {
                open.tabs
                    .iter()
                    .map(|(note, tab)| (tab.path.clone(), note.clone()))
                    .collect()
            })
            .unwrap_or_default();
        let agents = &mut self.agents;
        Some(AgentsJob {
            root,
            left: std::mem::take(&mut agents.left),
            answers: std::mem::take(&mut agents.answers),
            look,
            take,
            resolve,
            workspace,
            open: files,
            writes: self.settings.effective().agent_access.writes(),
            user_settings: self.settings.user_file().map(Path::to_owned),
        })
    }

    /// Takes in what [`AgentsJob::run`] found at `now`: the connected
    /// agents with their cursors and todo changes, the report of the notes
    /// changed in their files, and the requests: refused without writing
    /// access, else found among the notes next, and those for an open note
    /// wait for the window's edits.
    pub fn agents_done(&mut self, done: AgentsDone, now: Instant) {
        if let Some(problem) = done.problems.last() {
            self.status.clone_from(problem);
        }
        // ADR-018: said once in the log, again only after it worked.
        match &done.presence {
            Some(err) if !self.agents.presence_failed => {
                self.agents.presence_failed = true;
                eprintln!("The window presence for agents could not be written: {err}");
            }
            Some(_) => {}
            None if done.looked.is_some() => self.agents.presence_failed = false,
            None => {}
        }
        let root = self.workspace_root().map(Path::to_owned);
        if done.root.is_none() || done.root != root {
            // The workspace changed meanwhile: its requests are answered
            // busy, so their servers ask again.
            if let Some(old) = &done.root {
                for (id, _) in done.taken {
                    self.agents.answers.push((old.clone(), id, busy()));
                }
            }
            for (taken, _, _) in done.for_tabs {
                self.agents.answers.push((taken.root, taken.id, busy()));
            }
            self.agents.taken.extend(done.again);
            return;
        }
        let Some(root) = root else {
            return;
        };
        if let Some((at, connected)) = done.looked {
            self.agents_seen(connected, at, now);
        }
        for (agent, verb, note) in done.applied {
            self.report_agent_change(&agent, verb, note);
        }
        for (taken, entry, note) in done.for_tabs {
            self.wait_for_window(taken, entry, note, now);
        }
        self.agents.taken.extend(done.again);
        let writes = self.settings.effective().agent_access.writes();
        for (id, request) in done.taken {
            let agent = request
                .agent
                .clone()
                .unwrap_or_else(|| UNNAMED_AGENT.to_owned());
            // UC-017 BR-002: the window follows the access in effect, too.
            if !writes {
                let refused = only_reading(&request.note);
                self.agents.answers.push((root.clone(), id, refused));
                continue;
            }
            self.agents.taken.push(Taken {
                root: root.clone(),
                id,
                agent,
                request,
            });
        }
        if !self.agents.taken.is_empty() || !self.agents.answers.is_empty() {
            self.wake.wake();
        }
    }

    /// UC-017 BR-004, UC-018 BR-006, UC-020 A8: the agents connected at
    /// `at`: their cursors where they read, and a todo one changed in the
    /// status bar.
    fn agents_seen(&mut self, connected: Vec<Presence>, at: SystemTime, now: Instant) {
        let agents = &mut self.agents;
        for presence in &connected {
            // UC-020 A8 step 2: the status bar names the agent and the todo.
            if let Some((at, change)) = &presence.todo_change {
                let name = presence.shown_name().to_owned();
                if agents.todo_seen.insert(name.clone(), *at) != Some(*at) {
                    self.status = format!("{name} {change}");
                }
            }
            let Some(reading) = &presence.reading else {
                continue;
            };
            let name = presence.shown_name().to_owned();
            let known = agents.marks.get(&name).map(|m| m.reading_at);
            if known == Some(reading.at) || !reading.shown_at(at) {
                continue;
            }
            let kept = agents.marks.get(&name).filter(|m| {
                since(now, m.since) < SAME_PLACE
                    && m.note == reading.note
                    && m.lines == reading.lines
            });
            let start = kept.map_or(now, |m| m.since);
            agents.marks.insert(
                name,
                AgentCursor {
                    note: reading.note.clone(),
                    lines: reading.lines,
                    since: start,
                    wrote: reading.wrote,
                    reading_at: reading.at,
                },
            );
        }
        agents.connected = connected;
    }

    /// ADR-025 section 3.6: a request for the open note `note` asks the
    /// window for its edits and waits for them.
    fn wait_for_window(&mut self, taken: Taken, entry: NoteEntry, note: NoteId, now: Instant) {
        let syncs = self
            .open
            .as_ref()
            .and_then(|open| open.tabs.find(&note).map(|i| open.tabs.get(i).syncs));
        let Some(syncs) = syncs else {
            // Closed meanwhile: its file is changed instead.
            self.agents.taken.push(taken);
            return;
        };
        self.request_flush(Some(&note));
        self.agents.waiting.push(Waiting {
            taken,
            note,
            entry,
            asked: now,
            syncs,
        });
    }

    /// The tick's part (ADR-022): the requests for open notes whose window
    /// sent its edits, or did not within [`FLUSH_AT_MOST`], are made.
    pub(super) fn agents_tick(&mut self, now: Instant) {
        let waiting = std::mem::take(&mut self.agents.waiting);
        let root = self.workspace_root().map(Path::to_owned);
        let mut blocked: HashSet<NoteId> = HashSet::new();
        for waiting in waiting {
            if root.as_deref() != Some(waiting.taken.root.as_path()) {
                let Taken { root, id, .. } = waiting.taken;
                self.agents.answers.push((root, id, busy()));
                continue;
            }
            let tab = self
                .open
                .as_ref()
                .and_then(|open| open.tabs.find(&waiting.note).map(|i| open.tabs.get(i)));
            let Some(tab) = tab else {
                // The tab closed: the note's file is changed instead.
                self.agents.taken.push(waiting.taken);
                continue;
            };
            let flushed = !tab.window_synced
                || tab.syncs > waiting.syncs
                || since(now, waiting.asked) >= FLUSH_AT_MOST;
            if !flushed || blocked.contains(&waiting.note) {
                // A later change of a note waits while an earlier one of it
                // waits; changes of different notes do not wait for each
                // other.
                blocked.insert(waiting.note.clone());
                self.agents.waiting.push(waiting);
                continue;
            }
            let Waiting {
                taken, note, entry, ..
            } = waiting;
            let root = taken.root.clone();
            let id = taken.id.clone();
            if let Some(answer) = self.agent_change_tab(taken, &note, &entry, now) {
                self.agents.answers.push((root, id, answer));
            }
        }
        if !self.agents.answers.is_empty() || !self.agents.taken.is_empty() {
            self.wake.wake();
        }
    }

    /// UC-019 steps 2–4, A4–A7, BR-004, BR-007: the change of an agent to
    /// the open note `note`, planned on the stored text and put into the
    /// tab's text with the user's edits, as an undo step of its own; stored
    /// at once. The answer, or `None` when it comes once the note is
    /// stored (`Agents::after_save`).
    fn agent_change_tab(
        &mut self,
        taken: Taken,
        note: &NoteId,
        entry: &NoteEntry,
        now: Instant,
    ) -> Option<AgentAnswer> {
        // UC-017 BR-003: access switched off meanwhile counts at once.
        if !self.settings.effective().agent_access.writes() {
            return Some(only_reading(&taken.request.note));
        }
        let paused = !self.saves_paused.is_empty();
        let Some((open, index)) = self
            .open
            .as_mut()
            .and_then(|open| open.tabs.find(note).map(|index| (open, index)))
        else {
            return Some(busy());
        };
        let tab = open.tabs.get(index);
        // ADR-018: while it is stored (and, ADR-025 section 3.6, while the
        // window has not taken a reload, or git works on the notes).
        let storing = self.agents.after_save.iter().any(|(p, _)| *p == tab.path);
        if tab.saving || tab.mirror.reload_unconfirmed() || paused || storing {
            return Some(busy());
        }
        let current = tab.mirror.text();
        let merged = match plan_for_tab(entry, &tab.on_disk, &current, &taken.request.change) {
            Ok(merged) => merged,
            Err(problem) => return Some(AgentAnswer::refused(&problem)),
        };
        let edit = [(merged.range.clone(), merged.replacement.clone())];
        if let Err(error) = self.backend_edit(note, &edit, true) {
            return Some(AgentAnswer::refused(&WriteProblem::Write {
                note: entry.id.as_str().to_owned(),
                reason: error.message,
            }));
        }
        let Some(open) = self.open.as_mut() else {
            return Some(busy());
        };
        let tab = open.tabs.get_mut(index);
        tab.make_folder();
        let text = tab.mirror.text();
        tab.saving_text = Some(text.clone());
        self.saver.save(SaveJob {
            path: tab.path.clone(),
            text,
            started: now,
        });
        tab.saving = true;
        let path = tab.path.clone();
        self.agents.after_save.push((
            path,
            AnswerAfterSave {
                root: taken.root,
                id: taken.id,
                note: entry.id.clone(),
                answer: answer_for_tab(&entry.id, &merged),
            },
        ));
        self.agents.marks.insert(
            taken.agent,
            AgentCursor {
                note: note.clone(),
                lines: merged.lines,
                since: now,
                wrote: true,
                reading_at: 0,
            },
        );
        None
    }

    /// UC-019 BR-010: says in the status bar which notes not open an agent
    /// created or changed; changes in a row are reported together.
    fn report_agent_change(&mut self, agent: &str, verb: &'static str, note: NoteId) {
        let mut items = match self.agents.report.take() {
            Some(report) if report.status == self.status && report.agent == agent => report.items,
            _ => Vec::new(),
        };
        items.retain(|(_, n)| *n != note);
        items.push((verb, note));
        let mut parts: Vec<String> = Vec::new();
        for verb in ["created", "changed"] {
            let notes: Vec<String> = items
                .iter()
                .filter(|(v, _)| *v == verb)
                .map(|(_, n)| n.title())
                .collect();
            if !notes.is_empty() {
                parts.push(format!("{verb} {}", notes.join(", ")));
            }
        }
        self.status = format!("{agent} {}", parts.join(" and "));
        self.agents.report = Some(AgentReport {
            status: self.status.clone(),
            agent: agent.to_owned(),
            items,
        });
    }

    /// M-04.9 (UC-019 BR-010): "Open" or "Open {title}" for the notes the
    /// report in the status names, the newest first, at most three.
    pub(super) fn agent_opens(&self) -> Vec<AgentOpenView> {
        let Some(report) = self
            .agents
            .report
            .as_ref()
            .filter(|r| r.status == self.status)
        else {
            return Vec::new();
        };
        report
            .items
            .iter()
            .rev()
            .take(3)
            .map(|(verb, note)| AgentOpenView {
                note: note.as_str().to_owned(),
                created: *verb == "created",
                label: if report.items.len() == 1 {
                    "Open".to_owned()
                } else {
                    format!("Open {}", note.title())
                },
            })
            .collect()
    }

    /// D-04 (fixed, A-163): the report of an agent's changes stays while
    /// background saves say "Saved {t}".
    pub(super) fn agent_report_kept(&self) -> bool {
        self.agents
            .report
            .as_ref()
            .is_some_and(|r| r.status == self.status)
    }

    /// UC-017 step 2, BR-002: the level chosen in the agents menu changes
    /// the user setting.
    pub fn set_agent_access(&mut self, level: AgentLevelName) {
        let level = AgentAccess::from(level);
        if level == self.settings.user_agent_access() {
            return;
        }
        self.settings.change(
            Scope::Me,
            SettingKey::AgentAccess,
            Some(level.label().into()),
        );
        self.take_settings_messages();
    }

    /// UC-017 step 1, A1, A2, A4, M-17.1, M-17.2: the agents entry of the
    /// status bar with its menu, and the head of the section "Agents".
    #[must_use]
    pub fn agents_view(&self) -> AgentsView {
        let effective = self.settings.effective();
        let access = effective.agent_access;
        let Some(root) = self.workspace_root() else {
            return AgentsView {
                menu: None,
                head: vec!["Open a workspace to connect agents to it.".into()],
                workspace: false,
            };
        };
        let mut text = format!("Agents: {}", access.status_text());
        if effective.agent_access_lowered {
            text.push_str(" in this workspace");
        }
        let connected = &self.agents.connected;
        let mut asked_for = None;
        if access.reads() && !connected.is_empty() {
            let names: Vec<&str> = connected.iter().map(Presence::shown_name).collect();
            let lead = if names.len() == 1 {
                "Agent connected"
            } else {
                "Agents connected"
            };
            let _ = write!(text, " · {lead}: {}", names.join(", "));
        } else if !access.reads() {
            // A2: an agent asked while access is off.
            let asked: Vec<&str> = connected
                .iter()
                .filter(|p| p.refused)
                .map(Presence::shown_name)
                .collect();
            if !asked.is_empty() {
                asked_for = Some(format!("{} asked for access", asked.join(", ")));
            }
        }
        // A1: the workspace caps what the menu offers.
        let cap = self.settings.workspace_agent_access();
        let user = self.settings.user_agent_access();
        let levels = AgentAccess::ALL
            .iter()
            .filter(|level| cap.is_none_or(|cap| **level <= cap))
            .map(|level| AgentLevelView {
                level: (*level).into(),
                label: level.label().to_owned(),
                chosen: *level == user,
            })
            .collect();
        let access_line = match access {
            AgentAccess::Off => "Agent access is off; agents reach the notes only while it is on.",
            AgentAccess::ReadOnly => "Agent access is read only.",
            AgentAccess::ReadWrite => "Agent access is read and write.",
        };
        AgentsView {
            menu: Some(AgentsMenuView {
                text,
                asked: asked_for,
                hover: "What agents may do with the notes through Underline's MCP server".into(),
                levels,
                workspace_settings: "Settings of this workspace…".into(),
            }),
            head: vec![
                format!(
                    "Let an agent tool reach the notes of {} through Underline's MCP server.",
                    root.display()
                ),
                access_line.into(),
            ],
            workspace: true,
        }
    }

    /// UC-018 BR-006, UC-019 BR-006, UC-017 BR-005: each open note shows the
    /// cursors of the connected agents that read or wrote in it within 10
    /// seconds of `now`, as far as access allows.
    fn agent_marks(&mut self, now: Instant) -> Vec<AgentMarksView> {
        let access = self.settings.effective().agent_access;
        let connected: HashSet<&str> = self
            .agents
            .connected
            .iter()
            .map(Presence::shown_name)
            .collect();
        self.agents.marks.retain(|name, m| {
            connected.contains(name.as_str()) && since(now, m.since) <= READING_SHOWN
        });
        let Some(open) = &self.open else {
            return Vec::new();
        };
        let mut views = Vec::new();
        for (note, _) in open.tabs.iter() {
            let mut marks: Vec<AgentMarkView> = self
                .agents
                .marks
                .iter()
                .filter(|(_, m)| &m.note == note)
                .filter(|(_, m)| {
                    if m.wrote {
                        access.writes()
                    } else {
                        access.reads()
                    }
                })
                .map(|(name, m)| AgentMarkView {
                    from: count(m.lines.0.saturating_sub(1)),
                    to: count(m.lines.1.max(m.lines.0)),
                    name: name.clone(),
                    wrote: m.wrote,
                })
                .collect();
            if marks.is_empty() {
                continue;
            }
            marks.sort_by(|a, b| a.from.cmp(&b.from).then_with(|| a.name.cmp(&b.name)));
            views.push(AgentMarksView {
                note: note.as_str().to_owned(),
                marks,
            });
        }
        views
    }

    /// The agents' events: the status bar's entry and the settings' head
    /// when they changed.
    pub(super) fn agents_event(&mut self) -> Option<Event> {
        let view = self.agents_view();
        if self.agents.told.as_ref() == Some(&view) {
            return None;
        }
        self.agents.told = Some(view.clone());
        Some(Event::Agents(view))
    }

    /// The agent cursors at `now`, when they changed (M-17.6).
    pub(super) fn agent_marks_event(&mut self, now: Instant) -> Option<Event> {
        let marks = self.agent_marks(now);
        if marks == self.agents.told_marks {
            return None;
        }
        self.agents.told_marks.clone_from(&marks);
        Some(Event::AgentMarks(marks))
    }

    /// The window was told the agents view at start.
    pub(super) fn agents_told(&mut self) {
        self.agents.told = Some(self.agents_view());
    }

    /// M-21.3, inventory D-05 (fixed, A-162): the workspace at `root` is
    /// left: its window presence goes, its waiting requests are answered
    /// busy, and what was known of its agents is forgotten.
    pub(super) fn agents_leave(&mut self, root: &Path) {
        let agents = &mut self.agents;
        agents.left.push(root.to_owned());
        for waiting in std::mem::take(&mut agents.waiting) {
            let Taken { root, id, .. } = waiting.taken;
            agents.answers.push((root, id, busy()));
        }
        for taken in std::mem::take(&mut agents.taken) {
            agents.answers.push((taken.root, taken.id, busy()));
        }
        agents.connected.clear();
        agents.marks.clear();
        agents.looked = None;
        agents.requests_taken = None;
        agents.section.forget();
    }

    /// M-17.8, M-21.4: Underline ends: every request still open is
    /// answered (a stored change with its answer, the others busy, so their
    /// servers ask again), and the window's presence goes, so the MCP
    /// servers write the files themselves. The saves were settled before.
    pub(super) fn agents_end(&mut self) {
        if let Some(root) = self.workspace_root().map(Path::to_owned) {
            self.agents_leave(&root);
        }
        let agents = &mut self.agents;
        for (_, after) in std::mem::take(&mut agents.after_save) {
            agents.answers.push((after.root, after.id, busy()));
        }
        for (root, id, answer) in std::mem::take(&mut agents.answers) {
            if let Err(err) = answer_request(&root, &id, &answer) {
                eprintln!("An agent's change could not be answered: {err}");
            }
        }
        for root in std::mem::take(&mut agents.left) {
            remove_window_presence(&root);
        }
    }

    /// UC-019: a save of `path` ended (`stored`); an agent whose change it
    /// stored is answered: its answer, or why the note could not be
    /// written (A9; the tab keeps the change). Returns whether it was an
    /// agent's save.
    pub(super) fn agent_saved(&mut self, path: &Path, stored: Result<(), &str>) -> bool {
        let Some(at) = self.agents.after_save.iter().position(|(p, _)| p == path) else {
            return false;
        };
        let (_, after) = self.agents.after_save.remove(at);
        self.agents.answers.push(after.answer(stored));
        self.wake.wake();
        true
    }

    /// UC-019: an agent's change is being stored.
    pub(super) fn agents_storing(&self) -> bool {
        !self.agents.after_save.is_empty()
    }

    /// ADR-025 section 3.6: the window sent its edits; an agent's change
    /// waiting for them is made at once.
    pub(super) fn agents_synced(&self) {
        if !self.agents.waiting.is_empty() {
            self.wake.wake();
        }
    }
}

fn count(line: usize) -> u32 {
    u32::try_from(line).unwrap_or(u32::MAX)
}
