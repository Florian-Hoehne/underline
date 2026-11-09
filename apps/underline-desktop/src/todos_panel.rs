//! UC-020: the section "Todos" of the tool panel. It shows the todos and
//! gives back what the user asked for; the window carries it out.

use std::collections::HashMap;
use std::time::Instant;

use eframe::egui;
use underline_core::{Day, NoteId, Todo, TodoConflict, Todos, Urgency};

/// What the section remembers between frames.
#[derive(Default)]
pub struct TodosView {
    pub todos: Todos,
    /// UC-020 A7: the files when last read, and when they were looked at.
    pub fingerprint: Vec<(String, Option<std::time::SystemTime>, u64)>,
    pub checked: Option<Instant>,
    /// Step 3: the title typed for a new todo, and why it was not taken.
    pub new_title: String,
    pub new_problem: Option<String>,
    /// The todo shown with its fields, and what is typed into them.
    pub open: Option<u64>,
    pub edit: Edit,
    /// A9: changes that could not be stored, with the reason.
    pub unstored: HashMap<u64, (Todo, String)>,
    /// A5: the last deleted todo and the report that offers to undo.
    pub deleted: Option<(Todo, String)>,
    /// A10: the todos in conflict after a sync.
    pub conflicts: Vec<TodoConflict>,
    /// The month the calendar shows, and for which field.
    pub calendar: Option<(DateField, u64, Day)>,
}

/// What is typed into the fields of the open todo.
#[derive(Default, Clone)]
pub struct Edit {
    pub number: u64,
    pub title: String,
    pub due: String,
    pub scheduled: String,
    pub link_query: String,
    pub problem: Option<String>,
}

