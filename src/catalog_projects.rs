use super::*;

#[derive(Clone, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub(crate) struct ProjectFolder {
    pub project: usize,
    pub parent: Option<usize>,
    pub title: String,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum ProjectBranch {
    Project(usize),
    Folder(usize),
    Unfiled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ProjectLocation {
    pub project: Option<usize>,
    pub folder: Option<usize>,
}

impl ProjectLocation {
    pub(super) fn root(project: Option<usize>) -> Self {
        Self {
            project,
            folder: None,
        }
    }
}

#[derive(Clone, Copy)]
enum TreeRow {
    Branch(ProjectBranch, usize, (usize, usize)),
    Memo(usize, usize),
}

impl App {
    fn project_branch_location(&self, branch: ProjectBranch) -> Option<ProjectLocation> {
        match branch {
            ProjectBranch::Project(id) => Some(ProjectLocation::root(Some(id))),
            ProjectBranch::Unfiled => Some(ProjectLocation::root(None)),
            ProjectBranch::Folder(id) => Some(ProjectLocation {
                project: Some(self.catalog.project_folders.get(id)?.project),
                folder: Some(id),
            }),
        }
    }

    pub(super) fn valid_project_location(&self, location: ProjectLocation) -> bool {
        self.catalog_valid_project(location.project)
            && location.folder.is_none_or(|id| {
                self.catalog
                    .project_folders
                    .get(id)
                    .is_some_and(|folder| Some(folder.project) == location.project)
            })
    }

    pub(super) fn project_note_location(&self, id: usize) -> ProjectLocation {
        let project = self.data.notes[id].project;
        let folder = self
            .catalog
            .project_assignments
            .get(&id)
            .copied()
            .filter(|&f| {
                self.catalog
                    .project_folders
                    .get(f)
                    .is_some_and(|folder| Some(folder.project) == project)
            });
        ProjectLocation { project, folder }
    }

    pub(super) fn place_project_note(&mut self, id: usize, location: ProjectLocation) {
        self.data.notes[id].project = location.project;
        if let Some(folder) = location.folder {
            self.catalog.project_assignments.insert(id, folder);
        } else {
            self.catalog.project_assignments.remove(&id);
        }
    }

    fn create_project_folder(&mut self, target: ProjectLocation) {
        if !self.valid_project_location(target) {
            return;
        }
        let Some(project) = target.project else {
            return;
        };
        let id = self.catalog.project_folders.len();
        self.catalog.project_folders.push(ProjectFolder {
            project,
            parent: target.folder,
            title: "새 폴더".into(),
        });
        self.catalog.project_expanded.insert(
            target
                .folder
                .map_or(ProjectBranch::Project(project), ProjectBranch::Folder),
        );
        self.catalog.project_branch = Some(ProjectBranch::Folder(id));
        self.rename_project_folder(id);
        self.saving.request();
    }

    fn rename_project_folder(&mut self, id: usize) {
        self.catalog.project_folder_rename =
            Some((id, self.catalog.project_folders[id].title.clone(), true));
    }

    fn project_tree_rows(&self) -> Vec<TreeRow> {
        let mut memos = HashMap::<ProjectBranch, Vec<usize>>::new();
        let mut children = HashMap::<ProjectBranch, Vec<ProjectBranch>>::new();
        let mut counts = HashMap::<ProjectBranch, (usize, usize)>::new();
        for (id, folder) in self.catalog.project_folders.iter().enumerate() {
            let parent = folder
                .parent
                .filter(|&p| p < id && self.catalog.project_folders[p].project == folder.project);
            let branch = parent.map_or(
                ProjectBranch::Project(folder.project),
                ProjectBranch::Folder,
            );
            children
                .entry(branch)
                .or_default()
                .push(ProjectBranch::Folder(id));
        }
        for (id, note) in self
            .data
            .notes
            .iter()
            .enumerate()
            .filter(|(id, n)| self.visible(*id) && !n.purged)
        {
            let location = self.project_note_location(id);
            let root = location
                .project
                .map_or(ProjectBranch::Unfiled, ProjectBranch::Project);
            let count = counts.entry(root).or_default();
            count.0 += usize::from(note.done);
            count.1 += 1;
            if !note.done || self.catalog.project_show_completed {
                memos
                    .entry(location.folder.map_or(root, ProjectBranch::Folder))
                    .or_default()
                    .push(id);
            }
        }
        let roots = (0..self.data.projects.len())
            .filter(|&id| self.catalog_valid_project(Some(id)))
            .map(ProjectBranch::Project)
            .chain([ProjectBranch::Unfiled]);
        let mut stack: Vec<_> = roots.map(|branch| (branch, 0)).collect();
        stack.reverse();
        let mut rows = Vec::new();
        while let Some((branch, depth)) = stack.pop() {
            rows.push(TreeRow::Branch(
                branch,
                depth,
                counts.get(&branch).copied().unwrap_or_default(),
            ));
            if self.catalog.project_expanded.contains(&branch) {
                if let Some(ids) = memos.get(&branch) {
                    rows.extend(ids.iter().map(|&id| TreeRow::Memo(id, depth + 1)));
                }
                if let Some(folders) = children.get(&branch) {
                    stack.extend(folders.iter().rev().map(|&branch| (branch, depth + 1)));
                }
            }
        }
        rows
    }

    pub(super) fn catalog_projects(&mut self, ui: &mut egui::Ui) {
        let target = self
            .catalog
            .project_branch
            .and_then(|branch| self.project_branch_location(branch))
            .filter(|&location| self.valid_project_location(location));
        ui.horizontal_wrapped(|ui| {
            if ui.button("+ 새 메모").clicked() {
                let id = self.add_note(None);
                let location = target.unwrap_or(ProjectLocation::root(None));
                self.place_project_note(id, location);
                let root = location
                    .project
                    .map_or(ProjectBranch::Unfiled, ProjectBranch::Project);
                self.catalog.project_expanded.insert(root);
                if let Some(folder) = location.folder {
                    self.catalog
                        .project_expanded
                        .insert(ProjectBranch::Folder(folder));
                }
                self.catalog.selected = Some(id);
                self.catalog.selected_day = None;
            }
            if ui
                .add_enabled(
                    target.is_some_and(|loc| loc.project.is_some()),
                    egui::Button::new("+ 새 폴더"),
                )
                .clicked()
            {
                if let Some(target) = target {
                    self.create_project_folder(target);
                }
            }
            if ui.button("프로젝트 관리").clicked() {
                self.desktop.project_popup = true;
            }
            ui.toggle_value(&mut self.catalog.project_show_completed, "완료 메모 보기");
        });
        if self.catalog.selected.is_some_and(|id| {
            self.data.notes.get(id).is_none_or(|n| {
                !self.visible(id) || n.purged || (n.done && !self.catalog.project_show_completed)
            })
        }) {
            self.catalog.selected = None;
            self.catalog.selected_day = None;
        }
        let rows = self.project_tree_rows();
        self.catalog_selection_bar(
            ui,
            rows.iter()
                .filter_map(|row| match row {
                    TreeRow::Memo(id, _) => Some(*id),
                    _ => None,
                })
                .collect(),
        );

        let height = panel_height(ui);
        let full =
            egui::Rect::from_min_size(ui.cursor().min, Vec2::new(ui.available_width(), height));
        ui.allocate_rect(full, Sense::hover());
        let gap = 12.;
        let min = (full.width() * 0.32).clamp(100., 220.);
        let max = (full.width() - gap - min).max(min);
        let stored = self.catalog.project_preview_width;
        let preview_width = if stored > 0. {
            stored
        } else {
            full.width() * 0.62
        }
        .clamp(min, max);
        let split = full.right() - preview_width;
        let handle = egui::Rect::from_min_max(
            egui::pos2(split - gap, full.top()),
            egui::pos2(split, full.bottom()),
        );
        let response = ui
            .interact(handle, ui.id().with("project_splitter"), Sense::drag())
            .on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
            .on_hover_text("드래그해서 메모 영역 너비 조절");
        ui.painter().vline(
            handle.center().x,
            handle.y_range(),
            Stroke::new(
                1.0_f32,
                if response.hovered() || response.dragged() {
                    ACCENT
                } else {
                    ui::t(ui).line
                },
            ),
        );
        if response.dragged() {
            self.catalog.project_preview_width =
                (preview_width - response.drag_delta().x).clamp(min, max);
        }
        if response.double_clicked() {
            self.catalog.project_preview_width = 0.;
        }
        let tree_rect =
            egui::Rect::from_min_max(full.min, egui::pos2(handle.left(), full.bottom()));
        let memo_rect = egui::Rect::from_min_max(egui::pos2(split, full.top()), full.max);
        let tone = ui::t(ui);
        ui.painter().rect_filled(tree_rect, 14., tone.sidebar);
        ui.painter().rect_stroke(
            tree_rect,
            14.,
            Stroke::new(1_f32, tone.line),
            egui::StrokeKind::Inside,
        );
        // Reserve a clear header above the tree, keeping the drag and drop
        // targets inside the scroll area's actual content rectangle.
        let header = egui::Rect::from_min_max(
            tree_rect.min + Vec2::new(14., 12.),
            egui::pos2(tree_rect.right() - 14., tree_rect.top() + 48.),
        );
        ui.painter().text(
            header.left_top(),
            egui::Align2::LEFT_TOP,
            "프로젝트 구조",
            egui::FontId::proportional(14.),
            tone.ink,
        );
        ui.painter().text(
            header.left_top() + Vec2::new(0., 21.),
            egui::Align2::LEFT_TOP,
            "폴더와 메모",
            egui::FontId::proportional(11.),
            tone.muted,
        );
        let tree_content = egui::Rect::from_min_max(
            egui::pos2(tree_rect.left() + 6., tree_rect.top() + 56.),
            egui::pos2(tree_rect.right() - 6., tree_rect.bottom() - 6.),
        );
        let mut tree = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("project_tree")
                .max_rect(tree_content),
        );
        tree.set_clip_rect(tree_content.intersect(ui.clip_rect()));
        egui::ScrollArea::vertical()
            .id_salt("catalog_project_items")
            .max_height((height - 62.).max(0.))
            .auto_shrink([false, false])
            .show_rows(&mut tree, 30., rows.len(), |ui, range| {
                self.catalog_drag_scroll(ui);
                for row in &rows[range] {
                    match *row {
                        TreeRow::Branch(branch, depth, count) => {
                            self.project_tree_branch(ui, branch, depth, count)
                        }
                        TreeRow::Memo(id, depth) => self.project_tree_memo(ui, id, depth),
                    }
                }
            });
        let mut memo = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("project_preview")
                .max_rect(memo_rect),
        );
        memo.set_clip_rect(memo_rect.intersect(ui.clip_rect()));
        self.catalog_preview(&mut memo, height);
    }

    fn project_tree_branch(
        &mut self,
        ui: &mut egui::Ui,
        branch: ProjectBranch,
        depth: usize,
        (done, total): (usize, usize),
    ) {
        let Some(location) = self.project_branch_location(branch) else {
            return;
        };
        ui.push_id(branch, |ui| {
            ui.horizontal(|ui| {
                ui.add_space((depth as f32 * 16.).min(ui.available_width() * 0.5));
                if let ProjectBranch::Folder(id) = branch {
                    if self.project_tree_rename_input(ui, id) {
                        return;
                    }
                }
                let title = match branch {
                    ProjectBranch::Project(id) => {
                        format!("{}  {done}/{total}", self.data.projects[id])
                    }
                    ProjectBranch::Unfiled => format!("미분류 메모  {done}/{total}"),
                    ProjectBranch::Folder(id) => self.catalog.project_folders[id].title.clone(),
                };
                let open = self.catalog.project_expanded.contains(&branch);
                let (response, _) = ui::tree_row(
                    ui,
                    ui.id().with("branch"),
                    self.catalog.project_branch == Some(branch),
                    Some(open),
                    crate::icons::Icon::Folder,
                    &title,
                    0,
                );
                if response.double_clicked() {
                    if let ProjectBranch::Folder(id) = branch {
                        self.rename_project_folder(id);
                    }
                } else if response.clicked() {
                    self.catalog.project_branch = Some(branch);
                    if open {
                        self.catalog.project_expanded.remove(&branch);
                    } else {
                        self.catalog.project_expanded.insert(branch);
                    }
                }
                if location.project.is_some() {
                    response.context_menu(|ui| {
                        if ui.button("새 하위 폴더").clicked() {
                            self.create_project_folder(location);
                            ui.close_menu();
                        }
                        if let ProjectBranch::Folder(id) = branch {
                            if ui.button("이름 바꾸기").clicked() {
                                self.rename_project_folder(id);
                                ui.close_menu();
                            }
                        }
                    });
                }
                self.catalog_batch_drop(ui, response.rect, DropTarget::Project(location));
            });
        });
    }

    fn project_tree_rename_input(&mut self, ui: &mut egui::Ui, id: usize) -> bool {
        let Some((_, name, request_focus)) = self
            .catalog
            .project_folder_rename
            .as_mut()
            .filter(|(editing, _, _)| *editing == id)
        else {
            return false;
        };
        let response = ui.add_sized(
            [ui.available_width(), 30.],
            egui::TextEdit::singleline(name).hint_text("폴더 이름"),
        );
        let just_opened = std::mem::take(request_focus);
        if just_opened {
            response.request_focus();
        }
        let cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if cancel
            || (!just_opened
                && (response.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter))))
        {
            if !cancel && !name.trim().is_empty() {
                self.catalog.project_folders[id].title = name.trim().into();
                self.saving.request();
            }
            self.catalog.project_folder_rename = None;
        }
        true
    }

    fn project_tree_memo(&mut self, ui: &mut egui::Ui, id: usize, depth: usize) {
        let note = &self.data.notes[id];
        let (mut done, day) = (note.done, note.date);
        let selecting = self.catalog_selecting();
        let selected = if selecting {
            self.catalog.selection.items.contains(&id)
        } else {
            self.catalog.selected == Some(id)
        };
        let mut title = RichText::new(&note.title);
        if done {
            title = title.strikethrough().color(MUTED);
        } else if selected {
            title = title.color(ACCENT);
        }
        let progress = project_check_progress(note);
        ui.push_id(("project_memo", id), |ui| {
            ui.horizontal(|ui| {
                ui.add_space((depth as f32 * 16.).min(ui.available_width() * 0.5));
                if self
                    .catalog
                    .selection
                    .checkbox(ui, id, &self.catalog.selection_order)
                {
                    ui.memory_mut(|m| m.request_focus(catalog_list_focus()));
                }
                if ui
                    .add_enabled(!selecting, |ui: &mut egui::Ui| {
                        ui::round_check(ui, &mut done, None)
                    })
                    .on_hover_text("메모 완료 상태")
                    .changed()
                {
                    self.done_at(id, None, done);
                }
                let counter_width = if progress.is_some() { 48. } else { 0. };
                let r = ui.add_sized(
                    [(ui.available_width() - counter_width).max(1.), 30.],
                    egui::Label::new(title)
                        .truncate()
                        .sense(Sense::click_and_drag()),
                );
                if selected {
                    ui.painter().rect_stroke(
                        r.rect,
                        4.,
                        Stroke::new(1.0_f32, ui::t(ui).line),
                        egui::StrokeKind::Inside,
                    );
                }
                if let Some((checked, total)) = progress {
                    ui.label(
                        RichText::new(format!("{checked}/{total}"))
                            .small()
                            .color(MUTED),
                    )
                    .on_hover_text("메모 안의 체크박스 진행률");
                }
                self.catalog_note_response(ui, &r, id, day, false);
                if !self.catalog_selecting() {
                    if r.clicked() {
                        let loc = self.project_note_location(id);
                        self.catalog.project_branch = Some(
                            loc.folder.map_or(
                                loc.project
                                    .map_or(ProjectBranch::Unfiled, ProjectBranch::Project),
                                ProjectBranch::Folder,
                            ),
                        );
                    }
                    r.context_menu(|ui| {
                        if ui.button("독립 편집창 열기").clicked() {
                            self.open_note(id);
                            ui.close_menu();
                        }
                        if ui.button("휴지통으로 이동").clicked() {
                            self.catalog_batch_action(vec![id], BatchAction::Trash);
                            ui.close_menu();
                        }
                    });
                }
            });
        });
    }
}

