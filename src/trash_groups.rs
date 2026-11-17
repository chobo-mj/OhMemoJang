//! Recycle-bin groups. Deleting a wiki tab or a catalog folder together with
//! its memos sends them to the bin as one "이름/" row that opens like a folder.
//!
//! Nothing extra is stored: a group is derived from what is already saved —
//! a deleted wiki tab (`deleted_wiki_tabs`) or a deleted catalog folder, plus
//! the deleted memos that belong to it. So memos deleted one by one before the
//! tab was deleted join the same group, and restoring a single memo simply
//! brings its tab or folder back.
use crate::*;
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Group {
    /// A deleted wiki tab.
    Wiki(usize),
    /// A deleted catalog folder (the top one of a deleted branch).
    Folder(usize),
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
    pub fn trash_groups(&self) -> Vec<Group> {
        let mut groups = vec![];
        for t in 0..self.extra.wiki_tabs.len() {
            if self.extra.deleted_wiki_tabs.contains(&t)
                && !self.group_members(Group::Wiki(t)).is_empty()
            {
                groups.push(Group::Wiki(t));
            }
        }
        for (f, folder) in self.catalog.folders.iter().enumerate() {
            let top = folder
                .parent
                .is_none_or(|p| self.catalog.folders.get(p).is_none_or(|p| !p.deleted));
            if folder.deleted && top && !self.group_members(Group::Folder(f)).is_empty() {
                groups.push(Group::Folder(f));
            }
        }
        groups
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
        if self.data.notes[i].deleted || self.data.notes[i].purged {
            return;
        }
        self.data.notes[i].deleted = true;
        self.desktop
            .deleted_at
            .insert(i, Local::now().naive_local());
        if let Some(w) = self.studio.windows.get_mut(&i) {
            w.open = false;
        }
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
        for i in members {
            self.restore_note(i);
        }
        let name = self.group_name(g);
        self.toast = format!("‘{name}/’ 와 메모 {count}개를 복구했습니다");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
