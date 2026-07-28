// SPDX-License-Identifier: MPL-2.0

use super::{WorkloadResult, require, usize_to_u64};
use crate::report::WorkloadReport;
use luna_text::TextDocument;
use luna_text_cosmic::{TextEngine, TextLayoutCache, TextLayoutRequest};
use luna_theme::Theme;

const WORKLOAD: &str = "large_text_cache";
const LINE_COUNT: u32 = 4_096;
const VIEWPORT_HEIGHT: u32 = 720;
const MAXIMUM_LAYOUT_MISSES: u64 = 1;

pub(super) fn run(cycles: u32) -> WorkloadResult<WorkloadReport> {
    let text = large_document_text();
    let document = TextDocument::new(text);
    let theme = Theme::luna_dark();
    let request = TextLayoutRequest::new(960, 14.0, 20.0, theme.foreground);
    let mut engine = TextEngine::new();
    let mut cache = TextLayoutCache::new().with_overscan_viewports(1);
    let scroll_positions = [0_i32, 160, 640, 2_400, 9_600, 32_000, 64_000, 0];
    let mut maximum_content_height = 0_u64;
    let mut maximum_content_width = 0_u64;

    for _cycle in 0..cycles {
        for scroll_y in scroll_positions {
            let snapshot = cache.update(
                &mut engine,
                &document,
                1,
                request,
                scroll_y,
                VIEWPORT_HEIGHT,
            )?;
            let size = snapshot.content_size();
            maximum_content_width = maximum_content_width.max(u64::from(size.width));
            maximum_content_height = maximum_content_height.max(u64::from(size.height));
        }
    }

    let stats = cache.stats();
    let samples = u64::from(cycles).saturating_mul(usize_to_u64(scroll_positions.len()));
    require(
        WORKLOAD,
        document.line_count() == usize::try_from(LINE_COUNT).unwrap_or(usize::MAX),
        "large document line count changed",
    )?;
    require(
        WORKLOAD,
        stats.layout_misses <= MAXIMUM_LAYOUT_MISSES,
        format!(
            "layout misses exceeded {MAXIMUM_LAYOUT_MISSES}: {}",
            stats.layout_misses
        ),
    )?;
    require(
        WORKLOAD,
        stats.layout_hits.saturating_add(stats.layout_misses) == samples,
        "every cache sample must be classified as a layout hit or miss",
    )?;
    require(
        WORKLOAD,
        stats.raster_hits > 0,
        "repeated nearby scrolling must reuse at least one retained raster band",
    )?;
    require(
        WORKLOAD,
        stats.raster_misses < samples,
        "every raster sample missed; retained raster reuse is absent",
    )?;
    require(
        WORKLOAD,
        maximum_content_height > u64::from(VIEWPORT_HEIGHT),
        "large document must exceed one viewport",
    )?;

    let mut report = WorkloadReport::new(WORKLOAD);
    report.record("cycles", u64::from(cycles));
    report.record("samples", samples);
    report.record("document_bytes", usize_to_u64(document.text().len()));
    report.record("document_lines", usize_to_u64(document.line_count()));
    report.record("viewport_height", u64::from(VIEWPORT_HEIGHT));
    report.record("layout_hits", stats.layout_hits);
    report.record("layout_misses", stats.layout_misses);
    report.record("raster_hits", stats.raster_hits);
    report.record("raster_misses", stats.raster_misses);
    report.record("maximum_content_width", maximum_content_width);
    report.record("maximum_content_height", maximum_content_height);
    report.limit("layout_misses", MAXIMUM_LAYOUT_MISSES);
    report.limit("viewport_height", u64::from(VIEWPORT_HEIGHT));
    Ok(report)
}

fn large_document_text() -> String {
    let mut text = String::new();
    for line in 0..LINE_COUNT {
        if line != 0 {
            text.push('\n');
        }
        text.push_str(&format!(
            "line {line:04}: Luna M8.3 cache reuse — ffi مرحباً नमस्ते こんにちは 🌙"
        ));
    }
    text
}
