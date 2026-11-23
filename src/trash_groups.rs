//! Recycle-bin groups. Deleting a wiki tab or a catalog folder together with
//! its memos sends them to the bin as one "이름/" row that opens like a folder.
//!
//! Nothing extra is stored: a group is derived from what is already saved —
//! a deleted wiki tab (`deleted_wiki_tabs`) or a deleted catalog folder, plus
//! the deleted memos that belong to it. So memos deleted one by one before the
//! tab was deleted join the same group, and restoring a single memo simply
//! brings its tab or folder back.
use crate::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Group {
    /// A deleted wiki tab.
    Wiki(usize),
    /// A deleted catalog folder (the top one of a deleted branch).
    Folder(usize),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TrashItem {
    Memo(usize),
    Tab(usize, u64),
    Group(Group),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PurgeKind {
    Memo,
    Tab,
    Selection,
    Empty,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashTargets {
    memos: Vec<(usize, bool, Option<chrono::NaiveDateTime>)>,
    tabs: Vec<(usize, u64)>,
}

pub struct PurgeRequest {
    pub kind: PurgeKind,
    pub targets: TrashTargets,
    pub opened: bool,
}

impl TrashTargets {
    pub fn counts(&self) -> (usize, usize) {
        (self.memos.len(), self.tabs.len())
    }

    fn is_empty(&self) -> bool {
        self.memos.is_empty() && self.tabs.is_empty()
    }
}

impl App {
    fn in_bin(&self, i: usize) -> bool {
        self.data
            .notes
            .get(i)
            .is_some_and(|n| n.deleted && !n.purged)
    }

    /// The wiki tab a memo belongs to, through its folder when it has one.
    fn wiki_tab_of(&self, i: usize) -> Option<usize> {
        let n = &self.data.notes[i];
        if !n.wiki {
            return None;
        }
        match n.folder.and_then(|f| self.extra.folders.get(f)) {
            Some(f) => Some(f.tab),
            None => n.wiki_tab,
        }
    }

    fn trash_targets(
        &self,
        items: impl IntoIterator<Item = TrashItem>,
        include_live_memo: bool,
        purge: bool,
    ) -> TrashTargets {
        let mut memo_ids = BTreeSet::new();
        let mut tabs = BTreeSet::new();
        for item in items {
            match item {
                TrashItem::Memo(i)
                    if self
                        .data
                        .notes
                        .get(i)
                        .is_some_and(|n| !n.purged && (n.deleted || include_live_memo)) =>
                {
                    memo_ids.insert(i);
                }
                TrashItem::Tab(i, id)
                    if self.data.notes.get(i).is_some_and(|n| {
                        !n.purged && n.document.trash.iter().any(|tab| tab.id == id)
                    }) =>
                {
                    tabs.insert((i, id));
                }
                _ => {}
            }
        }
        if purge {
            for &i in &memo_ids {
                tabs.extend(
                    self.data.notes[i]
                        .document
                        .trash
                        .iter()
                        .map(|tab| (i, tab.id)),
                );
            }
        }
        let memos = memo_ids
            .into_iter()
            .map(|i| {
                (
                    i,
                    self.data.notes[i].deleted,
                    self.desktop.deleted_at.get(&i).copied(),
                )
            })
            .collect();
        TrashTargets {
            memos,
            tabs: tabs.into_iter().collect(),
        }
    }

    pub fn all_trash_items(&self) -> impl Iterator<Item = TrashItem> + '_ {
        self.data
            .notes
            .iter()
            .enumerate()
            .filter(|(_, n)| !n.purged)
            .flat_map(|(i, n)| {
                n.deleted.then_some(TrashItem::Memo(i)).into_iter().chain(
                    n.document
                        .trash
                        .iter()
                        .map(move |tab| TrashItem::Tab(i, tab.id)),
                )
            })
    }

    fn selected_trash_items(&self) -> (Vec<TrashItem>, Vec<Group>) {
        let mut items = Vec::new();
        let mut groups = Vec::new();
        for &item in &self.desktop.trash_selection.items {
            let TrashItem::Group(group) = item else {
                items.push(item);
                continue;
            };
            groups.push(group);
            if let Some(memos) = self.desktop.trash_selected_groups.get(&group) {
                items.extend(memos.iter().copied().map(TrashItem::Memo));
            }
        }
        (items, groups)
    }

    pub fn request_trash_purge(&mut self, kind: PurgeKind, single: Option<TrashItem>) {
        if self.desktop.purge_target.is_some() || self.desktop.purge_save.is_some() {
            return;
        }
        let targets = match kind {
            PurgeKind::Empty => self.trash_targets(self.all_trash_items(), false, true),
            PurgeKind::Selection => self.trash_targets(self.selected_trash_items().0, false, true),
            PurgeKind::Memo | PurgeKind::Tab => {
                self.trash_targets(single, kind == PurgeKind::Memo, true)
            }
        };
        if targets.is_empty() {
            return;
        }
        self.desktop.purge_target = Some(PurgeRequest {
            kind,
            targets,
            opened: false,
        });
        self.desktop.focus_dialog = true;
    }

    pub fn trash_purge_counts(&self, targets: &TrashTargets) -> String {
        let (memos, tabs) = targets.counts();
        match (memos, tabs) {
            (0, tabs) => format!("본문 탭 {tabs}개"),
            (memos, 0) => format!("메모 {memos}개"),
            (memos, tabs) => format!("메모 {memos}개와 본문 탭 {tabs}개"),
        }
    }

    pub fn trash_purge_title(&self, request: &PurgeRequest) -> String {
        match request.kind {
            PurgeKind::Memo => self
                .data
                .notes
                .get(request.targets.memos[0].0)
                .map(|n| n.title.clone())
                .unwrap_or_default(),
            PurgeKind::Tab => {
                let (i, id) = request.targets.tabs[0];
                self.data
                    .notes
                    .get(i)
                    .and_then(|n| {
                        n.document
                            .trash
                            .iter()
                            .find(|tab| tab.id == id)
                            .map(|tab| format!("{} - {}", n.title, tab.title))
                    })
                    .unwrap_or_default()
            }
            _ => self.trash_purge_counts(&request.targets),
        }
    }

    pub fn confirm_trash_purge(&mut self, request: PurgeRequest) -> Option<String> {
        let current = if request.kind == PurgeKind::Empty {
            self.trash_targets(self.all_trash_items(), false, true)
        } else {
            let items = request
                .targets
                .memos
                .iter()
                .map(|&(i, _, _)| TrashItem::Memo(i))
                .chain(
                    request
                        .targets
                        .tabs
                        .iter()
                        .map(|&(i, id)| TrashItem::Tab(i, id)),
                );
            self.trash_targets(items, request.kind == PurgeKind::Memo, true)
        };
        if current != request.targets {
            self.toast = "확인 중에 휴지통이 변경되었습니다. 대상을 다시 확인해 주세요.".into();
            return None;
        }
        let description = self.trash_purge_counts(&current);
        let memos = current.memos.into_iter().map(|(i, _, _)| i).collect();
        self.purge_notes(&memos);
        let mut tabs = BTreeMap::<usize, BTreeSet<_>>::new();
        for (i, id) in current.tabs {
            if !self.data.notes[i].purged {
                tabs.entry(i).or_default().insert(id);
            }
        }
        for (i, ids) in tabs {
            self.data.notes[i]
                .document
                .trash
                .retain(|tab| !ids.contains(&tab.id));
        }
        self.desktop.trash_selection.clear();
        self.desktop.trash_selected_groups.clear();
        crate::lists::CHANGES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Some(format!("{description} 영구삭제를 완료했습니다"))
    }

    pub fn restore_trash_items(&mut self) {
        let (items, groups) = self.selected_trash_items();
        let targets = self.trash_targets(items, false, false);
        if targets.is_empty() {
            return;
        }
        for group in groups {
            self.revive_group_containers(group);
        }
        let mut restored_memos: BTreeSet<_> = targets.memos.iter().map(|&(i, _, _)| i).collect();
        let mut tabs = BTreeMap::<usize, BTreeSet<_>>::new();
        for &(i, id) in &targets.tabs {
            tabs.entry(i).or_default().insert(id);
        }
        for (i, ids) in tabs {
            if self.in_bin(i) {
                restored_memos.insert(i);
            }
            self.restore_trash_tabs(i, &ids);
        }
        for i in restored_memos {
            self.restore_note(i);
        }
        self.desktop.trash_selection.clear();
        self.desktop.trash_selected_groups.clear();
        self.toast = format!("{} 복원을 완료했습니다", self.trash_purge_counts(&targets));
    }

    pub fn restore_trash_tabs(&mut self, i: usize, ids: &BTreeSet<u64>) -> bool {
        let Some(n) = self.data.notes.get_mut(i).filter(|n| !n.purged) else {
            return false;
        };
        let doc = &mut n.document;
        let before = doc.tabs.len();
        let deleted = std::mem::take(&mut doc.trash);
        for mut tab in deleted {
            if ids.contains(&tab.id) {
                tab.hidden = false;
                doc.tabs.push(tab);
            } else {
                doc.trash.push(tab);
            }
        }
        let restored = doc.tabs.len() > before;
        if restored {
            doc.active = doc.tabs.len() - 1;
            self.restore_note(i);
        }
        restored
    }

    /// The folder and the deleted folders below it, reached through deleted
    /// folders only.
    fn deleted_branch(&self, f: usize) -> Vec<usize> {
        let folders = &self.catalog.folders;
        let mut branch = vec![f];
        let mut at = 0;
        while at < branch.len() {
            let parent = branch[at];
            for (c, folder) in folders.iter().enumerate() {
                if folder.deleted && folder.parent == Some(parent) && !branch.contains(&c) {
                    branch.push(c);
                }
            }
            at += 1;
        }
        branch
    }

    /// The memos in the bin that belong to `g`.
    pub fn group_members(&self, g: Group) -> Vec<usize> {
        match g {
            Group::Wiki(t) => (0..self.data.notes.len())
                .filter(|&i| self.in_bin(i) && self.wiki_tab_of(i) == Some(t))
                .collect(),
            Group::Folder(f) => {
                let branch = self.deleted_branch(f);
                (0..self.data.notes.len())
                    .filter(|&i| {
                        self.in_bin(i)
                            && self
                                .catalog
                                .assignments
                                .get(&i)
                                .is_some_and(|a| branch.contains(a))
                    })
                    .collect()
            }
        }
    }

    /// Groups to show in the bin: deleted tabs and folders that still have
    /// memos in the bin.
    #[cfg(test)]
    fn trash_groups(&self) -> Vec<Group> {
        let (groups, _) = self.trash_grouped_members();
        groups
    }

    pub fn trash_grouped_members(&self) -> (Vec<Group>, HashMap<Group, Vec<usize>>) {
        let folders = &self.catalog.folders;
        let mut roots = vec![None; folders.len()];
        for (f, folder) in folders
            .iter()
            .enumerate()
            .filter(|(_, folder)| folder.deleted)
        {
            let mut root = f;
            let mut parent = folder.parent;
            for _ in 0..folders.len() {
                let Some(p) = parent.filter(|&p| folders.get(p).is_some_and(|x| x.deleted)) else {
                    roots[f] = Some(root);
                    break;
                };
                root = p;
                parent = folders[p].parent;
            }
        }
        let mut members = HashMap::<Group, Vec<_>>::new();
        for (i, _) in self
            .data
            .notes
            .iter()
            .enumerate()
            .filter(|(i, _)| self.in_bin(*i))
        {
            if let Some(t) = self
                .wiki_tab_of(i)
                .filter(|t| self.extra.deleted_wiki_tabs.contains(t))
            {
                members.entry(Group::Wiki(t)).or_default().push(i);
            }
            if let Some(root) = self
                .catalog
                .assignments
                .get(&i)
                .and_then(|&f| roots.get(f))
                .copied()
                .flatten()
            {
                members.entry(Group::Folder(root)).or_default().push(i);
            }
        }
        let groups = (0..self.extra.wiki_tabs.len())
            .map(Group::Wiki)
            .chain((0..folders.len()).map(Group::Folder))
            .filter(|g| members.contains_key(g))
            .collect();
        (groups, members)
    }

    pub fn group_name(&self, g: Group) -> String {
        match g {
            Group::Wiki(t) => self.extra.wiki_tabs.get(t).cloned().unwrap_or_default(),
            Group::Folder(f) => self
                .catalog
                .folders
                .get(f)
                .map(|f| f.title.clone())
                .unwrap_or_default(),
        }
    }

    /// How a memo is named inside its group: below a folder's own level the
    /// path of subfolders comes first ("하위/메모").
    pub fn group_member_title(&self, g: Group, i: usize) -> String {
        let title = self.data.notes[i].title.clone();
        let Group::Folder(top) = g else {
            return title;
        };
        let mut path = vec![];
        let mut at = self.catalog.assignments.get(&i).copied();
        while let Some(f) = at.filter(|&f| f != top) {
            let Some(folder) = self.catalog.folders.get(f) else {
                break;
            };
            path.push(folder.title.clone());
            at = folder.parent;
        }
        path.reverse();
        path.push(title);
        path.join("/")
    }

    /// "탭이름/메모제목" for a memo in the bin that belonged to a wiki tab which
    /// is still there.
    pub fn bin_prefix(&self, i: usize) -> Option<String> {
        let t = self.wiki_tab_of(i)?;
        if self.extra.deleted_wiki_tabs.contains(&t) {
            return None;
        }
        let name = self.extra.wiki_tabs.get(t)?;
        Some(format!("{name}/{}", self.data.notes[i].title))
    }

    fn send_to_bin(&mut self, i: usize) {
        self.trash_note(i);
    }

    /// Delete a wiki tab together with every memo in it (also those in its
    /// folders). The tab and its memos form one group in the bin.
    pub fn trash_wiki_tab(&mut self, t: usize) {
        let members: Vec<usize> = (0..self.data.notes.len())
            .filter(|&i| self.wiki_tab_of(i) == Some(t))
            .collect();
        let count = members.len();
        for i in members {
            self.send_to_bin(i);
        }
        self.extra.deleted_wiki_tabs.insert(t);
        let name = self.group_name(Group::Wiki(t));
        self.toast = format!("‘{name}’ 탭과 메모 {count}개를 휴지통으로 옮겼습니다");
    }

    /// Delete a catalog folder, its subfolders and all their memos.
    pub fn trash_catalog_folder_with_notes(&mut self, f: usize) {
        let mut branch = vec![f];
        let mut at = 0;
        while at < branch.len() {
            let parent = branch[at];
            for (c, folder) in self.catalog.folders.iter().enumerate() {
                if !folder.deleted && folder.parent == Some(parent) && !branch.contains(&c) {
                    branch.push(c);
                }
            }
            at += 1;
        }
        let members: Vec<usize> = self
            .catalog
            .assignments
            .iter()
            .filter(|(_, a)| branch.contains(a))
            .map(|(&i, _)| i)
            .filter(|&i| i < self.data.notes.len())
            .collect();
        let count = members
            .iter()
            .filter(|&&i| !self.data.notes[i].deleted)
            .count();
        for i in members {
            self.send_to_bin(i);
        }
        for &b in &branch {
            self.catalog.folders[b].deleted = true;
        }
        if self
            .catalog
            .folder_selected
            .is_some_and(|s| branch.contains(&s))
        {
            self.catalog.folder_selected = None;
        }
        let name = self.catalog.folders[f].title.clone();
        self.toast = format!("‘{name}’ 폴더와 메모 {count}개를 휴지통으로 옮겼습니다");
    }

    /// Bring back the tab or folders a memo lives in, so a restored memo is
    /// never left in something that is gone.
    pub fn revive_containers(&mut self, i: usize) {
        if let Some(t) = self.wiki_tab_of(i) {
            self.extra.deleted_wiki_tabs.remove(&t);
        }
        let mut at = self.catalog.assignments.get(&i).copied();
        let mut seen = HashSet::new();
        while let Some(f) = at {
            let Some(folder) = self.catalog.folders.get_mut(f) else {
                break;
            };
            if !seen.insert(f) {
                break;
            }
            folder.deleted = false;
            at = folder.parent;
        }
    }

    /// Restore a whole group: the tab or folder and every memo in it.
    pub fn restore_group(&mut self, g: Group) {
        let members = self.group_members(g);
        let count = members.len();
        self.revive_group_containers(g);
        for i in members {
            self.restore_note(i);
        }
        let name = self.group_name(g);
        self.toast = format!("‘{name}/’ 와 메모 {count}개를 복구했습니다");
    }

    fn revive_group_containers(&mut self, g: Group) {
        match g {
            Group::Wiki(t) => {
                self.extra.deleted_wiki_tabs.remove(&t);
            }
            Group::Folder(f) => {
                for b in self.deleted_branch(f) {
                    self.catalog.folders[b].deleted = false;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restoring_a_batch_preserves_tab_order_and_revives_its_memo() {
        let mut a = app();
        let mut tabs = Vec::new();
        for name in ["첫 번째", "두 번째", "세 번째"] {
            let doc = &mut a.data.notes[0].document;
            doc.add_tab();
            doc.tabs[doc.active].title = name.into();
            doc.delete_tab(doc.active);
            tabs.push(doc.trash.last().unwrap().id);
        }
        a.trash_note(0);
        a.desktop
            .trash_selection
            .items
            .extend([TrashItem::Tab(0, tabs[2]), TrashItem::Tab(0, tabs[0])]);
        a.restore_trash_items();
        let n = &a.data.notes[0];
        assert!(!n.deleted);
        assert_eq!(n.document.trash[0].id, tabs[1]);
        assert_eq!(n.document.tabs[1].title, "첫 번째");
        assert_eq!(n.document.tabs[2].title, "세 번째");
    }

    #[test]
    fn an_empty_request_covers_hidden_groups_and_live_owners_tabs() {
        let mut a = app();
        a.trash_note(0);
        a.trash_note(1);
        let doc = &mut a.data.notes[2].document;
        doc.delete_tab(0);
        a.request_trash_purge(PurgeKind::Empty, None);
        let request = a.desktop.purge_target.take().unwrap();
        assert_eq!(request.targets.counts(), (2, 1));
        assert!(a.confirm_trash_purge(request).is_some());
        assert!(a.data.notes[0].purged && a.data.notes[1].purged);
        assert!(!a.data.notes[2].purged && a.data.notes[2].document.trash.is_empty());
        assert_eq!(a.all_trash_items().count(), 0);
    }

    #[test]
    fn changes_to_an_empty_request_require_new_confirmation() {
        let mut a = app();
        a.trash_note(0);
        a.request_trash_purge(PurgeKind::Empty, None);
        let request = a.desktop.purge_target.take().unwrap();
        a.trash_note(1);
        assert!(a.confirm_trash_purge(request).is_none());
        assert!(!a.data.notes[0].purged && !a.data.notes[1].purged);
    }

    #[test]
    fn selected_groups_deduplicate_memos_and_their_tabs() {
        let mut a = app();
        a.trash_note(0);
        a.trash_note(1);
        let doc = &mut a.data.notes[0].document;
        doc.delete_tab(0);
        let id = doc.trash[0].id;
        let group = Group::Folder(0);
        a.desktop.trash_selected_groups.insert(group, vec![0, 1]);
        a.desktop.trash_selection.items.extend([
            TrashItem::Group(group),
            TrashItem::Memo(0),
            TrashItem::Tab(0, id),
        ]);
        a.request_trash_purge(PurgeKind::Selection, None);
        let request = a.desktop.purge_target.take().unwrap();
        assert_eq!(request.targets.counts(), (2, 1));
        a.restore_note(1);
        assert!(a.confirm_trash_purge(request).is_none());
        assert!(a.data.notes[0].deleted && !a.data.notes[0].purged);
    }

    fn app() -> App {
        let mut a = App::sample(NaiveDate::from_ymd_opt(2026, 11, 9).unwrap());
        a.extra.wiki_tabs = vec!["위키 1".into(), "자료".into()];
        a.extra.deleted_wiki_tabs.clear();
        a.extra.folders.clear();
        a.catalog.folders.clear();
        a.catalog.assignments.clear();
        for i in 0..a.data.notes.len() {
            let n = &mut a.data.notes[i];
            n.deleted = false;
            n.purged = false;
            n.wiki = false;
            n.wiki_tab = None;
            n.folder = None;
        }
        a
    }
    fn wiki(a: &mut App, i: usize, tab: usize) {
        a.data.notes[i].wiki = true;
        a.data.notes[i].wiki_tab = Some(tab);
    }

    #[test]
    fn deleting_a_wiki_tab_sends_the_tab_and_all_its_memos_to_the_bin() {
        let mut a = app();
        wiki(&mut a, 0, 1);
        wiki(&mut a, 1, 1);
        wiki(&mut a, 2, 0);
        a.trash_wiki_tab(1);
        assert!(a.data.notes[0].deleted && a.data.notes[1].deleted);
        assert!(!a.data.notes[2].deleted, "other tabs keep their memos");
        assert!(a.extra.deleted_wiki_tabs.contains(&1));
        assert_eq!(a.trash_groups(), vec![Group::Wiki(1)]);
        assert_eq!(a.group_members(Group::Wiki(1)), vec![0, 1]);
    }

    #[test]
    fn restoring_the_group_brings_back_the_tab_and_every_memo() {
        let mut a = app();
        wiki(&mut a, 0, 1);
        wiki(&mut a, 1, 1);
        a.trash_wiki_tab(1);
        a.restore_group(Group::Wiki(1));
        assert!(!a.extra.deleted_wiki_tabs.contains(&1));
        assert!(!a.data.notes[0].deleted && !a.data.notes[1].deleted);
        assert!(a.trash_groups().is_empty());
    }

    #[test]
    fn restoring_one_memo_recreates_the_tab_with_only_that_memo() {
        let mut a = app();
        wiki(&mut a, 0, 1);
        wiki(&mut a, 1, 1);
        a.trash_wiki_tab(1);
        a.restore_note(0);
        assert!(!a.extra.deleted_wiki_tabs.contains(&1), "the tab is back");
        assert!(!a.data.notes[0].deleted);
        assert!(a.data.notes[1].deleted, "the other memo stays in the bin");
        // It is no longer a group: the remaining memo shows as 탭/제목.
        assert!(a.trash_groups().is_empty());
        let title = a.data.notes[1].title.clone();
        assert_eq!(a.bin_prefix(1), Some(format!("자료/{title}")));
    }

    #[test]
    fn a_memo_deleted_alone_is_named_after_its_tab_and_joins_the_group_later() {
        let mut a = app();
        wiki(&mut a, 0, 1);
        wiki(&mut a, 1, 1);
        a.trash_note(0);
        let title = a.data.notes[0].title.clone();
        assert_eq!(a.bin_prefix(0), Some(format!("자료/{title}")));
        assert!(a.trash_groups().is_empty());
        a.trash_wiki_tab(1);
        // Both memos are now under the one tab group.
        assert_eq!(a.trash_groups(), vec![Group::Wiki(1)]);
        assert_eq!(a.group_members(Group::Wiki(1)), vec![0, 1]);
        assert_eq!(a.bin_prefix(0), None);
    }

    #[test]
    fn memos_in_wiki_folders_follow_the_same_rules() {
        let mut a = app();
        a.extra.folders.push(crate::extras::Folder {
            title: "폴더".into(),
            description: String::new(),
            parent: None,
            tab: 1,
        });
        a.data.notes[0].wiki = true;
        a.data.notes[0].folder = Some(0);
        a.data.notes[0].wiki_tab = Some(1);
        a.trash_wiki_tab(1);
        assert!(a.data.notes[0].deleted);
        assert_eq!(a.group_members(Group::Wiki(1)), vec![0]);
        a.restore_note(0);
        assert!(!a.extra.deleted_wiki_tabs.contains(&1));
    }

    fn folders(a: &mut App) {
        let f = |title: &str, parent| crate::catalog::Folder {
            title: title.into(),
            parent,
            open: true,
            deleted: false,
        };
        a.catalog.folders = vec![f("업무", None), f("하위", Some(0)), f("개인", None)];
    }

    #[test]
    fn deleting_a_folder_with_its_memos_makes_one_group_with_subfolders_inside() {
        let mut a = app();
        folders(&mut a);
        a.catalog.assignments.insert(0, 0);
        a.catalog.assignments.insert(1, 1);
        a.catalog.assignments.insert(2, 2);
        a.trash_catalog_folder_with_notes(0);
        assert!(a.catalog.folders[0].deleted && a.catalog.folders[1].deleted);
        assert!(!a.catalog.folders[2].deleted);
        assert!(a.data.notes[0].deleted && a.data.notes[1].deleted);
        assert!(!a.data.notes[2].deleted);
        assert_eq!(a.trash_groups(), vec![Group::Folder(0)]);
        assert_eq!(a.group_members(Group::Folder(0)), vec![0, 1]);
        let own = a.data.notes[0].title.clone();
        let nested = a.data.notes[1].title.clone();
        assert_eq!(a.group_member_title(Group::Folder(0), 0), own);
        assert_eq!(
            a.group_member_title(Group::Folder(0), 1),
            format!("하위/{nested}")
        );
    }

    #[test]
    fn restoring_a_folder_group_or_one_of_its_memos_brings_the_folders_back() {
        let mut a = app();
        folders(&mut a);
        a.catalog.assignments.insert(0, 0);
        a.catalog.assignments.insert(1, 1);
        a.trash_catalog_folder_with_notes(0);
        a.restore_group(Group::Folder(0));
        assert!(a.catalog.folders.iter().all(|f| !f.deleted));
        assert!(!a.data.notes[0].deleted && !a.data.notes[1].deleted);

        a.trash_catalog_folder_with_notes(0);
        a.restore_note(1);
        assert!(!a.catalog.folders[1].deleted, "the memo's own folder");
        assert!(!a.catalog.folders[0].deleted, "and the folder above it");
        assert!(!a.data.notes[1].deleted);
        assert!(a.data.notes[0].deleted, "the other memo stays in the bin");
    }

    #[test]
    fn deleting_a_folder_but_keeping_its_memos_makes_no_group() {
        let mut a = app();
        folders(&mut a);
        a.catalog.assignments.insert(0, 2);
        a.catalog.remove_folder(2);
        assert!(a.trash_groups().is_empty());
        assert!(!a.data.notes[0].deleted);
    }
}
