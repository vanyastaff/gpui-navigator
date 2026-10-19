//! Route resolution timing on route tables of growing size.
//!
//! Run with `cargo bench --bench resolve`. Uses only `std`, so it adds no
//! dependencies; numbers are rough wall-clock averages.

#[cfg(all(
    feature = "gpui-fast",
    not(feature = "gpui-02"),
    not(feature = "gpui-pre")
))]
extern crate gpui_fast as gpui;
#[cfg(all(feature = "gpui-pre", not(feature = "gpui-02")))]
extern crate gpui_pre as gpui;

use gpui::{div, IntoElement};
use gpui_navigator::{resolve_match_stack, Route};
use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

fn page(path: &str) -> Arc<Route> {
    Arc::new(Route::view(path, || div().into_any_element()))
}

/// `n` sections, each with a list, a detail page and a nested settings tree.
fn table(n: usize) -> Vec<Arc<Route>> {
    let mut routes = vec![page("/")];
    for i in 0..n {
        routes.push(Arc::new(
            Route::view(format!("/section{i}"), || div().into_any_element()).children(vec![
                page(""),
                page(":id<int>"),
                Arc::new(
                    Route::view("settings", || div().into_any_element())
                        .children(vec![page("profile"), page("security")]),
                ),
            ]),
        ));
    }
    routes.push(page("*"));
    routes
}

fn bench(label: &str, routes: &[Arc<Route>], path: &str) {
    const ITERS: u32 = 20_000;
    // Warm up
    for _ in 0..1_000 {
        black_box(resolve_match_stack(routes, path));
    }
    let start = Instant::now();
    for _ in 0..ITERS {
        black_box(resolve_match_stack(black_box(routes), black_box(path)));
    }
    let per_iter = start.elapsed() / ITERS;
    println!("{label:<40} {per_iter:>10.2?}");
}

fn main() {
    for n in [10, 100, 500] {
        let routes = table(n);
        let last = n - 1;
        bench(
            &format!("{n} sections, first section leaf"),
            &routes,
            "/section0/settings/profile",
        );
        bench(
            &format!("{n} sections, last section leaf"),
            &routes,
            &format!("/section{last}/settings/profile"),
        );
        bench(
            &format!("{n} sections, param + query"),
            &routes,
            &format!("/section{last}/42?tab=1"),
        );
        bench(
            &format!("{n} sections, not found (wildcard)"),
            &routes,
            "/nope/nope",
        );
    }
}
