//! Shared visual language: colors, type scale, compact controls and
//! dialogs that open as real windows instead of being clipped by a parent.
use crate::icons::{paint_icon, Icon};
use eframe::egui::{
    self, Color32, CornerRadius, FontId, Id, Pos2, Rect, Response, RichText, Sense, Stroke,
    StrokeKind, Ui, Vec2, ViewportBuilder, ViewportClass, ViewportId,
};
use std::hash::Hash;

pub const ACCENT: Color32 = Color32::from_rgb(91, 89, 204);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(228, 229, 253);
pub const BLUE: Color32 = Color32::from_rgb(71, 100, 201);
const CORAL: Color32 = Color32::from_rgb(217, 87, 56);

fn variant_key(ctx: &egui::Context) -> Id {
    Id::new(("workspace-light-variant", ctx.viewport_id()))
}

/// Light 1 uses the yellow paper palette only in the main workspace.
pub fn palette_variant(ctx: &egui::Context, variant: usize) -> usize {
    if variant == 0 && ctx.viewport_id() != crate::main_viewport() {
        1
    } else {
        variant
    }
}
pub fn paper(ui: &Ui) -> bool {
    variant(ui) == 0 && ui.ctx().viewport_id() == crate::main_viewport()
}
pub const MEMO_YELLOW: Color32 = Color32::from_rgb(255, 218, 95);
const OCHRE: Color32 = Color32::from_rgb(117, 79, 10);

/// The layout variant is local to each viewport.
pub fn variant_ctx(ctx: &egui::Context) -> usize {
    let key = variant_key(ctx);
    ctx.data(|d| d.get_temp::<usize>(key)).unwrap_or(1)
}
pub fn variant(ui: &Ui) -> usize {
    variant_ctx(ui.ctx())
}
pub fn accent_for(variant: usize) -> Color32 {
    match variant {
        0 => OCHRE,
        2 => ACCENT,
        _ => CORAL,
    }
}
pub fn accent(ui: &Ui) -> Color32 {
    let key = variant_key(ui.ctx());
    let selected = ui.ctx().data(|d| d.get_temp::<usize>(key));
    selected
        .map(|v| accent_for(palette_variant(ui.ctx(), v)))
        .unwrap_or(CORAL)
}
fn blue(ui: &Ui) -> Color32 {
    let key = variant_key(ui.ctx());
    if ui
        .ctx()
        .data(|d| d.get_temp::<usize>(key))
        .is_none_or(|v| v < 2)
    {
        Color32::from_rgb(56, 110, 196)
    } else {
        BLUE
    }
}

/// Color tokens resolved for the current light or dark theme.
#[derive(Clone, Copy)]
pub struct Tone {
    pub bg: Color32,
    pub sidebar: Color32,
    pub field: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    pub line: Color32,
    pub line_strong: Color32,
    pub ink: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub accent_soft: Color32,
}
pub fn tone(dark: bool) -> Tone {
    if dark {
        Tone {
            bg: Color32::from_rgb(30, 30, 29),
            sidebar: Color32::from_rgb(25, 25, 24),
            field: Color32::from_rgb(43, 43, 41),
            hover: Color32::from_rgb(50, 50, 48),
            pressed: Color32::from_rgb(60, 60, 57),
            line: Color32::from_rgb(52, 52, 50),
            line_strong: Color32::from_rgb(70, 70, 67),
            ink: Color32::from_rgb(234, 233, 229),
            muted: Color32::from_rgb(158, 157, 152),
            faint: Color32::from_rgb(110, 109, 105),
            accent_soft: Color32::from_rgb(70, 42, 34),
        }
    } else {
        Tone {
            bg: Color32::from_rgb(248, 249, 253),
            sidebar: Color32::from_rgb(235, 238, 248),
            field: Color32::from_rgb(234, 237, 248),
            hover: Color32::from_rgb(225, 229, 245),
            pressed: Color32::from_rgb(211, 216, 238),
            line: Color32::from_rgb(222, 226, 238),
            line_strong: Color32::from_rgb(200, 207, 225),
            ink: Color32::from_rgb(36, 43, 69),
            muted: Color32::from_rgb(102, 114, 143),
            faint: Color32::from_rgb(139, 150, 174),
            accent_soft: ACCENT_SOFT,
        }
    }
}

/// Paper, coral, and indigo workspace palettes.
pub fn light_tone(variant: usize) -> Tone {
    if variant == 0 {
        Tone {
            bg: Color32::from_rgb(250, 249, 244),
            sidebar: Color32::from_rgb(244, 241, 229),
            field: Color32::from_rgb(239, 237, 225),
            hover: Color32::from_rgb(244, 238, 209),
            pressed: Color32::from_rgb(237, 223, 166),
            line: Color32::from_rgb(223, 221, 206),
            line_strong: Color32::from_rgb(196, 191, 169),
            ink: Color32::from_rgb(47, 48, 36),
            muted: Color32::from_rgb(94, 95, 77),
            faint: Color32::from_rgb(109, 109, 91),
            accent_soft: Color32::from_rgb(255, 238, 171),
        }
    } else if variant == 2 {
        tone(false)
    } else {
        Tone {
            bg: Color32::WHITE,
            sidebar: Color32::from_rgb(248, 247, 245),
            field: Color32::from_rgb(242, 241, 238),
            hover: Color32::from_rgb(235, 234, 231),
            pressed: Color32::from_rgb(225, 224, 220),
            line: Color32::from_rgb(234, 233, 230),
            line_strong: Color32::from_rgb(214, 213, 209),
            ink: Color32::from_rgb(44, 43, 40),
            muted: Color32::from_rgb(120, 119, 115),
            faint: Color32::from_rgb(166, 165, 160),
            accent_soft: Color32::from_rgb(252, 237, 231),
        }
    }
}
const BRIGHTER_DARK_BG: Color32 = Color32::from_rgb(48, 48, 46);

