//! Frame cost of the heavy pages with many memos. Run by hand:
//! `cargo test --release perf_bench -- --ignored --nocapture`
use super::*;
use std::time::Instant;

fn busy_app(count: usize) -> App {
    let today = NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
    let mut a = App::sample(today);
    let template = a.data.notes[0].clone();
    a.extra.wiki_tabs = vec!["하나".into(), "둘".into()];
    for k in 0..count {
        let mut n = template.clone();
        n.title = format!("메모 {k} 제목입니다");
        n.body = "본문 내용입니다. 여러 줄\n두 번째 줄 ".repeat(8);
        n.date = Some(today + chrono::Duration::days((k % 60) as i64 - 20));
        n.created = today.and_hms_opt(9, 0, 0).unwrap() - chrono::Duration::minutes(k as i64);
        n.wiki = k % 3 == 0;
        n.wiki_tab = if n.wiki { Some(k % 2) } else { None };
        n.project = Some(k % 2);
        n.deleted = false;
        a.data.notes.push(n);
    }
    a
}

fn frames(label: &str, a: &mut App, n: usize, mut show: impl FnMut(&mut App, &mut egui::Ui)) {
    let ctx = egui::Context::default();
    if let Ok(bytes) = std::fs::read("C:/Windows/Fonts/malgun.ttf") {
        let mut fonts = egui::FontDefinitions::default();
        fonts
            .font_data
            .insert("malgun".into(), egui::FontData::from_owned(bytes).into());
        fonts
            .families
            .get_mut(&egui::FontFamily::Proportional)
            .unwrap()
            .insert(0, "malgun".into());
        ctx.set_fonts(fonts);
    }
    crate::ui::apply(&ctx, false, false, egui::FontFamily::Proportional, 14.);
    let mut run = |a: &mut App, moved: bool| {
        let events = if moved {
            vec![egui::Event::PointerMoved(egui::pos2(
                300. + (a.data.notes.len() % 7) as f32,
                300.,
            ))]
        } else {
            vec![]
        };
        let out = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200., 800.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| show(a, ui));
            },
        );
        let t = Instant::now();
        let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
        (
            t.elapsed(),
            prims
                .iter()
                .map(|p| match &p.primitive {
                    egui::epaint::Primitive::Mesh(m) => m.vertices.len(),
                    _ => 0,
                })
                .sum::<usize>(),
        )
    };
    for _ in 0..3 {
        run(a, false);
    }
    let mut samples = Vec::with_capacity(n);
    let mut tess = std::time::Duration::ZERO;
    let mut prims = 0;
    for i in 0..n {
        let start = Instant::now();
        let (t, p) = run(a, i % 2 == 0);
        samples.push(start.elapsed().as_secs_f64() * 1000.);
        tess += t;
        prims = p;
    }
    samples.sort_by(f64::total_cmp);
    let p95 = samples[n.saturating_mul(95).div_ceil(100).saturating_sub(1)];
    println!(
        "{label}: p50 {:.2}, p95 {p95:.2} ms/frame (tessellate {:.2}, {prims} vertices)",
        samples[n / 2],
        tess.as_secs_f64() * 1000. / n as f64
    );
}

#[test]
#[ignore]
fn catalog_large_frame_costs() {
    let baseline = std::env::var_os("OHMEMOJANG_PERF_BASELINE").is_some();
    for count in [1500, 20_000] {
        let mut a = busy_app(count);
        a.page = 4;
        a.catalog.tab = crate::catalog::FOLDER_TAB;
        frames(&format!("folders {count}"), &mut a, 40, |a, ui| {
            a.catalog_ui(ui)
        });
        let mut a = busy_app(count);
        a.page = 4;
        a.catalog.tab = crate::catalog::ALL_MEMOS_TAB;
        frames(&format!("all memos {count}"), &mut a, 40, |a, ui| {
            a.catalog_ui(ui)
        });
        if !baseline {
            let mut a = busy_app(count);
            a.page = 4;
            a.catalog.tab = crate::catalog::PROJECT_TAB;
            frames(&format!("projects {count}"), &mut a, 40, |a, ui| {
                a.catalog_ui(ui)
            });
        }
    }
}

#[test]
#[ignore]
fn page_frame_costs() {
    for count in [50, 1500] {
        println!("--- {count} memos");
        let mut a = busy_app(count);
        frames("wiki", &mut a, 60, |a, ui| a.wiki(ui));
        let mut a = busy_app(count);
        frames("calendar grid", &mut a, 60, |a, ui| {
            a.calendar_grid(ui);
        });
        let mut a = busy_app(count);
        frames("calendar day detail", &mut a, 60, |a, ui| {
            a.calendar_day_detail(ui)
        });
    }
}

#[test]
#[ignore]
fn wiki_with_open_memos() {
    let mut a = busy_app(1500);
    let wikis: Vec<usize> = (0..a.data.notes.len())
        .filter(|&i| a.data.notes[i].wiki)
        .collect();
    a.extra.wiki_panes = [Some(wikis[0]), Some(wikis[1])];
    a.extra.wiki_blank = false;
    frames("wiki, two memos open", &mut a, 60, |a, ui| a.wiki(ui));
}
