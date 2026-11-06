use eframe::egui;
use underline_core::{Note, NoteId, Notebook};
use underline_wiki::WikiRemote;

/// Main window: page list on the left, Markdown editor in the middle.
pub struct UnderlineApp {
    notebook: Notebook,
    pages: Vec<NoteId>,
    current: Option<Note>,
    new_page: String,
    wiki_url: String,
    status: String,
}

impl UnderlineApp {
    pub fn new(notebook: Notebook) -> Self {
        let mut app = Self {
            notebook,
            pages: Vec::new(),
            current: None,
            new_page: String::new(),
            wiki_url: String::new(),
            status: String::new(),
        };
        app.refresh();
        app
    }

    fn refresh(&mut self) {
        match self.notebook.list() {
            Ok(pages) => self.pages = pages,
            Err(err) => self.status = err.to_string(),
        }
    }

    fn open(&mut self, id: &NoteId) {
        match self.notebook.read(id) {
            Ok(note) => self.current = Some(note),
            Err(err) => self.status = err.to_string(),
        }
    }

    fn save(&mut self) {
        if let Some(note) = &self.current {
            self.status = match self.notebook.write(note) {
                Ok(()) => format!("Saved {}", note.id),
                Err(err) => err.to_string(),
            };
            self.refresh();
        }
    }
}

impl eframe::App for UnderlineApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("wiki").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Wiki project URL");
                ui.text_edit_singleline(&mut self.wiki_url);
                if ui.button("Check").clicked() {
                    self.status = match WikiRemote::from_project_url(&self.wiki_url) {
                        Ok(remote) => format!("Wiki repository: {}", remote.clone_url()),
                        Err(err) => err.to_string(),
                    };
                }
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.label(format!(
                "{}  |  {}",
                self.notebook.root().display(),
                self.status
            ));
        });

        egui::SidePanel::left("pages").show(ctx, |ui| {
            ui.heading("Pages");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut self.new_page);
                if ui.button("New").clicked() && !self.new_page.trim().is_empty() {
                    self.current = Some(Note::new(&self.new_page, ""));
                    self.new_page.clear();
                }
            });
            ui.separator();
            let mut selected = None;
            for id in &self.pages {
                let active = self.current.as_ref().is_some_and(|n| &n.id == id);
                if ui.selectable_label(active, id.as_str()).clicked() {
                    selected = Some(id.clone());
                }
            }
            if let Some(id) = selected {
                self.open(&id);
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(title) = self.current.as_ref().map(Note::title) else {
                ui.label("Select or create a page.");
                return;
            };
            ui.horizontal(|ui| {
                ui.heading(title);
                if ui.button("Save").clicked() {
                    self.save();
                }
            });
            if let Some(note) = &mut self.current {
                ui.add_sized(
                    ui.available_size(),
                    egui::TextEdit::multiline(&mut note.body).code_editor(),
                );
            }
        });
    }
}