pub fn theme_tone(dark: bool, brighter: bool) -> Tone {
    let base = tone(dark);
    if dark && brighter {
        Tone {
            bg: BRIGHTER_DARK_BG,
            sidebar: Color32::from_rgb(41, 41, 39),
            field: Color32::from_rgb(60, 60, 57),
            hover: Color32::from_rgb(68, 68, 65),
            pressed: Color32::from_rgb(78, 78, 75),
            line: Color32::from_rgb(68, 68, 65),
            line_strong: Color32::from_rgb(87, 87, 83),
            muted: Color32::from_rgb(176, 175, 169),
            faint: Color32::from_rgb(138, 137, 131),
            accent_soft: Color32::from_rgb(88, 54, 45),
            ..base
        }
    } else {
        base
    }
}
pub fn t(ui: &Ui) -> Tone {
    let visuals = ui.visuals();
    if !visuals.dark_mode {
        let key = variant_key(ui.ctx());
        if let Some(variant) = ui.ctx().data(|d| d.get_temp::<usize>(key)) {
            return light_tone(palette_variant(ui.ctx(), variant));
        }
    }
    theme_tone(visuals.dark_mode, visuals.panel_fill == BRIGHTER_DARK_BG)
}

/// Install visuals and spacing for a whole context. `body` is the body size in points.
pub fn apply(ctx: &egui::Context, dark: bool, brighter: bool, family: egui::FontFamily, body: f32) {
    let key = variant_key(ctx);
    ctx.data_mut(|d| d.remove::<usize>(key));
    apply_inner(ctx, dark, brighter, None, family, body);
}

/// Install one of the workspace's three light designs.
pub fn apply_light(ctx: &egui::Context, variant: usize, family: egui::FontFamily, body: f32) {
    let variant = variant.min(2);
    let key = variant_key(ctx);
    ctx.data_mut(|d| d.insert_temp(key, variant));
    apply_inner(
        ctx,
        false,
        false,
        Some(palette_variant(ctx, variant)),
        family,
        body,
    );
}

fn apply_inner(
    ctx: &egui::Context,
    dark: bool,
    brighter: bool,
    variant: Option<usize>,
    family: egui::FontFamily,
    body: f32,
) {
    // Every window asks for its look every frame; the style is shared, so
    // build and install it only when what it is made of has changed.
    let key = egui::Id::new("ui-applied");
    let want = (dark, brighter, variant, family.clone(), body.to_bits());
    if ctx.data(|d| d.get_temp::<(bool, bool, Option<usize>, egui::FontFamily, u32)>(key))
        == Some(want.clone())
    {
        return;
    }
    ctx.data_mut(|d| d.insert_temp(key, want));
    let c = variant
        .map(light_tone)
        .unwrap_or_else(|| theme_tone(dark, brighter));
    let active_accent = variant.map(accent_for).unwrap_or(CORAL);
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.panel_fill = c.bg;
    v.window_fill = c.bg;
    v.faint_bg_color = if dark && brighter {
        Color32::from_rgb(54, 54, 52)
    } else if dark {
        Color32::from_rgb(35, 35, 34)
    } else if variant == Some(0) {
        Color32::WHITE
    } else if variant == Some(1) {
        Color32::from_rgb(250, 250, 248)
    } else {
        Color32::from_rgb(241, 244, 251)
    };
    v.extreme_bg_color = c.field;
    v.code_bg_color = c.field;
    v.override_text_color = Some(c.ink);
    v.hyperlink_color = if variant.is_none_or(|v| v < 2) {
        Color32::from_rgb(56, 110, 196)
    } else {
        BLUE
    };
    v.selection.bg_fill = c.accent_soft;
    v.selection.stroke = Stroke::new(1.0_f32, active_accent);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v.window_stroke = Stroke::new(1.0_f32, c.line);
    v.window_shadow = egui::epaint::Shadow {
        offset: [0, 6],
        blur: 22,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 90 } else { 28 }),
    };
    v.popup_shadow = egui::epaint::Shadow {
        offset: [0, 4],
        blur: 14,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 90 } else { 24 }),
    };
    // A blinking caret would redraw (and present) its window about ten times
    // a second for as long as it is there; a steady caret costs nothing.
    v.text_cursor.blink = false;
    v.striped = false;
    v.slider_trailing_fill = true;
    v.handle_shape = egui::style::HandleShape::Circle;
    let radius = CornerRadius::same(6);
    let w = &mut v.widgets;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, c.line);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, c.ink);
    w.noninteractive.corner_radius = radius;
    for (state, fill) in [
        (&mut w.inactive, c.field),
        (&mut w.hovered, c.hover),
        (&mut w.active, c.pressed),
        (&mut w.open, c.hover),
    ] {
        state.weak_bg_fill = fill;
        state.bg_fill = fill;
        state.bg_stroke = Stroke::NONE;
        state.corner_radius = radius;
        state.expansion = 0.;
        state.fg_stroke = Stroke::new(1.2_f32, c.ink);
    }
    w.inactive.bg_fill = c.line_strong;
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(6., 6.);
        s.spacing.button_padding = Vec2::new(9., 4.);
        s.spacing.interact_size = Vec2::new(28., 26.);
        s.spacing.icon_width = 15.;
        s.spacing.icon_width_inner = 8.;
        s.spacing.icon_spacing = 6.;
        s.spacing.menu_margin = egui::Margin::same(6);
        s.spacing.window_margin = egui::Margin::same(14);
        s.spacing.combo_width = 120.;
        s.spacing.indent = 16.;
        s.spacing.slider_width = 160.;
        s.spacing.scroll = egui::style::ScrollStyle::floating();
        s.spacing.scroll.bar_width = 8.;
        s.spacing.scroll.floating_allocated_width = 0.;
        s.spacing.scroll.dormant_handle_opacity = 0.;
        s.spacing.scroll.dormant_background_opacity = 0.;
        s.interaction.selectable_labels = false;
        // A tooltip shows only once the pointer has rested on a widget
        // (tooltip_delay). With egui's grace time, the tooltip of each icon
        // the pointer passed over popped up and went again at once, so
        // sweeping over a sticker's toolbar redrew the window on every frame.
        s.interaction.show_tooltips_only_when_still = false;
        s.interaction.tooltip_grace_time = 0.;
        let sizes = [
            (egui::TextStyle::Small, body * 0.86),
            (egui::TextStyle::Body, body),
            (egui::TextStyle::Button, body),
            (egui::TextStyle::Heading, body * 1.5),
            (egui::TextStyle::Monospace, body * 0.92),
        ];
        for (style, size) in sizes {
            s.text_styles.insert(
                style.clone(),
                FontId::new(
                    size,
                    if style == egui::TextStyle::Monospace {
                        egui::FontFamily::Monospace
                    } else {
                        family.clone()
                    },
                ),
            );
        }
    });
}

