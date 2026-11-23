use super::projects::ProjectLocation;
use super::*;

#[derive(
    Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
pub(crate) enum OrderScope {
    All,
    Folder(Option<usize>),
}

#[derive(Clone, Copy)]
pub(super) enum DropTarget {
    Project(ProjectLocation),
    Order(OrderScope, usize, bool),
}

pub(super) enum Edit {
    Folder(Vec<(usize, Option<usize>, Option<usize>)>),
    Project(Vec<(usize, ProjectLocation, ProjectLocation)>),
    Trash(Vec<usize>),
    Order(OrderScope, Vec<usize>, Vec<usize>),
}

#[derive(Clone, Copy)]
pub(super) enum Action {
    Folder(Option<usize>),
    Project(Option<usize>),
    Complete(bool),
    Trash,
}

impl App {
    pub(super) fn catalog_project_visibility_key(&self) -> u64 {
        let hidden: Vec<_> = self.extra.hidden_projects.iter().copied().collect();
        let deleted: Vec<_> = self.extra.deleted_projects.iter().copied().collect();
        crate::lists::key((hidden, deleted))
    }
    pub(super) fn catalog_batch_ids(&self) -> Vec<usize> {
        self.catalog
            .selection_order
            .iter()
            .copied()
            .filter(|id| self.catalog.selection.items.contains(id))
            .collect()
    }

    pub(super) fn catalog_batch_menu(&self, ui: &mut egui::Ui) -> Option<Action> {
        let mut action = None;
        if ui.button("휴지통으로 이동").clicked() {
            action = Some(Action::Trash);
        }
        if ui.button("완료").clicked() {
            action = Some(Action::Complete(true));
        }
        if ui.button("미완료").clicked() {
            action = Some(Action::Complete(false));
        }
        ui.menu_button("폴더 이동", |ui| {
            if ui.button("폴더 분류 해제").clicked() {
                action = Some(Action::Folder(None));
            }
            for (id, folder) in self.catalog.folders.iter().enumerate() {
                if !folder.deleted && ui.button(self.catalog_folder_path(id)).clicked() {
                    action = Some(Action::Folder(Some(id)));
                }
            }
        });
        ui.menu_button("프로젝트 지정", |ui| {
            if ui.button("미분류로 이동").clicked() {
                action = Some(Action::Project(None));
            }
            for (id, name) in self.data.projects.iter().enumerate() {
                if self.catalog_valid_project(Some(id)) && ui.button(name).clicked() {
                    action = Some(Action::Project(Some(id)));
                }
            }
        });
        if action.is_some() {
            ui.close_menu();
        }
        action
    }

    pub(super) fn catalog_folder_path(&self, id: usize) -> String {
        let mut names = Vec::new();
        let mut next = Some(id);
        while let Some(folder) = next.and_then(|id| self.catalog.folders.get(id)) {
            if names.len() >= self.catalog.folders.len() {
                break;
            }
            names.push(folder.title.as_str());
            next = folder.parent;
        }
        names.reverse();
        names.join(" / ")
    }

    fn catalog_valid_folder(&self, id: Option<usize>) -> bool {
        id.is_none_or(|id| {
            self.catalog
                .folders
                .get(id)
                .is_some_and(|folder| !folder.deleted)
        })
    }

    pub(super) fn catalog_valid_project(&self, id: Option<usize>) -> bool {
        id.is_none_or(|id| {
            id < self.data.projects.len()
                && !self.extra.hidden_projects.contains(&id)
                && !self.extra.deleted_projects.contains(&id)
        })
    }

