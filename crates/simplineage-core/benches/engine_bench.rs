//! Baseline Criterion benches (expand with real graph work later).

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use simplineage_core::{Engine, Settings};

fn engine_status(c: &mut Criterion) {
    let engine = Engine::new(Settings::default());
    c.bench_function("engine_status", |b| {
        b.iter(|| black_box(engine.status()));
    });
}

fn settings_default(c: &mut Criterion) {
    c.bench_function("settings_default", |b| {
        b.iter(|| black_box(Settings::default()));
    });
}

criterion_group!(benches, engine_status, settings_default);
criterion_main!(benches);