/// Muted helper text.
pub fn hint(ui: &mut Ui, text: impl Into<String>) -> Response {
    let c = t(ui).muted;
    ui.label(RichText::new(text.into()).small().color(c))
}

/// Small uppercase-style group label used in the sidebar and settings.
pub fn caption(ui: &mut Ui, text: &str) -> Response {
    let c = t(ui).faint;
    ui.label(RichText::new(text).size(11.5).color(c).strong())
}

/// Filled accent button for the single primary action of a view.
pub fn primary(ui: &mut Ui, text: &str) -> Response {
    let (fill, ink) = if paper(ui) {
        (MEMO_YELLOW, t(ui).ink)
    } else {
        (accent(ui), Color32::WHITE)
    };
    ui.add(
        egui::Button::new(RichText::new(text).color(ink).strong())
            .fill(fill)
            .corner_radius(6.),
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Menu opened from an icon-only button.
pub fn icon_menu(ui: &mut Ui, icon: Icon, tooltip: &str, add: impl FnOnce(&mut Ui)) -> Response {
    let button = egui::Button::new(RichText::new("  ").size(12.))
        .frame(false)
        .min_size(Vec2::splat(28.));
    let r = egui::menu::menu_custom_button(ui, button, add).response;
    if r.hovered() {
        ui.painter().rect_filled(
            r.rect,
            6.,
            if ui.visuals().dark_mode {
                Color32::from_white_alpha(18)
            } else {
                Color32::from_black_alpha(10)
            },
        );
    }
    let color = ui
        .visuals()
        .text_color()
        .gamma_multiply(if r.hovered() { 1. } else { 0.78 });
    paint_icon(ui.painter(), r.rect.shrink(5.5), icon, color);
    r.on_hover_text(tooltip)
}

/// iOS-like segmented control. Returns true when the value changed.
pub fn segmented(ui: &mut Ui, value: &mut usize, labels: &[&str]) -> bool {
    let c = t(ui);
    let font = FontId::new(
        ui.style().text_styles[&egui::TextStyle::Button].size * 0.95,
        ui.style().text_styles[&egui::TextStyle::Button]
            .family
            .clone(),
    );
    let galleys: Vec<_> = labels
        .iter()
        .map(|l| {
            ui.painter()
                .layout_no_wrap((*l).into(), font.clone(), c.ink)
        })
        .collect();
    let widths: Vec<f32> = galleys.iter().map(|g| g.size().x + 22.).collect();
    let height = 28.;
    let base = ui.next_auto_id();
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(widths.iter().sum::<f32>() + 4., height),
        Sense::hover(),
    );
    ui.painter().rect_filled(rect, 7., c.field);
    let mut x = rect.left() + 2.;
    let mut changed = false;
    for (i, (galley, w)) in galleys.into_iter().zip(widths).enumerate() {
        let r = Rect::from_min_size(Pos2::new(x, rect.top() + 2.), Vec2::new(w, height - 4.));
        let resp = ui
            .interact(r, base.with(("seg", i)), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let selected = *value == i;
        if selected {
            ui.painter().rect_filled(
                r.translate(Vec2::new(0., 1.)),
                6.,
                Color32::from_black_alpha(14),
            );
            ui.painter().rect_filled(
                r,
                6.,
                if ui.visuals().dark_mode {
                    c.pressed
                } else if paper(ui) {
                    MEMO_YELLOW
                } else {
                    Color32::WHITE
                },
            );
        } else if resp.hovered() {
            ui.painter().rect_filled(r, 6., c.hover);
        }
        ui.painter().galley(
            r.center() - galley.size() / 2.,
            galley,
            if selected { c.ink } else { c.muted },
        );
        if resp.clicked() && !selected {
            *value = i;
            changed = true;
        }
        x += w;
    }
    changed
}

/// File-folder tabs size to their labels and flow onto another line at narrow widths.
pub fn paper_tabs(ui: &mut Ui, selected: &mut usize, entries: &[(&str, Icon)]) {
    let c = t(ui);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(4., 3.);
        let font = egui::TextStyle::Button.resolve(ui.style());
        for (i, &(label, icon)) in entries.iter().enumerate() {
            let active = *selected == i;
            let ink = if active { c.ink } else { c.muted };
            let title = ui
                .painter()
                .layout_no_wrap(label.to_owned(), font.clone(), ink);
            let (rect, response) = ui.allocate_exact_size(
                Vec2::new(title.size().x + 40., title.size().y + 17.),
                Sense::click(),
            );
            let fill = if active {
                MEMO_YELLOW
            } else if response.hovered() {
                c.hover
            } else {
                c.field
            };
            let radius = CornerRadius {
                nw: 8,
                ne: 8,
                sw: 1,
                se: 1,
            };
            ui.painter().rect_filled(rect, radius, fill);
            if active || response.has_focus() {
                ui.painter().line_segment(
                    [rect.left_bottom(), rect.right_bottom()],
                    Stroke::new(2_f32, if active { c.ink } else { OCHRE }),
                );
            }
            paint_icon(
                ui.painter(),
                Rect::from_center_size(rect.left_center() + Vec2::new(15., 0.), Vec2::splat(15.)),
                icon,
                ink,
            );
            ui.painter().galley(
                Pos2::new(rect.left() + 28., rect.center().y - title.size().y * 0.5),
                title,
                ink,
            );
            response.widget_info(|| {
                egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, active, label)
            });
            if response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                *selected = i;
            }
        }
    });
}