    pub(super) fn catalog_batch_action(&mut self, ids: Vec<usize>, action: Action) {
        if ids.is_empty() || !ids.iter().all(|&id| self.catalog_actionable(id)) {
            return;
        }
        let edit = match action {
            Action::Folder(target) if self.catalog_valid_folder(target) => Edit::Folder(
                ids.into_iter()
                    .filter_map(|id| {
                        let old = self.catalog.assignments.get(&id).copied();
                        (old != target).then_some((id, old, target))
                    })
                    .collect(),
            ),
            Action::Project(target) => {
                self.catalog_move_to_project(&ids, ProjectLocation::root(target));
                return;
            }
            Action::Trash => Edit::Trash(ids),
            Action::Complete(value) => {
                if ids.iter().any(|&id| {
                    self.data.notes[id].duration > 1
                        && self.data.notes[id].date.is_some()
                        && !self.data.notes[id].deadline
                }) {
                    self.catalog.batch_completion = Some((ids, value));
                } else {
                    self.catalog_complete_ids(&ids, value, false);
                }
                return;
            }
            _ => return,
        };
        self.catalog_apply_new(edit);
    }

    fn catalog_complete_ids(&mut self, ids: &[usize], value: bool, future: bool) {
        if !ids.iter().all(|&id| self.catalog_actionable(id)) {
            self.toast = "메모 상태가 바뀌었습니다. 대상을 다시 선택하세요".into();
            return;
        }
        let now = Local::now();
        for &id in ids {
            if let Some(day) = self.completion_day(id, self.data.notes[id].date) {
                self.complete_span(id, day, value, future, now.naive_local());
            } else {
                self.done(id, value);
            }
        }
        self.toast = format!(
            "메모 {}개를 {} 처리했습니다",
            ids.len(),
            if value { "완료" } else { "미완료" }
        );
        self.catalog_changed();
    }

    pub(super) fn catalog_completion_dialog(&mut self, ctx: &egui::Context) {
        let Some((ids, value)) = self.catalog.batch_completion.as_ref() else {
            return;
        };
        let count = ids.len();
        let value = *value;
        let mut open = true;
        let mut choice = None;
        let mut cancel = false;
        ui::dialog(
            ctx,
            "catalog_batch_completion",
            "일괄 완료 범위",
            [440., 180.],
            true,
            &mut open,
            |ui| {
                ui.label(format!(
                    "메모 {count}개 중 기간 일정의 완료 범위를 선택하세요."
                ));
                ui.horizontal(|ui| {
                    if ui.button("기준 회차만").clicked() {
                        choice = Some(false);
                    }
                    if ui.button("기준 회차 이후 모두").clicked() {
                        choice = Some(true);
                    }
                    if ui.button("취소").clicked() {
                        cancel = true;
                    }
                });
            },
        );
        if let Some(future) = choice {
            if let Some((ids, _)) = self.catalog.batch_completion.take() {
                self.catalog_complete_ids(&ids, value, future);
            }
        } else if cancel || !open {
            self.catalog.batch_completion = None;
        }
    }

    fn catalog_actionable(&self, id: usize) -> bool {
        self.data
            .notes
            .get(id)
            .is_some_and(|note| !note.purged && self.visible(id))
    }

    pub(super) fn catalog_move_to_project(&mut self, ids: &[usize], target: ProjectLocation) {
        if !self.valid_project_location(target)
            || !ids.iter().all(|&id| self.catalog_actionable(id))
        {
            return;
        }
        let changes = ids
            .iter()
            .filter_map(|&id| {
                let old = self.project_note_location(id);
                (old != target).then_some((id, old, target))
            })
            .collect();
        self.catalog_apply_new(Edit::Project(changes));
    }

    pub(super) fn catalog_ordered(&self, scope: OrderScope, ids: &mut [usize]) {
        if let Some(order) = self.catalog.manual_order.get(&scope) {
            let rank: HashMap<_, _> = order.iter().enumerate().map(|(at, id)| (*id, at)).collect();
            ids.sort_by_key(|id| rank.get(id).copied().unwrap_or(usize::MAX));
        }
    }