impl Edit {
    fn of(todo: &Todo) -> Self {
        let day = |d: Option<Day>| d.map(|d| d.to_string()).unwrap_or_default();
        Edit {
            number: todo.number,
            title: todo.title.clone(),
            due: day(todo.due),
            scheduled: day(todo.scheduled),
            link_query: String::new(),
            problem: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DateField {
    Due,
    Scheduled,
}

/// What the user asked for in the section.
pub enum TodoAction {
    Create(String),
    Save(Todo),
    Delete(u64),
    Open(NoteId),
    MissingNote(String),
    Resolve(u64, Option<Todo>),
}

impl TodosView {
    /// A7: the todos as they are now; the fields of the open todo keep what
    /// the user typed.
    pub fn replace(&mut self, todos: Todos) {
        self.todos = todos;
        if let Some(number) = self.open {
            if self.todos.get(number).is_none() && !self.unstored.contains_key(&number) {
                self.open = None;
            }
        }
    }

    fn shown(&self, number: u64) -> Option<Todo> {
        self.unstored
            .get(&number)
            .map(|(t, _)| t.clone())
            .or_else(|| self.todos.get(number).cloned())
    }
}

fn urgency_color(ui: &egui::Ui, urgency: Urgency) -> egui::Color32 {
    match urgency {
        Urgency::Low => ui.visuals().weak_text_color(),
        Urgency::Normal => ui.visuals().text_color(),
        Urgency::High => egui::Color32::from_rgb(220, 140, 20),
        Urgency::Critical => egui::Color32::from_rgb(210, 50, 50),
    }
}

/// UC-020 steps 1–10, A1–A5, A10: the section "Todos". `active` is the note
/// of the active tab, `notes` the notes of the workspace as listed.
pub fn todos_ui(
    ui: &mut egui::Ui,
    view: &mut TodosView,
    active: Option<&NoteId>,
    notes: &[NoteId],
) -> Vec<TodoAction> {
    let mut actions = Vec::new();
    let open_count = view.todos.todos.iter().filter(|t| !t.is_done()).count();
    egui::CollapsingHeader::new(format!("Todos ({open_count})"))
        .id_salt("todos")
        .default_open(true)
        .show(ui, |ui| {
            conflicts_ui(ui, view, &mut actions);
            // Step 3: a new todo.
            let field = ui.add(
                egui::TextEdit::singleline(&mut view.new_title)
                    .hint_text("New todo")
                    .desired_width(f32::INFINITY),
            );
            if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                actions.push(TodoAction::Create(view.new_title.clone()));
                field.request_focus();
            }
            if let Some(problem) = &view.new_problem {
                ui.colored_label(ui.visuals().warn_fg_color, problem);
            }
            for problem in &view.todos.unreadable {
                ui.colored_label(ui.visuals().warn_fg_color, problem.to_string());
            }
            let mut open: Vec<Todo> = view
                .todos
                .todos
                .iter()
                .filter(|t| !t.is_done())
                .map(|t| view.shown(t.number).unwrap_or_else(|| t.clone()))
                .collect();
            // UC-021 orders and filters the list; until then the next due first.
            open.sort_by_key(|t| (t.due.is_none(), t.due, t.number));
            if open.is_empty() {
                ui.weak("No open todos");
            }
            for todo in open {
                todo_row(ui, view, todo, active, notes, &mut actions);
            }
            let done: Vec<Todo> = view
                .todos
                .todos
                .iter()
                .filter(|t| t.is_done())
                .cloned()
                .collect();
            if !done.is_empty() {
                egui::CollapsingHeader::new(format!("Done ({})", done.len()))
                    .id_salt("todos-done")
                    .show(ui, |ui| {
                        for todo in done {
                            todo_row(ui, view, todo, active, notes, &mut actions);
                        }
                    });
            }
        });
    calendar_ui(ui, view, &mut actions);
    actions
}

/// One todo: its line, and its fields while it is open (steps 5–10).
fn todo_row(
    ui: &mut egui::Ui,
    view: &mut TodosView,
    todo: Todo,
    active: Option<&NoteId>,
    notes: &[NoteId],
    actions: &mut Vec<TodoAction>,
) {
    let number = todo.number;
    let unstored = view.unstored.get(&number).map(|(_, why)| why.clone());
    ui.horizontal(|ui| {
        let mut done = todo.is_done();
        if ui.checkbox(&mut done, "").on_hover_text("Done").changed() {
            let mut changed = todo.clone();
            changed.set_done(done);
            actions.push(TodoAction::Save(changed));
        }
        let mut text = egui::RichText::new(todo.name()).color(urgency_color(ui, todo.urgency));
        if todo.is_done() {
            text = text.strikethrough().weak();
        }
        if ui
            .selectable_label(view.open == Some(number), text)
            .clicked()
        {
            if view.open == Some(number) {
                view.open = None;
            } else {
                view.open = Some(number);
                view.edit = Edit::of(&todo);
            }
        }
        if let Some(due) = todo.due {
            ui.weak(format!("due {due}"));
        }
        if let Some(why) = &unstored {
            ui.colored_label(ui.visuals().warn_fg_color, "not stored")
                .on_hover_text(why);
        }
    });
    if view.open != Some(number) {
        return;
    }
    if view.edit.number != number {
        view.edit = Edit::of(&todo);
    }
    ui.indent(("todo", number), |ui| {
        egui::Grid::new(("todo-fields", number))
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                ui.weak("Title");
                let title = ui.text_edit_singleline(&mut view.edit.title);
                if title.lost_focus() && view.edit.title != todo.title {
                    let mut changed = todo.clone();
                    match changed.set_title(&view.edit.title) {
                        Ok(()) => {
                            view.edit.problem = None;
                            actions.push(TodoAction::Save(changed));
                        }
                        Err(err) => view.edit.problem = Some(err.to_string()),
                    }
                }
                ui.end_row();
                for (label, field) in [("Due", DateField::Due), ("Scheduled", DateField::Scheduled)]
                {
                    ui.weak(label);
                    ui.horizontal(|ui| {
                        let text = match field {
                            DateField::Due => &mut view.edit.due,
                            DateField::Scheduled => &mut view.edit.scheduled,
                        };
                        let edit = ui.add(
                            egui::TextEdit::singleline(text)
                                .hint_text("YYYY-MM-DD")
                                .desired_width(90.0),
                        );
                        if edit.lost_focus() {
                            let typed = text.trim().to_owned();
                            let parsed = if typed.is_empty() {
                                Ok(None)
                            } else {
                                typed.parse::<Day>().map(Some)
                            };
                            match parsed {
                                Ok(day) => {
                                    let mut changed = todo.clone();
                                    match field {
                                        DateField::Due => changed.due = day,
                                        DateField::Scheduled => changed.scheduled = day,
                                    }
                                    if changed != todo {
                                        actions.push(TodoAction::Save(changed));
                                    }
                                    view.edit.problem = None;
                                }
                                Err(err) => view.edit.problem = Some(err.to_string()),
                            }
                        }
                        if ui
                            .small_button("Pick")
                            .on_hover_text("Choose a day in a calendar")
                            .clicked()
                        {
                            let current = match field {
                                DateField::Due => todo.due,
                                DateField::Scheduled => todo.scheduled,
                            };
                            let month = current.unwrap_or_else(Day::today).month_start();
                            view.calendar = Some((field, number, month));
                        }
                    });
                    ui.end_row();
                }
                ui.weak("Urgency");
                let mut urgency = todo.urgency;
                egui::ComboBox::from_id_salt(("urgency", number))
                    .selected_text(urgency.label())
                    .show_ui(ui, |ui| {
                        for value in Urgency::ALL {
                            ui.selectable_value(&mut urgency, value, value.label());
                        }
                    });
                if urgency != todo.urgency {
                    let mut changed = todo.clone();
                    changed.urgency = urgency;
                    actions.push(TodoAction::Save(changed));
                }
                ui.end_row();
            });
        if todo.scheduled_after_due() {
            ui.weak("The scheduled day is after the due day.");
        }
        if let Some(problem) = &view.edit.problem {
            ui.colored_label(ui.visuals().warn_fg_color, problem);
        }
        // Step 8, A2, A6: the linked notes.
        ui.weak("Notes");
        for link in &todo.links {
            ui.horizontal(|ui| {
                let exists = notes.iter().any(|n| n.as_str().eq_ignore_ascii_case(link));
                let label = NoteId::from_relative(link).title();
                if exists {
                    if ui.link(&label).on_hover_text(link).clicked() {
                        actions.push(TodoAction::Open(NoteId::from_relative(link)));
                    }
                } else if ui
                    .link(egui::RichText::new(format!("{label} (missing)")).weak())
                    .clicked()
                {
                    actions.push(TodoAction::MissingNote(link.clone()));
                }
                if ui
                    .small_button("×")
                    .on_hover_text("Remove the link")
                    .clicked()
                {
                    let mut changed = todo.clone();
                    changed.unlink(link);
                    actions.push(TodoAction::Save(changed));
                }
            });
        }
        // Step 7: the note of the active tab.
        if let Some(active) = active.filter(|a| !todo.links_to(a)) {
            if ui
                .small_button(format!("Link {}", active.title()))
                .clicked()
            {
                let mut changed = todo.clone();
                changed.link(active);
                actions.push(TodoAction::Save(changed));
            }
        }
        // A1: another note, found by its title.
        ui.add(
            egui::TextEdit::singleline(&mut view.edit.link_query)
                .hint_text("Link a note…")
                .desired_width(f32::INFINITY),
        );
        let query = view.edit.link_query.trim().to_lowercase();
        if !query.is_empty() {
            let mut shown = 0;
            for note in notes {
                if shown == 8 {
                    break;
                }
                if todo.links_to(note) || !note.title().to_lowercase().contains(&query) {
                    continue;
                }
                shown += 1;
                if ui
                    .selectable_label(false, note.as_str().replace('-', " "))
                    .clicked()
                {
                    let mut changed = todo.clone();
                    changed.link(note);
                    actions.push(TodoAction::Save(changed));
                    view.edit.link_query.clear();
                }
            }
            if shown == 0 {
                ui.weak("No note matches");
            }
        }
        ui.horizontal(|ui| {
            if ui.small_button("Delete").clicked() {
                actions.push(TodoAction::Delete(number));
            }
            if let Some(done) = &todo.done {
                ui.weak(format!(
                    "done {}",
                    done.get(..16).unwrap_or(done).replace('T', " ")
                ));
            } else {
                ui.weak(format!(
                    "created {}",
                    todo.created
                        .get(..16)
                        .unwrap_or(&todo.created)
                        .replace('T', " ")
                ));
            }
        });
    });
}