/// Text tabs with an accent underline for the selected one.
#[allow(dead_code)]
pub fn tabs(ui: &mut Ui, value: &mut usize, labels: &[&str]) -> bool {
    let c = t(ui);
    let mut changed = false;
    let base = ui.next_auto_id();
    let (bar, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 34.), Sense::hover());
    ui.painter().line_segment(
        [bar.left_bottom(), bar.right_bottom()],
        Stroke::new(1.0_f32, c.line),
    );
    let font = egui::TextStyle::Button.resolve(ui.style());
    let mut x = bar.left();
    for (i, label) in labels.iter().enumerate() {
        let selected = *value == i;
        let galley = ui.painter().layout_no_wrap(
            (*label).into(),
            font.clone(),
            if selected { c.ink } else { c.muted },
        );
        let r = Rect::from_min_size(
            Pos2::new(x, bar.top()),
            Vec2::new(galley.size().x + 20., 34.),
        );
        let resp = ui
            .interact(r, base.with(("tab", i)), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        if resp.hovered() && !selected {
            ui.painter()
                .rect_filled(r.shrink2(Vec2::new(2., 4.)), 6., c.hover);
        }
        ui.painter()
            .galley(r.center() - galley.size() / 2., galley, c.ink);
        if selected {
            ui.painter().line_segment(
                [
                    Pos2::new(r.left() + 6., r.bottom() - 1.),
                    Pos2::new(r.right() - 6., r.bottom() - 1.),
                ],
                Stroke::new(2.0_f32, accent(ui)),
            );
        }
        if resp.clicked() && !selected {
            *value = i;
            changed = true;
        }
        x = r.right() + 4.;
    }
    changed
}

/// A compact on/off switch.
pub fn switch(ui: &mut Ui, on: &mut bool) -> Response {
    let size = Vec2::new(32., 18.);
    let (rect, mut r) = ui.allocate_exact_size(size, Sense::click());
    if r.clicked() {
        *on = !*on;
        r.mark_changed();
    }
    let how_on = ui.ctx().animate_bool_responsive(r.id, *on);
    let c = t(ui);
    ui.painter()
        .rect_filled(rect, 9., lerp_color(c.line_strong, accent(ui), how_on));
    let x = egui::lerp((rect.left() + 9.)..=(rect.right() - 9.), how_on);
    ui.painter()
        .circle_filled(Pos2::new(x, rect.center().y), 7., Color32::WHITE);
    r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, *on, ""));
    r.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A settings row: label (and optional description) on the left, switch on the right.
pub fn toggle_row(ui: &mut Ui, on: &mut bool, label: &str, description: &str) -> Response {
    let mut out = None;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width((ui.available_width() - 48.).max(120.));
            ui.label(label);
            if !description.is_empty() {
                hint(ui, description);
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            out = Some(switch(ui, on));
        });
    });
    ui.add_space(4.);
    out.unwrap()
}

/// Round task checkbox in the style of to-do apps.
pub fn round_check(ui: &mut Ui, checked: &mut bool, tint: Option<Color32>) -> Response {
    let (rect, mut r) = ui.allocate_exact_size(Vec2::splat(20.), Sense::click());
    if r.clicked() {
        *checked = !*checked;
        r.mark_changed();
    }
    let c = t(ui);
    let color = tint.unwrap_or(c.faint);
    let center = rect.center();
    if *checked {
        ui.painter().circle_filled(center, 8., accent(ui));
        check_mark(ui.painter(), center, Color32::WHITE);
    } else {
        ui.painter()
            .circle_stroke(center, 7.5, Stroke::new(1.4_f32, color));
        if r.hovered() {
            check_mark(ui.painter(), center, color);
        }
    }
    r.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, *checked, "완료")
    });
    r.on_hover_cursor(egui::CursorIcon::PointingHand)
}
fn check_mark(p: &egui::Painter, c: Pos2, color: Color32) {
    p.add(egui::Shape::line(
        vec![
            c + Vec2::new(-3.6, 0.2),
            c + Vec2::new(-1., 2.8),
            c + Vec2::new(3.8, -2.4),
        ],
        Stroke::new(1.6_f32, color),
    ));
}

/// Small pill used for project tags and meta information.
pub fn chip(ui: &mut Ui, text: &str, fill: Color32, color: Color32) -> Response {
    let font = FontId::proportional(11.5);
    let galley = ui.painter().layout_no_wrap(text.into(), font, color);
    let (rect, r) = ui.allocate_exact_size(galley.size() + Vec2::new(14., 6.), Sense::hover());
    ui.painter().rect_filled(rect, 9., fill);
    ui.painter()
        .galley(rect.center() - galley.size() / 2., galley, color);
    r
}

/// Selectable pill for filter toggles.
pub fn pill(ui: &mut Ui, selected: bool, text: &str) -> Response {
    let c = t(ui);
    let font = FontId::new(
        ui.style().text_styles[&egui::TextStyle::Button].size * 0.93,
        ui.style().text_styles[&egui::TextStyle::Button]
            .family
            .clone(),
    );
    let galley = ui.painter().layout_no_wrap(
        text.into(),
        font,
        if selected { accent(ui) } else { c.muted },
    );
    let (rect, r) = ui.allocate_exact_size(galley.size() + Vec2::new(20., 10.), Sense::click());
    let fill = if selected {
        c.accent_soft
    } else if r.hovered() {
        c.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 14., fill);
    if !selected {
        ui.painter().rect_stroke(
            rect,
            14.,
            Stroke::new(1.0_f32, c.line_strong),
            StrokeKind::Inside,
        );
    }
    ui.painter()
        .galley(rect.center() - galley.size() / 2., galley, c.ink);
    r.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Heading of a list section: icon, title and an optional count.
pub fn section_header(ui: &mut Ui, icon: Icon, title: &str, count: Option<usize>) {
    let c = t(ui);
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(16.), Sense::hover());
        paint_icon(ui.painter(), rect, icon, c.muted);
        ui.label(RichText::new(title).size(13.).strong().color(c.ink));
        if let Some(n) = count {
            ui.label(RichText::new(n.to_string()).size(12.).color(c.faint));
        }
    });
    ui.add_space(4.);
}