    pub(super) fn catalog_reorder(
        &mut self,
        scope: OrderScope,
        anchor: usize,
        after: bool,
        ids: &[usize],
    ) {
        if ids.contains(&anchor) || !ids.iter().all(|&id| self.catalog_actionable(id)) {
            return;
        }
        let mut original: Vec<_> = self
            .data
            .notes
            .iter()
            .enumerate()
            .filter(|(id, note)| {
                !note.purged
                    && match scope {
                        OrderScope::All => true,
                        OrderScope::Folder(folder) => {
                            self.catalog.assignments.get(id).copied() == folder
                        }
                    }
            })
            .map(|(id, _)| id)
            .collect();
        if scope == OrderScope::All {
            original.sort_by_key(|&id| (self.data.notes[id].created, id));
        }
        self.catalog_ordered(scope, &mut original);
        let moving: HashSet<_> = ids.iter().copied().collect();
        let mut ordered: Vec<_> = original
            .iter()
            .copied()
            .filter(|id| !moving.contains(id))
            .collect();
        if original.len() - ordered.len() != moving.len() {
            return;
        }
        let Some(at) = ordered.iter().position(|&id| id == anchor) else {
            return;
        };
        ordered.splice(
            at + usize::from(after)..at + usize::from(after),
            ids.iter().copied(),
        );
        if original != ordered {
            let previous = self
                .catalog
                .manual_order
                .get(&scope)
                .cloned()
                .unwrap_or_default();
            self.catalog_apply_new(Edit::Order(scope, previous, ordered));
        }
    }

    fn catalog_apply_new(&mut self, edit: Edit) {
        if self.catalog_apply_edit(&edit, true) {
            self.catalog.undo.push(edit);
            if self.catalog.undo.len() > 30 {
                self.catalog.undo.remove(0);
            }
            self.catalog.redo.clear();
        }
    }

    pub(super) fn catalog_history(&mut self, forward: bool) {
        let edit = if forward {
            self.catalog.redo.pop()
        } else {
            self.catalog.undo.pop()
        };
        if let Some(edit) = edit {
            if self.catalog_apply_edit(&edit, forward) {
                if forward {
                    self.catalog.undo.push(edit);
                } else {
                    self.catalog.redo.push(edit);
                }
            } else {
                self.toast = "메모나 대상 위치가 바뀌어 작업을 되돌릴 수 없습니다".into();
            }
        }
    }

    fn catalog_apply_edit(&mut self, edit: &Edit, forward: bool) -> bool {
        fn endpoints<T>(forward: bool, old: T, new: T) -> (T, T) {
            if forward {
                (old, new)
            } else {
                (new, old)
            }
        }
        let (count, verb) = match edit {
            Edit::Folder(changes) => {
                if changes.is_empty()
                    || !changes.iter().all(|&(id, old, new)| {
                        let (from, to) = endpoints(forward, old, new);
                        self.catalog_actionable(id)
                            && self.catalog.assignments.get(&id).copied() == from
                            && self.catalog_valid_folder(to)
                    })
                {
                    return false;
                }
                for &(id, old, new) in changes {
                    let (_, to) = endpoints(forward, old, new);
                    self.catalog.assign_notes(&[id], to);
                }
                (changes.len(), "폴더 위치 변경")
            }
            Edit::Project(changes) => {
                if changes.is_empty()
                    || !changes.iter().all(|&(id, old, new)| {
                        let (from, to) = endpoints(forward, old, new);
                        self.catalog_actionable(id)
                            && self.project_note_location(id) == from
                            && self.valid_project_location(to)
                    })
                {
                    return false;
                }
                for &(id, old, new) in changes {
                    self.place_project_note(id, endpoints(forward, old, new).1);
                }
                (changes.len(), "프로젝트 위치 변경")
            }
            Edit::Trash(ids) => {
                if !ids.iter().all(|&id| {
                    self.data
                        .notes
                        .get(id)
                        .is_some_and(|note| !note.purged && note.deleted != forward)
                }) {
                    return false;
                }
                for &id in ids {
                    if forward {
                        self.trash_note(id);
                    } else {
                        self.restore_note(id);
                    }
                }
                (
                    ids.len(),
                    if forward {
                        "휴지통 이동"
                    } else {
                        "휴지통 이동 취소"
                    },
                )
            }
            Edit::Order(scope, old, new) => {
                let (from, to) = if forward { (old, new) } else { (new, old) };
                if self
                    .catalog
                    .manual_order
                    .get(scope)
                    .map(Vec::as_slice)
                    .unwrap_or_default()
                    != from
                    || matches!(scope, OrderScope::Folder(id) if !self.catalog_valid_folder(*id))
                {
                    return false;
                }
                self.catalog.manual_order.insert(*scope, to.clone());
                self.catalog.order_revision = self.catalog.order_revision.wrapping_add(1);
                (0, "메모 순서 변경")
            }
        };
        self.toast = if count == 0 {
            verb.into()
        } else {
            format!("메모 {count}개 · {verb}")
        };
        self.catalog_changed();
        true
    }

