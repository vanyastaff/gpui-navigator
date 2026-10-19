//! Property tests for path matching: whatever the values, building a path from
//! a pattern and matching it back gives the same parameters, query strings
//! never affect matching, and the match stack agrees with `Route::matches`.

#[cfg(all(
    feature = "gpui-fast",
    not(feature = "gpui-02"),
    not(feature = "gpui-pre")
))]
extern crate gpui_fast as gpui;
#[cfg(all(feature = "gpui-pre", not(feature = "gpui-02")))]
extern crate gpui_pre as gpui;

use gpui::{div, IntoElement};
use gpui_navigator::*;
use proptest::prelude::*;
use std::sync::Arc;

fn segment() -> impl Strategy<Value = String> {
    "[a-zA-Z0-9_-]{1,12}"
}

fn query() -> impl Strategy<Value = String> {
    "[a-z]{1,5}=[a-z0-9]{0,5}(&[a-z]{1,5}=[a-z0-9]{0,5}){0,3}"
}

fn page(path: &str) -> Arc<Route> {
    Arc::new(Route::view(path, || div().into_any_element()))
}

proptest! {
    #[test]
    fn url_for_round_trips_through_matching(user in segment(), post in segment(), rest in proptest::collection::vec(segment(), 0..4)) {
        let mut registry = NamedRouteRegistry::new();
        registry.register("post", "/users/:user/posts/:post/*rest");

        let mut params = RouteParams::new();
        params.insert("user", user.clone());
        params.insert("post", post.clone());
        params.insert("rest", rest.join("/"));

        let joined = rest.join("/");
        let url = registry.url_for("post", &params).unwrap();
        let matched = RouteParams::from_path(&url, "/users/:user/posts/:post/*rest");
        prop_assert_eq!(matched.get("user"), Some(&user));
        prop_assert_eq!(matched.get("post"), Some(&post));
        prop_assert_eq!(matched.get("rest"), Some(&joined));
    }

    #[test]
    fn query_strings_never_change_the_match(id in segment(), q in query()) {
        let routes = vec![page("/"), page("/items/:id")];
        let plain = resolve_match_stack(&routes, &format!("/items/{id}"));
        let with_query = resolve_match_stack(&routes, &format!("/items/{id}?{q}#frag"));
        prop_assert_eq!(plain.len(), 1);
        prop_assert_eq!(with_query.len(), 1);
        prop_assert_eq!(plain.params(), with_query.params());
        prop_assert_eq!(QueryParams::from_location(&format!("/x?{q}")), QueryParams::from_query_string(&q));
    }

    #[test]
    fn normalize_path_is_idempotent(segs in proptest::collection::vec(segment(), 0..5), lead in any::<bool>(), trail in any::<bool>()) {
        let mut path = segs.join("/");
        if lead { path.insert(0, '/'); }
        if trail { path.push('/'); }
        let once = normalize_path(&path).into_owned();
        prop_assert!(once.starts_with('/'));
        prop_assert_eq!(normalize_path(&once).into_owned(), once);
    }

    #[test]
    fn match_stack_agrees_with_route_matches(segs in proptest::collection::vec(segment(), 0..4)) {
        let patterns = ["/", "/a/:x", "/a/:x/b", "/:x/:y", "/files/*rest", "*"];
        let routes: Vec<_> = patterns.iter().map(|p| page(p)).collect();
        let path = format!("/{}", segs.join("/"));
        let stack = resolve_match_stack(&routes, &path);
        let first = routes.iter().find(|r| r.matches(&path).is_some());
        prop_assert_eq!(
            stack.leaf().map(|e| e.route.config.path.clone()),
            first.map(|r| r.config.path.clone())
        );
    }
}