/// A rounded card that groups a list.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let c = t(ui);
    egui::Frame::NONE
        .fill(if paper(ui) { Color32::WHITE } else { c.bg })
        .stroke(Stroke::new(1.0_f32, c.line))
        .corner_radius(if paper(ui) { 6. } else { 10. })
        .inner_margin(egui::Margin::symmetric(8, 8))
        .show(ui, add)
        .inner
}

/// One full-width list row: a color dot, the title and right-aligned meta
/// text. The row is clickable and draggable.
pub fn list_row(
    ui: &mut Ui,
    id: Id,
    selected: bool,
    dot: Option<Color32>,
    title: &str,
    meta: &str,
    height: f32,
) -> Response {
    let c = t(ui);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
    let r = ui.interact(rect, id, Sense::click_and_drag());
    if selected {
        ui.painter().rect_filled(rect, 7., c.accent_soft);
    } else if r.hovered() {
        ui.painter().rect_filled(rect, 7., c.hover);
    }
    let mut x = rect.left() + 10.;
    if let Some(color) = dot {
        ui.painter()
            .circle_filled(egui::pos2(x + 4., rect.center().y), 4.5, color);
        x += 16.;
    }
    let meta_galley = ui
        .painter()
        .layout_no_wrap(meta.into(), FontId::proportional(11.5), c.faint);
    let meta_w = if meta.is_empty() {
        0.
    } else {
        meta_galley.size().x + 12.
    };
    let title_galley = ui.painter().layout(
        title.into(),
        FontId::proportional(13.5),
        if selected { accent(ui) } else { c.ink },
        f32::INFINITY,
    );
    let room = (rect.right() - 10. - meta_w - x).max(10.);
    let clip = egui::Rect::from_min_max(
        egui::pos2(x, rect.top()),
        egui::pos2(x + room, rect.bottom()),
    );
    ui.painter().with_clip_rect(clip).galley(
        egui::pos2(x, rect.center().y - title_galley.size().y / 2.),
        title_galley,
        c.ink,
    );
    if !meta.is_empty() {
        ui.painter().galley(
            egui::pos2(
                rect.right() - 10. - meta_galley.size().x,
                rect.center().y - meta_galley.size().y / 2.,
            ),
            meta_galley,
            c.faint,
        );
    }
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, title));
    r
}

/// A folder row in a tree: chevron, folder icon, title and a count.
/// Returns the row response and whether the chevron was clicked.
pub fn tree_row(
    ui: &mut Ui,
    id: Id,
    selected: bool,
    open: Option<bool>,
    icon: Icon,
    title: &str,
    count: usize,
) -> (Response, bool) {
    let c = t(ui);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.), Sense::hover());
    let r = ui.interact(rect, id, Sense::click_and_drag());
    if selected {
        ui.painter().rect_filled(rect, 7., c.accent_soft);
    } else if r.hovered() {
        ui.painter().rect_filled(rect, 7., c.hover);
    }
    let chevron = egui::Rect::from_center_size(
        egui::pos2(rect.left() + 13., rect.center().y),
        Vec2::splat(18.),
    );
    if let Some(open) = open {
        paint_icon(
            ui.painter(),
            chevron.shrink(4.),
            if open {
                Icon::ChevronDown
            } else {
                Icon::ChevronRight
            },
            c.muted,
        );
    }
    paint_icon(
        ui.painter(),
        egui::Rect::from_center_size(
            egui::pos2(rect.left() + 34., rect.center().y),
            Vec2::splat(15.),
        ),
        icon,
        if selected { accent(ui) } else { c.muted },
    );
    let title_galley = ui.painter().layout_no_wrap(
        title.into(),
        FontId::proportional(13.5),
        if selected { accent(ui) } else { c.ink },
    );
    let clip = egui::Rect::from_min_max(
        egui::pos2(rect.left() + 48., rect.top()),
        egui::pos2(rect.right() - 34., rect.bottom()),
    );
    ui.painter().with_clip_rect(clip).galley(
        egui::pos2(
            rect.left() + 48.,
            rect.center().y - title_galley.size().y / 2.,
        ),
        title_galley,
        c.ink,
    );
    if count > 0 {
        ui.painter().text(
            egui::pos2(rect.right() - 10., rect.center().y),
            egui::Align2::RIGHT_CENTER,
            count.to_string(),
            FontId::proportional(11.5),
            c.faint,
        );
    }
    r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, title));
    let toggled = open.is_some()
        && r.clicked()
        && r.interact_pointer_pos()
            .is_some_and(|p| chevron.expand(4.).contains(p));
    (r, toggled)
}

/// A quiet, centered placeholder for an empty list.
pub fn empty_state(ui: &mut Ui, icon: Icon, title: &str, hint_text: &str) {
    let c = t(ui);
    ui.vertical_centered(|ui| {
        ui.add_space(18.);
        if paper(ui) {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(40.), Sense::hover());
            let fold = 9.;
            let p = rect.right_top();
            let a = p - Vec2::new(fold, 0.);
            let b = p + Vec2::new(0., fold);
            ui.painter().add(egui::Shape::convex_polygon(
                vec![
                    rect.left_top(),
                    a,
                    b,
                    rect.right_bottom(),
                    rect.left_bottom(),
                ],
                c.accent_soft,
                Stroke::NONE,
            ));
            ui.painter().add(egui::Shape::convex_polygon(
                vec![a, b, a + Vec2::new(0., fold)],
                MEMO_YELLOW,
                Stroke::NONE,
            ));
            paint_icon(ui.painter(), rect.shrink(10.), icon, OCHRE);
        } else {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(26.), Sense::hover());
            paint_icon(ui.painter(), rect, icon, c.faint);
        }
        ui.add_space(6.);
        ui.label(RichText::new(title).size(13.).color(c.muted));
        if !hint_text.is_empty() {
            ui.label(RichText::new(hint_text).size(11.5).color(c.faint));
        }
        ui.add_space(18.);
    });
}

pub fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(
        l(a.r(), b.r()),
        l(a.g(), b.g()),
        l(a.b(), b.b()),
        l(a.a(), b.a()),
    )
}

/// A thin full-width divider.
pub fn divider(ui: &mut Ui) {
    let c = t(ui).line;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.), Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        Stroke::new(1.0_f32, c),
    );
}