    fn catalog_changed(&mut self) {
        crate::lists::CHANGES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.saving.request();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        App::sample(NaiveDate::from_ymd_opt(2026, 11, 23).unwrap())
    }

    #[test]
    fn dropping_several_memos_moves_only_folder_and_can_be_undone() {
        let mut app = app();
        let first_project = app.data.notes[0].project;
        let first_date = app.data.notes[0].date;
        let folder = app.catalog.create_folder();
        app.catalog_batch_action(vec![1, 0], Action::Folder(Some(folder)));
        assert_eq!(app.catalog.assignments.get(&0), Some(&folder));
        assert_eq!(app.catalog.assignments.get(&1), Some(&folder));
        assert_eq!(app.data.notes[0].project, first_project);
        assert_eq!(app.data.notes[0].date, first_date);
        app.catalog_history(false);
        assert!(app.catalog.assignments.is_empty());
        app.catalog_history(true);
        assert_eq!(app.catalog.assignments.get(&1), Some(&folder));
    }

    #[test]
    fn manual_reorder_keeps_hidden_rows_and_group_order() {
        let mut app = app();
        app.catalog
            .manual_order
            .insert(OrderScope::All, vec![0, 1, 2, 3]);
        let note = Note::new("추가 메모", None, 3);
        while app.data.notes.len() < 4 {
            app.data.notes.push(note.clone());
        }
        app.catalog_reorder(OrderScope::All, 0, false, &[3, 2]);
        assert_eq!(
            &app.catalog.manual_order[&OrderScope::All][..4],
            &[3, 2, 0, 1]
        );
        app.catalog_history(false);
        assert_eq!(&app.catalog.manual_order[&OrderScope::All], &[0, 1, 2, 3]);
    }

    #[test]
    fn project_reassignment_preserves_folder_and_invalidates_stale_undo() {
        let mut app = app();
        let folder = app.catalog.create_folder();
        let project = app.data.projects.len();
        app.data.projects.push("새 프로젝트".into());
        app.catalog.assign_notes(&[0, 1], Some(folder));
        app.catalog_batch_action(vec![0, 1], Action::Project(Some(project)));
        assert_eq!(app.data.notes[0].project, Some(project));
        assert_eq!(app.data.notes[1].project, Some(project));
        assert_eq!(app.catalog.assignments.get(&0), Some(&folder));
        app.data.notes[0].project = None;
        app.catalog_history(false);
        assert_eq!(app.data.notes[1].project, Some(project));
    }

    #[test]
    fn trash_reuses_retention_and_restore() {
        let mut app = app();
        app.catalog_batch_action(vec![0, 1], Action::Trash);
        assert!(app.data.notes[1].deleted);
        assert!(app.desktop.deleted_at.contains_key(&0));
        app.catalog_history(false);
        assert!(!app.data.notes[1].deleted);
        assert!(!app.desktop.deleted_at.contains_key(&0));
    }
}