fn project_check_progress(note: &Note) -> Option<(usize, usize)> {
    if note.locked {
        return None;
    }
    let (checked, total) = std::iter::once(note.body.as_str())
        .chain(
            note.document
                .tabs
                .iter()
                .skip(1)
                .map(|tab| tab.body.as_str()),
        )
        .filter_map(crate::model::check_progress)
        .fold((0, 0), |(checked, total), (d, n)| (checked + d, total + n));
    (total > 0).then_some((checked, total))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        App::sample(NaiveDate::from_ymd_opt(2026, 11, 24).unwrap())
    }

    #[test]
    fn project_tree_reveals_nested_memos_and_hides_completed_until_enabled() {
        let mut a = app();
        let root = ProjectBranch::Project(0);
        let loc = ProjectLocation::root(Some(0));
        a.place_project_note(0, loc);
        a.create_project_folder(loc);
        assert!(a.catalog.project_expanded.contains(&root));
        let folder = a.project_branch_location(ProjectBranch::Folder(0)).unwrap();
        a.create_project_folder(folder);
        let child = a.project_branch_location(ProjectBranch::Folder(1)).unwrap();
        a.catalog.project_folders[1].title = "하위 업무".into();
        a.catalog.project_expanded.insert(ProjectBranch::Folder(1));
        a.place_project_note(1, child);
        a.data.notes[0].done = true;
        a.data.notes[1].done = false;
        let visible = a.project_tree_rows();
        assert!(visible.iter().any(|row| matches!(row, TreeRow::Memo(1, 3))));
        assert!(!visible.iter().any(|row| matches!(row, TreeRow::Memo(0, _))));
        a.catalog.project_show_completed = true;
        assert!(a
            .project_tree_rows()
            .iter()
            .any(|row| matches!(row, TreeRow::Memo(0, 1))));
        a.catalog.project_expanded.remove(&ProjectBranch::Folder(0));
        assert!(!a
            .project_tree_rows()
            .iter()
            .any(|row| matches!(row, TreeRow::Memo(1, _))));
    }

    #[test]
    fn project_folders_roundtrip_and_batch_undo_restores_previous_folder() {
        let mut a = app();
        let root = ProjectLocation::root(Some(0));
        a.create_project_folder(root);
        let loc = a.project_branch_location(ProjectBranch::Folder(0)).unwrap();
        a.place_project_note(0, loc);
        a.catalog_move_to_project(&[0], root);
        assert_eq!(a.project_note_location(0), root);
        a.catalog_history(false);
        assert_eq!(a.project_note_location(0), loc);
        let generic_folder = a.catalog.create_folder();
        a.catalog.assign_notes(&[0], Some(generic_folder));
        let path = std::env::temp_dir().join("ohmemojang-project-folders-record");
        let persisted = a.snapshot(&path);
        let mut restored = app();
        restored.restore(&persisted);
        assert_eq!(restored.project_note_location(0), loc);
        assert_eq!(restored.catalog.assignments.get(&0), Some(&generic_folder));
        assert_eq!(restored.catalog.project_folders, a.catalog.project_folders);
    }

    #[test]
    fn checklist_totals_include_active_body_tabs_and_respect_locked_memos() {
        let mut note = Note::new("체크 메모", None, 0);
        note.body = "☑ 검토\n☐ 대기".into();
        note.document.add_tab();
        note.document.tabs[1].body = "☑ 제출".into();
        assert_eq!(project_check_progress(&note), Some((2, 3)));
        note.locked = true;
        assert_eq!(project_check_progress(&note), None);
    }
}