pub fn weekday(d: chrono::NaiveDate) -> &'static str {
    use chrono::Datelike;
    ["일", "월", "화", "수", "목", "금", "토"][d.weekday().num_days_from_sunday() as usize]
}

pub fn main_viewport() -> ViewportId {
    ViewportId::from_hash_of("ohmemojang_main")
}

fn raise_key(vid: ViewportId) -> Id {
    Id::new(("dialog_raise", vid))
}
/// Bring the dialog `id` (as passed to [`dialog`]) in front of other windows
/// the next time it is shown, e.g. when its button is pressed while it is
/// already open behind the main window.
pub fn raise(ctx: &egui::Context, id: impl Hash) {
    let vid = ViewportId::from_hash_of(("dialog", id));
    ctx.data_mut(|d| d.insert_temp(raise_key(vid), true));
}
/// Show `add` in a separate native window, so it is never limited to the
/// bounds of the window that opened it. Closing the window clears `open`.
pub fn dialog<R>(
    ctx: &egui::Context,
    id: impl Hash,
    title: &str,
    size: [f32; 2],
    on_top: bool,
    open: &mut bool,
    add: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    if !*open {
        return None;
    }
    let vid = ViewportId::from_hash_of(("dialog", id));
    let key = Id::new(("dialog_position", vid));
    let size = Vec2::from(size);
    let position = ctx.data(|d| d.get_temp::<Pos2>(key)).unwrap_or_else(|| {
        let (outer, monitor) = ctx.input(|i| (i.viewport().outer_rect, i.viewport().monitor_size));
        let area = outer
            .filter(|r| r.width() > 120. && r.min.x > -10000. && r.min.y > -10000.)
            .or_else(|| monitor.map(|m| Rect::from_min_size(Pos2::ZERO, m)))
            .unwrap_or(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280., 800.)));
        let p = area.center() - size * 0.5;
        let p = Pos2::new(p.x.max(area.left()), p.y.max(area.top()));
        ctx.data_mut(|d| d.insert_temp(key, p));
        p
    });
    let builder = ViewportBuilder::default()
        .with_title(title)
        .with_inner_size(size)
        .with_position(position)
        .with_min_inner_size([220., 120.])
        .with_taskbar(false)
        .with_window_level(if on_top {
            egui::WindowLevel::AlwaysOnTop
        } else {
            egui::WindowLevel::Normal
        });
    let mut add = Some(add);
    let mut out = None;
    let mut close = false;
    let raise = ctx.data_mut(|d| d.remove_temp::<bool>(raise_key(vid)).unwrap_or(false));
    ctx.show_viewport_immediate(vid, builder, |ctx, class| {
        crate::tutorial::mark(ctx, crate::tutorial::Spot::Popup, ctx.screen_rect());
        if raise {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        if class == ViewportClass::Embedded {
            let mut keep = true;
            egui::Window::new(title)
                .id(Id::new(("embedded_dialog", vid)))
                .open(&mut keep)
                .default_width(size.x)
                .show(ctx, |ui| {
                    if let Some(f) = add.take() {
                        out = Some(f(ui));
                    }
                });
            close = !keep;
            return;
        }
        let fill = ctx.style().visuals.window_fill;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(fill)
                    .inner_margin(egui::Margin::same(20)),
            )
            .show(ctx, |ui| {
                if let Some(f) = add.take() {
                    out = Some(f(ui));
                }
            });
        if ctx.input(|i| i.viewport().close_requested())
            || ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            close = true;
        }
    });
    if close {
        *open = false;
    }
    if !*open {
        ctx.data_mut(|d| d.remove::<Pos2>(key));
    }
    out
}

/// Rounded search field with a magnifier icon.
pub fn search_field(
    ui: &mut Ui,
    text: &mut String,
    id: impl Hash,
    hint: &str,
    width: f32,
) -> Response {
    let c = t(ui);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 30.), Sense::hover());
    let edit_id = Id::new(("search_field", id));
    let focused = ui.memory(|m| m.has_focus(edit_id));
    ui.painter().rect_filled(rect, 7., c.field);
    if focused {
        ui.painter().rect_stroke(
            rect,
            7.,
            Stroke::new(1.0_f32, accent(ui).gamma_multiply(0.7)),
            StrokeKind::Inside,
        );
    }
    paint_icon(
        ui.painter(),
        Rect::from_center_size(rect.left_center() + Vec2::new(15., 0.), Vec2::splat(15.)),
        Icon::Search,
        c.muted,
    );
    let inner = Rect::from_min_max(rect.min + Vec2::new(26., 0.), rect.max - Vec2::new(6., 0.));
    ui.put(
        inner,
        egui::TextEdit::singleline(text)
            .id(edit_id)
            .frame(false)
            .vertical_align(egui::Align::Center)
            .hint_text(hint)
            .desired_width(inner.width()),
    )
}

/// Two buttons aligned right at the bottom of a dialog. Returns (confirm, cancel).
pub fn dialog_buttons(ui: &mut Ui, confirm: &str, cancel: &str) -> (bool, bool) {
    let mut out = (false, false);
    ui.add_space(8.);
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        out.0 = primary(ui, confirm).clicked();
        if !cancel.is_empty() {
            out.1 = ui.button(cancel).clicked();
        }
    });
    out
}

/// Label + control on one aligned row.
pub fn field(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        let c = t(ui).muted;
        ui.add_sized([96., 26.], egui::Label::new(RichText::new(label).color(c)));
        add(ui);
    });
}