/// A10: each todo in conflict, both versions field by field.
fn conflicts_ui(ui: &mut egui::Ui, view: &TodosView, actions: &mut Vec<TodoAction>) {
    for conflict in &view.conflicts {
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.colored_label(
                ui.visuals().warn_fg_color,
                format!(
                    "Todo #{} was changed here and on the remote",
                    conflict.number
                ),
            );
            let fields = |t: &Option<Todo>| -> Vec<String> {
                match t {
                    None => vec!["(deleted)".into(); 6],
                    Some(t) => vec![
                        t.title.clone(),
                        t.due.map(|d| d.to_string()).unwrap_or_else(|| "–".into()),
                        t.scheduled
                            .map(|d| d.to_string())
                            .unwrap_or_else(|| "–".into()),
                        t.urgency.label().to_owned(),
                        if t.is_done() {
                            "done".into()
                        } else {
                            "open".into()
                        },
                        if t.links.is_empty() {
                            "–".into()
                        } else {
                            t.links.join(", ")
                        },
                    ],
                }
            };
            let (mine, remote) = (fields(&conflict.mine), fields(&conflict.remote));
            egui::Grid::new(("todo-conflict", conflict.number))
                .num_columns(3)
                .spacing([8.0, 2.0])
                .show(ui, |ui| {
                    ui.label("");
                    ui.strong("Here");
                    ui.strong("Remote");
                    ui.end_row();
                    for (i, name) in ["Title", "Due", "Scheduled", "Urgency", "Status", "Notes"]
                        .iter()
                        .enumerate()
                    {
                        ui.weak(*name);
                        let differ = mine[i] != remote[i];
                        let text = |t: &str| {
                            let rich = egui::RichText::new(t.to_owned());
                            if differ {
                                rich.strong()
                            } else {
                                rich
                            }
                        };
                        ui.label(text(&mine[i]));
                        ui.label(text(&remote[i]));
                        ui.end_row();
                    }
                });
            ui.horizontal(|ui| {
                if ui.button("Keep mine").clicked() {
                    actions.push(TodoAction::Resolve(conflict.number, conflict.mine.clone()));
                }
                if ui.button("Take remote").clicked() {
                    actions.push(TodoAction::Resolve(
                        conflict.number,
                        conflict.remote.clone(),
                    ));
                }
            });
        });
    }
}

