use eframe::egui;
use std::collections::HashSet;
use std::hash::Hash;

pub struct Selection<T> {
    pub active: bool,
    pub items: HashSet<T>,
    anchor: Option<T>,
}

impl<T> Default for Selection<T> {
    fn default() -> Self {
        Self {
            active: false,
            items: HashSet::new(),
            anchor: None,
        }
    }
}

impl<T: Copy + Eq + Hash> Selection<T> {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn retain_visible(&mut self, ordered: &[T]) {
        if self.items.is_empty() && self.anchor.is_none() {
            return;
        }
        let visible: HashSet<_> = ordered.iter().copied().collect();
        self.items.retain(|item| visible.contains(item));
        if self.anchor.is_some_and(|anchor| !visible.contains(&anchor)) {
            self.anchor = None;
        }
    }

    pub fn click(&mut self, item: T, ordered: &[T], modifiers: egui::Modifiers) {
        if !self.active && !(modifiers.ctrl || modifiers.shift || modifiers.command) {
            return;
        }
        self.active = true;
        if modifiers.shift {
            let Some(to) = ordered.iter().position(|&id| id == item) else {
                return;
            };
            let from = self
                .anchor
                .and_then(|anchor| ordered.iter().position(|&id| id == anchor));
            if !(modifiers.ctrl || modifiers.command) {
                self.items.clear();
            }
            let start = from.unwrap_or(to);
            self.anchor.get_or_insert(item);
            self.items
                .extend(ordered[start.min(to)..=start.max(to)].iter().copied());
        } else {
            if !self.items.remove(&item) {
                self.items.insert(item);
            }
            self.anchor = Some(item);
        }
    }

    pub fn keyboard(&mut self, ctx: &egui::Context, ordered: &[T], focused: bool, blocked: bool) {
        if blocked || ctx.memory(|m| m.any_popup_open()) {
            return;
        }
        if self.active
            && (focused || !ctx.wants_keyboard_input())
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.clear();
        }
        if focused
            && ctx.input_mut(|i| {
                i.consume_key(egui::Modifiers::CTRL, egui::Key::A)
                    || i.consume_key(egui::Modifiers::COMMAND, egui::Key::A)
            })
        {
            self.active = true;
            self.items = ordered.iter().copied().collect();
            self.anchor = ordered.first().copied();
        }
    }

    pub fn toolbar(&mut self, ui: &mut egui::Ui) -> bool {
        let label = if self.active {
            "선택 종료"
        } else {
            "여러 개 선택"
        };
        let clicked = ui.button(label).clicked();
        if clicked {
            if self.active {
                self.clear();
            } else {
                self.active = true;
            }
        }
        if self.active {
            ui.label(format!("{}개 선택", self.items.len()));
        }
        clicked
    }

    pub fn checkbox(&mut self, ui: &mut egui::Ui, item: T, ordered: &[T]) -> bool {
        if !self.active {
            return false;
        }
        let mut selected = self.items.contains(&item);
        let response = ui.checkbox(&mut selected, "").on_hover_text("선택");
        if response.changed() {
            self.click(item, ordered, ui.input(|i| i.modifiers));
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selecting_a_range_uses_display_order_and_keeps_its_anchor() {
        let mut selection = Selection::default();
        let order = [13, 4, 9, 2, 7];
        selection.click(13, &order, egui::Modifiers::CTRL);
        selection.click(2, &order, egui::Modifiers::CTRL);
        selection.click(4, &order, egui::Modifiers::CTRL | egui::Modifiers::SHIFT);
        assert_eq!(selection.items, HashSet::from([13, 4, 9, 2]));
        selection.click(7, &order, egui::Modifiers::SHIFT);
        assert_eq!(selection.items, HashSet::from([2, 7]));
    }

    #[test]
    fn changing_visible_items_removes_hidden_selections_and_their_anchor() {
        let mut selection = Selection::default();
        selection.click(1, &[1, 2, 3], egui::Modifiers::CTRL);
        selection.retain_visible(&[2, 3]);
        assert!(selection.items.is_empty());
        selection.click(3, &[2, 3], egui::Modifiers::SHIFT);
        assert_eq!(selection.items, HashSet::from([3]));
        selection.clear();
        assert!(!selection.active);
    }
}