/// A compact month grid used by the date and alarm dialogs.
pub fn month_grid(ui: &mut Ui, month: &mut chrono::NaiveDate, day: &mut chrono::NaiveDate) {
    use chrono::{Datelike, Duration};
    let c = t(ui);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(month.format("%Y년 %-m월").to_string())
                .strong()
                .size(15.),
        );
        ui.add_space(4.);
        if crate::icons::icon_button(ui, Icon::ChevronLeft, "이전 달").clicked() {
            *month = (*month - Duration::days(1)).with_day(1).unwrap();
        }
        if crate::icons::icon_button(ui, Icon::ChevronRight, "다음 달").clicked() {
            *month = (*month + Duration::days(32)).with_day(1).unwrap();
        }
    });
    ui.add_space(4.);
    // The grid spans the window width; rows keep a compact height.
    let cell_w = (ui.available_width() / 7.).max(34.);
    let cell_h = 34.;
    let (head, _) = ui.allocate_exact_size(Vec2::new(cell_w * 7., 20.), Sense::hover());
    for (i, s) in ["일", "월", "화", "수", "목", "금", "토"]
        .iter()
        .enumerate()
    {
        ui.painter().text(
            Pos2::new(head.left() + cell_w * (i as f32 + 0.5), head.center().y),
            egui::Align2::CENTER_CENTER,
            *s,
            FontId::proportional(11.5),
            match i {
                0 => accent(ui),
                6 => blue(ui),
                _ => c.faint,
            },
        );
    }
    let offset = month.weekday().num_days_from_sunday() as i64;
    let today = chrono::Local::now().date_naive();
    let (grid, _) = ui.allocate_exact_size(Vec2::new(cell_w * 7., cell_h * 6.), Sense::hover());
    for slot in 0..42 {
        let d = *month + Duration::days(slot - offset);
        if d.month() != month.month() {
            continue;
        }
        let r = Rect::from_min_size(
            grid.min + Vec2::new((slot % 7) as f32 * cell_w, (slot / 7) as f32 * cell_h),
            Vec2::new(cell_w, cell_h),
        )
        .shrink(2.);
        let resp = ui
            .interact(r, ui.id().with(("month_grid", d)), Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let selected = *day == d;
        if selected {
            ui.painter().circle_filled(r.center(), 14., accent(ui));
        } else if resp.hovered() {
            ui.painter().circle_filled(r.center(), 14., c.hover);
        } else if d == today {
            ui.painter()
                .circle_stroke(r.center(), 13.5, Stroke::new(1.0_f32, accent(ui)));
        }
        let wd = d.weekday().num_days_from_sunday();
        let holiday = crate::holidays::name(d);
        if let Some(name) = &holiday {
            resp.clone().on_hover_text(name.as_str());
        }
        ui.painter().text(
            r.center(),
            egui::Align2::CENTER_CENTER,
            d.day().to_string(),
            FontId::proportional(13.),
            if selected {
                Color32::WHITE
            } else if wd == 0 || holiday.is_some() {
                accent(ui)
            } else if wd == 6 || crate::holidays::is_blue_friday(d) {
                blue(ui)
            } else {
                c.ink
            },
        );
        if resp.clicked() {
            *day = d;
        }
    }
}

/// Hour and minute steppers in one row.
pub fn time_row(ui: &mut Ui, hour: &mut u32, minute: &mut u32) {
    ui.horizontal(|ui| {
        let c = t(ui).muted;
        ui.label(RichText::new("시간").color(c));
        ui.add_space(8.);
        time_input(ui, "time_row", hour, minute);
    });
}

/// Step a time by `delta` minutes, wrapping around midnight.
pub fn step_minutes(hour: u32, minute: u32, delta: i32) -> (u32, u32) {
    let total = (hour as i32 * 60 + minute as i32 + delta).rem_euclid(24 * 60);
    ((total / 60) as u32, (total % 60) as u32)
}

/// Hour and minute boxes. A click opens a picker of whole hours and
/// 5-minute steps; the arrows beside the minutes step by one minute.
pub fn time_input(ui: &mut Ui, id: impl Hash, hour: &mut u32, minute: &mut u32) {
    let c = t(ui);
    let popup = ui.make_persistent_id(("time_input", id));
    let height = 28.;
    ui.spacing_mut().item_spacing.x = 0.;
    let field = |ui: &mut Ui, text: String| {
        let (rect, r) = ui.allocate_exact_size(Vec2::new(38., height), Sense::click());
        let fill = if r.hovered() { c.hover } else { c.field };
        ui.painter().rect_filled(rect, 6., fill);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            FontId::monospace(14.),
            c.ink,
        );
        r.on_hover_cursor(egui::CursorIcon::PointingHand)
    };
    let h = field(ui, format!("{:02}", *hour));
    ui.add_sized([12., height], egui::Label::new(RichText::new(":").strong()));
    let m = field(ui, format!("{:02}", *minute));
    // Two half-height arrows stuck to the minute box: ±1 minute.
    let (spin, _) = ui.allocate_exact_size(Vec2::new(18., height), Sense::hover());
    for (k, up) in [(0., true), (1., false)] {
        let r = Rect::from_min_size(
            spin.min + Vec2::new(1., k * height / 2.),
            Vec2::new(17., height / 2.),
        );
        let resp = ui
            .interact(r, popup.with(("spin", up)), Sense::click())
            .on_hover_text(if up {
                "1분 늘리기"
            } else {
                "1분 줄이기"
            });
        ui.painter().rect_filled(
            r.shrink(0.5),
            3.,
            if resp.hovered() { c.hover } else { c.field },
        );
        crate::icons::paint_icon(
            ui.painter(),
            Rect::from_center_size(r.center(), Vec2::splat(9.)),
            if up {
                Icon::ChevronUp
            } else {
                Icon::ChevronDown
            },
            c.muted,
        );
        if resp.clicked() {
            (*hour, *minute) = step_minutes(*hour, *minute, if up { 1 } else { -1 });
        }
    }
    if h.clicked() || m.clicked() {
        ui.memory_mut(|mem| mem.toggle_popup(popup));
    }
    let anchor = h.union(m);
    crate::float::popup(ui, popup, &anchor, 6. * 38. + 14., |ui| {
        ui.set_width(6. * 38.);
        ui.spacing_mut().item_spacing = Vec2::new(2., 2.);
        caption(ui, "시");
        let mut close = false;
        egui::Grid::new(popup.with("hours"))
            .spacing([2., 2.])
            .show(ui, |ui| {
                for v in 0..24u32 {
                    if pick_cell(ui, v == *hour, format!("{v:02}")).clicked() {
                        *hour = v;
                    }
                    if v % 6 == 5 {
                        ui.end_row();
                    }
                }
            });
        ui.add_space(6.);
        caption(ui, "분");
        egui::Grid::new(popup.with("minutes"))
            .spacing([2., 2.])
            .show(ui, |ui| {
                for k in 0..12u32 {
                    let v = k * 5;
                    if pick_cell(ui, v == *minute, format!("{v:02}")).clicked() {
                        *minute = v;
                        close = true;
                    }
                    if k % 6 == 5 {
                        ui.end_row();
                    }
                }
            });
        if close {
            ui.memory_mut(|mem| mem.close_popup());
        }
    });
}
fn pick_cell(ui: &mut Ui, selected: bool, text: String) -> Response {
    let c = t(ui);
    let (rect, r) = ui.allocate_exact_size(Vec2::new(36., 26.), Sense::click());
    let fill = if selected {
        accent(ui)
    } else if r.hovered() {
        c.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 6., fill);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        FontId::monospace(13.),
        if selected { Color32::WHITE } else { c.ink },
    );
    r.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The OhMemoJang banner picture (`assets/banner.png`).