/// UC-020 BR-003: a month to choose a day from.
fn calendar_ui(ui: &mut egui::Ui, view: &mut TodosView, actions: &mut Vec<TodoAction>) {
    let Some((field, number, month)) = view.calendar else {
        return;
    };
    // The calendar belongs to the todo it was opened for.
    let Some(todo) = (view.open == Some(number))
        .then(|| view.shown(number))
        .flatten()
    else {
        view.calendar = None;
        return;
    };
    let mut open = true;
    // Some(None) is "No day".
    let mut chosen: Option<Option<Day>> = None;
    let mut shown = month;
    egui::Window::new("Choose a day")
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                if ui.small_button("◀").clicked() {
                    shown = month.add_months(-1);
                }
                ui.strong(month.month_title());
                if ui.small_button("▶").clicked() {
                    shown = month.add_months(1);
                }
            });
            egui::Grid::new("calendar").num_columns(7).show(ui, |ui| {
                for name in ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"] {
                    ui.weak(name);
                }
                ui.end_row();
                let lead = month.monday_offset();
                for _ in 0..lead {
                    ui.label("");
                }
                let today = Day::today();
                for number in 1..=month.days_in_month() {
                    let day = month.with_day(number);
                    let text = egui::RichText::new(number.to_string());
                    let text = if day == today { text.strong() } else { text };
                    if ui.small_button(text).clicked() {
                        chosen = Some(Some(day));
                    }
                    if (lead + number) % 7 == 0 {
                        ui.end_row();
                    }
                }
            });
            if ui.small_button("No day").clicked() {
                chosen = Some(None);
            }
        });
    view.calendar = open.then_some((field, number, shown));
    if let Some(day) = chosen {
        view.calendar = None;
        let mut changed = todo.clone();
        let text = day.map(|d| d.to_string()).unwrap_or_default();
        match field {
            DateField::Due => {
                changed.due = day;
                view.edit.due = text;
            }
            DateField::Scheduled => {
                changed.scheduled = day;
                view.edit.scheduled = text;
            }
        }
        actions.push(TodoAction::Save(changed));
    }
}
