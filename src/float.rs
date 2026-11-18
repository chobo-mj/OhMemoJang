//! Right-click menus and small popups shown in their own borderless,
//! always-on-top window, so a short sticker, a dialog or the calendar window
//! never clips them: they may reach past the window they belong to.
use eframe::egui::{self, Id, Pos2, Rect, Vec2};

/// An open floating menu, kept in the context's temporary data.
#[derive(Clone, Copy, Debug)]
struct Open {
    /// Screen rect it hangs from (a point for a right click).
    anchor: Rect,
    /// Open above the anchor (a toolbar at the bottom of its window).
    above: bool,
    size: Vec2,
    seen_focus: bool,
    away_since: Option<f64>,
    /// Opened this frame: the click that opened it may still read as a press.
    fresh: bool,
}

fn state(id: Id) -> Id {
    id.with("float_menu")
}

/// The top-left of the current window on the screen (points).
pub fn screen_origin(ctx: &egui::Context) -> Pos2 {
    ctx.input(|v| v.viewport().inner_rect.map(|r| r.min))
        .unwrap_or_default()
}

/// Open the menu `id` at screen point `at`, `width` wide.
pub fn open_at(ctx: &egui::Context, id: Id, at: Pos2, width: f32) {
    open(ctx, id, Rect::from_min_size(at, Vec2::ZERO), false, width);
}

fn open(ctx: &egui::Context, id: Id, anchor: Rect, above: bool, width: f32) {
    ctx.data_mut(|d| {
        d.insert_temp(
            state(id),
            Open {
                anchor,
                above,
                size: Vec2::new(width, 120.),
                seen_focus: false,
                away_since: None,
                fresh: true,
            },
        )
    });
}

pub fn close(ctx: &egui::Context, id: Id) {
    ctx.data_mut(|d| d.remove::<Open>(state(id)));
}

pub fn is_open(ctx: &egui::Context, id: Id) -> bool {
    ctx.data(|d| d.get_temp::<Open>(state(id)).is_some())
}

/// Keep the menu open after the click that is being handled (a row that
/// unfolds more rows, for instance).
pub fn keep_open(ui: &egui::Ui) {
    ui.ctx()
        .data_mut(|d| d.insert_temp(Id::new("float_keep_open"), true));
}

/// Draw the menu `id` when it is open. A `menu` closes after any click on
/// it (except into a text box or a row that called [`keep_open`]); a popup
/// closes when its content closes the egui popup `popup` (as
/// `ui.memory_mut(|m| m.close_popup())` does). Both close on Escape, on a
/// click back in the window they belong to (outside `exempt`, the button
/// that toggles it), and when the focus has gone elsewhere.
fn show(
    ctx: &egui::Context,
    id: Id,
    menu: bool,
    popup: Option<Id>,
    exempt: Option<Rect>,
    add: impl FnOnce(&mut egui::Ui),
) {
    let Some(mut o) = ctx.data(|d| d.get_temp::<Open>(state(id))) else {
        return;
    };
    if !std::mem::take(&mut o.fresh) {
        let press = ctx.input(|v| {
            v.pointer.any_pressed()
                && v.pointer
                    .interact_pos()
                    .is_some_and(|p| !exempt.is_some_and(|r| r.contains(p)))
        });
        if press {
            close(ctx, id);
            return;
        }
    }
    let at = if o.above {
        Pos2::new(o.anchor.left(), o.anchor.top() - o.size.y - 2.)
    } else if o.anchor.area() > 0. {
        Pos2::new(o.anchor.left(), o.anchor.bottom() + 2.)
    } else {
        o.anchor.min
    };
    let keep = Id::new("float_keep_open");
    let mut done = false;
    let shown = crate::studio::menu_window(
        ctx,
        egui::ViewportId::from_hash_of(state(id)),
        at,
        o.size,
        None,
        |ui| {
            if let Some(p) = popup {
                ui.memory_mut(|m| m.open_popup(p));
            }
            add(ui);
            let ctx = ui.ctx();
            if let Some(p) = popup {
                if ui.memory(|m| m.is_popup_open(p)) {
                    ui.memory_mut(|m| m.close_popup());
                } else {
                    done = true;
                }
            }
            let kept = ctx.data_mut(|d| d.remove_temp::<bool>(keep).unwrap_or(false));
            if menu && ctx.input(|v| v.pointer.any_click()) && !kept {
                let typing = ctx
                    .memory(|m| m.focused())
                    .is_some_and(|f| egui::TextEdit::load_state(ctx, f).is_some());
                done |= !typing;
            }
        },
    );
    o.size = shown.size;
    let now = ctx.input(|v| v.time);
    if shown.focused {
        o.seen_focus = true;
        o.away_since = None;
    } else if o.seen_focus {
        let since = *o.away_since.get_or_insert(now);
        done |= now - since > 0.3;
    }
    if done || shown.escape {
        close(ctx, id);
    } else {
        ctx.data_mut(|d| d.insert_temp(state(id), o));
    }
}

/// Like `response.context_menu`, in a window of its own at the pointer.
pub fn context_menu(response: &egui::Response, width: f32, add: impl FnOnce(&mut egui::Ui)) {
    let ctx = &response.ctx;
    let id = response.id;
    if response.secondary_clicked() {
        if let Some(p) = ctx.input(|v| v.pointer.interact_pos()) {
            open_at(ctx, id, screen_origin(ctx) + p.to_vec2(), width);
        }
    }
    show(ctx, id, true, None, None, add);
}

/// A menu opened with [`open_at`] (a right click the caller noticed).
pub fn menu(ctx: &egui::Context, id: Id, add: impl FnOnce(&mut egui::Ui)) {
    show(ctx, id, true, None, None, add);
}

/// Like `egui::popup_below_widget` (or above, for a widget in the lower half
/// of its window), in a window of its own. The caller opens and closes it
/// with egui's popup memory as before (`toggle_popup(popup_id)` /
/// `close_popup()`).
pub fn popup(
    ui: &egui::Ui,
    popup_id: Id,
    anchor: &egui::Response,
    width: f32,
    add: impl FnOnce(&mut egui::Ui),
) {
    let ctx = ui.ctx();
    if ui.memory(|m| m.is_popup_open(popup_id)) {
        ui.memory_mut(|m| m.close_popup());
        if is_open(ctx, popup_id) {
            close(ctx, popup_id);
        } else {
            let origin = screen_origin(ctx);
            let above = anchor.rect.center().y > ctx.screen_rect().center().y;
            open(
                ctx,
                popup_id,
                anchor.rect.translate(origin.to_vec2()),
                above,
                width,
            );
        }
    }
    show(ctx, popup_id, false, Some(popup_id), Some(anchor.rect), add);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_menu_opens_once_and_closes() {
        let ctx = egui::Context::default();
        let id = Id::new("m");
        assert!(!is_open(&ctx, id));
        open_at(&ctx, id, Pos2::new(10., 10.), 200.);
        assert!(is_open(&ctx, id));
        close(&ctx, id);
        assert!(!is_open(&ctx, id));
    }
}