const BANNER_PNG: &[u8] = include_bytes!("../assets/banner.png");
/// Width / height of the banner picture, kept whatever the width it is drawn at.
pub fn banner_ratio() -> f32 {
    banner_size().map_or(2., |[w, h]| w as f32 / h as f32)
}
fn banner_size() -> Option<[usize; 2]> {
    use image::ImageDecoder;
    let d = image::codecs::png::PngDecoder::new(std::io::Cursor::new(BANNER_PNG)).ok()?;
    let (w, h) = d.dimensions();
    Some([w as usize, h as usize])
}
/// The banner, `width` wide and as high as the picture's proportions make it.
pub fn banner(ui: &mut Ui, width: f32) -> Response {
    let size = Vec2::new(width, width / banner_ratio());
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    let key = Id::new("ohmemojang_banner_texture");
    let texture = ui
        .ctx()
        .data_mut(|d| d.get_temp::<egui::TextureHandle>(key));
    let texture = texture.or_else(|| {
        let img = image::load_from_memory(BANNER_PNG).ok()?.to_rgba8();
        let color = egui::ColorImage::from_rgba_unmultiplied(
            [img.width() as usize, img.height() as usize],
            img.as_raw(),
        );
        let t = ui
            .ctx()
            .load_texture("ohmemojang_banner", color, egui::TextureOptions::LINEAR);
        ui.ctx().data_mut(|d| d.insert_temp(key, t.clone()));
        Some(t)
    });
    if let Some(t) = texture {
        ui.painter().image(
            t.id(),
            rect,
            egui::Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
            Color32::WHITE,
        );
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_design_switch_updates_palette_and_accent() {
        let ctx = egui::Context::default();
        for viewport in [
            crate::main_viewport(),
            egui::ViewportId::from_hash_of("calendar"),
        ] {
            for variant in [0, 1, 2, 0] {
                let mut input = egui::RawInput {
                    viewport_id: viewport,
                    ..Default::default()
                };
                input.viewports.entry(viewport).or_default();
                let _ = ctx.run(input, |ctx| {
                    apply_light(ctx, variant, egui::FontFamily::Proportional, 14.);
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let expected = if viewport != crate::main_viewport() && variant == 0 {
                            1
                        } else {
                            variant
                        };
                        assert_eq!(super::variant(ui), variant);
                        assert_eq!(paper(ui), expected == 0);
                        assert_eq!(t(ui).bg, light_tone(expected).bg);
                        assert_eq!(accent(ui), accent_for(expected));
                        assert_eq!(ui.visuals().selection.stroke.color, accent_for(expected));
                    });
                });
            }
        }
    }

    #[test]
    fn the_banner_keeps_the_pictures_proportions_at_any_width() {
        let ctx = egui::Context::default();
        let ratio = banner_ratio();
        assert!(
            (ratio - 2.0).abs() < 0.01,
            "the picture is twice as wide as high: {ratio}"
        );
        for width in [120., 190., 400.] {
            let mut size = Vec2::ZERO;
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    size = banner(ui, width).rect.size();
                });
            });
            assert!((size.x / size.y - ratio).abs() < 0.01, "{size:?}");
        }
    }

    #[test]
    fn sweeping_over_a_toolbar_pops_no_tooltips_until_the_pointer_rests() {
        let ctx = egui::Context::default();
        apply(&ctx, false, false, egui::FontFamily::Proportional, 14.);
        let i = ctx.style().interaction.clone();
        assert!(!i.show_tooltips_only_when_still);
        assert_eq!(i.tooltip_grace_time, 0.);
        assert!(i.tooltip_delay > 0.);
        // Two buttons side by side; (rect, tooltip open) of each.
        let frame = |ctx: &egui::Context, input: egui::RawInput| {
            let mut seen = Vec::new();
            let _ = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        for label in ["a", "b"] {
                            let r = ui.button(label).on_hover_text("tip");
                            seen.push((r.rect, r.is_tooltip_open()));
                        }
                    });
                });
            });
            seen
        };
        let at = |t: f64, p: Option<egui::Pos2>| egui::RawInput {
            time: Some(t),
            events: p.map(egui::Event::PointerMoved).into_iter().collect(),
            ..Default::default()
        };
        let mut seen = frame(&ctx, at(0., None));
        let (a, b) = (seen[0].0.center(), seen[1].0.center());
        // Resting on "a" shows its tooltip.
        let mut t = 1.;
        let _ = frame(&ctx, at(t, Some(a)));
        while t < 2. {
            t += 0.05;
            seen = frame(&ctx, at(t, None));
        }
        assert!(seen[0].1, "the tooltip of a resting pointer shows");
        // Passing on to "b" does not show its tooltip right away.
        let _ = frame(&ctx, at(t + 0.016, Some(b)));
        for k in 2..6 {
            seen = frame(&ctx, at(t + 0.016 * f64::from(k), None));
            assert!(!seen[1].1, "no tooltip while passing over");
        }
        // It shows once the pointer has rested there.
        let mut u = t + 0.1;
        while u < t + 1.2 {
            u += 0.05;
            seen = frame(&ctx, at(u, None));
        }
        assert!(seen[1].1);
    }
}
