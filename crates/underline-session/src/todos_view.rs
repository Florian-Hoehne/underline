//! What the window and the session exchange about the todos (feature F15:
//! UC-020 Manage Todos, UC-021 Work Through Todo List; inventory M-04.5,
//! M-04.6, M-06.5, M-16.3, M-18): the section "Todos" of the tool panel,
//! the tab "Todos", the overdue todos of the status bar and the changes the
//! user makes. The texts are egui's (`apps/underline-desktop/src/todos_panel.rs`).

use serde::{Deserialize, Serialize};
use underline_core::{TodoFilter, TodoSort, Urgency};

/// UC-020 BR-001: how pressing a todo is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum UrgencyName {
    /// Shown in the weak colour.
    Low,
    /// Shown in the text colour.
    Normal,
    /// Shown in egui's orange.
    High,
    /// Shown in egui's red.
    Critical,
}

impl From<Urgency> for UrgencyName {
    fn from(urgency: Urgency) -> Self {
        match urgency {
            Urgency::Low => Self::Low,
            Urgency::Normal => Self::Normal,
            Urgency::High => Self::High,
            Urgency::Critical => Self::Critical,
        }
    }
}

impl From<UrgencyName> for Urgency {
    fn from(urgency: UrgencyName) -> Self {
        match urgency {
            UrgencyName::Low => Self::Low,
            UrgencyName::Normal => Self::Normal,
            UrgencyName::High => Self::High,
            UrgencyName::Critical => Self::Critical,
        }
    }
}

/// UC-021 BR-003: which todos the tab "Todos" shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum TodoFilterName {
    /// "Open", the default.
    Open,
    /// "Done".
    Done,
    /// "All".
    All,
}

impl From<TodoFilter> for TodoFilterName {
    fn from(filter: TodoFilter) -> Self {
        match filter {
            TodoFilter::Open => Self::Open,
            TodoFilter::Done => Self::Done,
            TodoFilter::All => Self::All,
        }
    }
}

impl From<TodoFilterName> for TodoFilter {
    fn from(filter: TodoFilterName) -> Self {
        match filter {
            TodoFilterName::Open => Self::Open,
            TodoFilterName::Done => Self::Done,
            TodoFilterName::All => Self::All,
        }
    }
}

/// UC-021 BR-001, BR-002: the order of the tab "Todos".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum TodoSortName {
    /// "Due date".
    Due,
    /// "Urgency".
    Urgency,
}

impl From<TodoSort> for TodoSortName {
    fn from(sort: TodoSort) -> Self {
        match sort {
            TodoSort::Due => Self::Due,
            TodoSort::Urgency => Self::Urgency,
        }
    }
}

impl From<TodoSortName> for TodoSort {
    fn from(sort: TodoSortName) -> Self {
        match sort {
            TodoSortName::Due => Self::Due,
            TodoSortName::Urgency => Self::Urgency,
        }
    }
}

/// M-18.4, UC-020 step 8, A6: a note a todo links to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TodoLinkView {
    /// The link as stored (`Projects/Plan`), shown at the pointer and sent
    /// back to open or remove it.
    pub link: String,
    /// The note's title (`Plan`).
    pub title: String,
    /// The note is in the workspace; else the todo shows it "(missing)".
    pub exists: bool,
}

/// M-18.2, M-18.3, M-18.8: one todo, as the section and the tab show it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TodoView {
    /// The number (`#12`), sent back with every change.
    pub number: u32,
    /// The title.
    pub title: String,
    /// How it is named in messages and for screen readers: `#12 Title`.
    pub name: String,
    /// The urgency, which colours the title.
    pub urgency: UrgencyName,
    /// "Low", "Normal", "High" or "Critical".
    pub urgency_label: String,
    /// Done, shown struck through.
    pub done: bool,
    /// The due day, `YYYY-MM-DD`.
    pub due: Option<String>,
    /// The scheduled day, `YYYY-MM-DD`.
    pub scheduled: Option<String>,
    /// M-18.8, UC-021 A2: "{due}  1 day overdue" or "{due}  N days
    /// overdue", in the error colour; `None` when it is not overdue.
    pub overdue: Option<String>,
    /// M-18.3, UC-020 BR-003: "The scheduled day is after the due day."
    pub scheduled_after_due: bool,
    /// The notes it links to, in order.
    pub links: Vec<TodoLinkView>,
    /// M-18.8, UC-021 BR-009: "+N" after the first two notes of a row.
    pub more_links: Option<String>,
    /// UC-021 BR-009: "Plan, Ideas +2", shown at the pointer on "+N".
    pub links_summary: String,
    /// It links to the note of the active tab; "Link {title}" is not
    /// offered then.
    pub links_active: bool,
    /// M-18.5: "created YYYY-MM-DD HH:MM" or "done YYYY-MM-DD HH:MM".
    pub stamp: String,
    /// M-18.8: the column "Done": "YYYY-MM-DD HH:MM" while done.
    pub done_at: Option<String>,
    /// M-18.2, UC-020 A9: why the last change could not be stored; the
    /// todo shows "not stored" with it at the pointer.
    pub unstored: Option<String>,
}

/// M-18.4: the note of the active tab, which a todo can link
/// ("Link {title}").
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TodoActiveNoteView {
    /// The note.
    pub note: String,
    /// "Link {title}".
    pub link_label: String,
}

/// M-18.6, M-18.8 (UC-021): the tab "Todos": its sort, its filter and its
/// rows in their order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TodoTableView {
    /// The sort chosen (BR-001, BR-002), kept for the workspace.
    pub sort: TodoSortName,
    /// The filter chosen (BR-003), kept for the workspace.
    pub filter: TodoFilterName,
    /// The numbers of the rows, in the order of the sort.
    pub rows: Vec<u32>,
    /// A3: "No open todos. Type a title above to create one.", "No done
    /// todos." or "No todos. Type a title above to create one." when no
    /// todo is shown.
    pub empty: Option<String>,
    /// The heading of the column "Done": empty under "Open".
    pub done_heading: String,
}

/// M-16.3, M-18, M-04.5 (UC-020, UC-021): the todos of the open workspace,
/// as read last, with the changes not stored yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TodosView {
    /// M-18.1: "Todos (N)", N the open todos.
    pub heading: String,
    /// M-06.5: how many todos are open, for the tab "Todos (N)".
    pub open_count: u32,
    /// Every todo, by number.
    pub todos: Vec<TodoView>,
    /// M-18.1: the open todos in the section's order: by due day, those
    /// without one last, then by number.
    pub open: Vec<u32>,
    /// M-18.1: the done todos, by number ("Done (N)").
    pub done: Vec<u32>,
    /// M-18.1: one line per todo file that cannot be read.
    pub unreadable: Vec<String>,
    /// M-04.5, UC-021 A2: "1 todo overdue" or "N todos overdue"; `None`
    /// while none is.
    pub overdue: Option<String>,
    /// M-18.4: the note of the active note tab; `None` while none is, or
    /// while the tab "Todos" is the active one.
    pub active_note: Option<TodoActiveNoteView>,
    /// M-18.6, M-18.8: the tab "Todos".
    pub table: TodoTableView,
}

/// M-18.3, M-18.4, M-18.2 (UC-020 steps 5 to 10, A1, A2, A4): a change of
/// one todo, made to the todo as it is shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TodoChange {
    /// The title typed (A3: 1 to 200 characters).
    Title {
        /// What was typed.
        title: String,
    },
    /// The due day typed (`YYYY-MM-DD`), or chosen in the calendar; empty
    /// for none.
    Due {
        /// What was typed.
        day: String,
    },
    /// The scheduled day, likewise.
    Scheduled {
        /// What was typed.
        day: String,
    },
    /// The urgency chosen.
    Urgency {
        /// The urgency.
        urgency: UrgencyName,
    },
    /// The checkbox of the section: done or open again (steps 9, 10, A4).
    Done {
        /// Done.
        done: bool,
    },
    /// Links the note `note` (step 7, A1).
    Link {
        /// The note, as listed.
        note: String,
    },
    /// Removes the link `link` (A2).
    Unlink {
        /// The link, as stored.
        link: String,
    },
}

/// What a todo command answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TodoAnswer {
    /// Done; a new todo's field is emptied.
    Done,
    /// UC-020 A3, M-18.3: why it was not done, shown under the field, whose
    /// text stays.
    Problem {
        /// egui's text, such as "a todo needs a title".
        problem: String,
    },
    /// It failed; the status bar says why, the field keeps its text.
    Failed,
}

/// M-18.4, UC-020 A1: a note "Link a note…" offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TodoCandidateView {
    /// The note, sent back to link it.
    pub note: String,
    /// The note as the overview's place names it (`Projects/Road map`).
    pub label: String,
}

/// M-18.4, UC-020 A1: what "Link a note…" offers for the text typed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TodoCandidatesView {
    /// Up to 8 notes whose title holds the text, not linked yet.
    pub notes: Vec<TodoCandidateView>,
    /// "No note matches" when none does and something was typed.
    pub none: Option<String>,
}

/// One day of the calendar "Choose a day" (M-18.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CalendarDayView {
    /// The day, `YYYY-MM-DD`, sent back when it is chosen.
    pub day: String,
    /// Its number in the month.
    pub label: u32,
    /// It is today, shown in bold.
    pub today: bool,
}

/// M-18.3, UC-020 BR-003: a month of the calendar "Choose a day", Monday
/// first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CalendarView {
    /// The month and year, such as "November 2026".
    pub title: String,
    /// The first day of the month, `YYYY-MM-DD`, sent back with ◀ and ▶.
    pub month: String,
    /// How many empty places come before the first day (0 for Monday).
    pub lead: u32,
    /// The days of the month.
    pub days: Vec<CalendarDayView>,
}
