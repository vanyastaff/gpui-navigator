# gpui-navigator: Nested Routing Architecture Rework

## Проблема

Текущая архитектура nested routing ломается на глубокой вложенности из-за фундаментальной ошибки проектирования: **каждый outlet независимо ищет себя в дереве при каждом рендере**.

### Текущий flow (сломанный)

```
Navigation: push("/app/workspace/abc/project/xyz")
  │
  ├─ RouterView::render()
  │    └─ state.current_route_for_rendering()     ← O(n) поиск top-level route
  │         └─ рендерит Route("/") builder
  │              │
  │              ├─ RouterOutlet::render() [depth 1]
  │              │    ├─ find_parent_route_for_path()    ← 140 строк рекурсии
  │              │    │   (кто мой родитель? → /)
  │              │    └─ resolve_child_route()            ← ещё рекурсия
  │              │        (какой child? → app)
  │              │         └─ рендерит Route("app") builder
  │              │              │
  │              │              ├─ RouterOutlet::render() [depth 2]
  │              │              │    ├─ find_parent_route_for_path() ← ОПЯТЬ ищет с корня!
  │              │              │    │   (проблема: не знает что depth=2,
  │              │              │    │    ищет deepest parent заново)
  │              │              │    └─ resolve_child_route()
  │              │              │        ← тут ломается на :id параметрах
```

**Почему ломается:**
1. `find_parent_route_for_path` ищет "deepest parent with children" — но для `/app/workspace/abc/project/xyz` может вернуть `workspace/:id` вместо `app`
2. Каждый outlet не знает свою глубину — все начинают поиск с корня
3. Parameter routes (`:id`) ломают string prefix matching (`starts_with`)
4. Одни и те же пути нормализуются 5-6 раз за один render

### Новый flow (MatchStack)

```
Navigation: push("/app/workspace/abc/project/xyz")
  │
  ├─ resolve_match_stack() ← ОДИН раз, при навигации
  │    Result: MatchStack [
  │      [0] Route("/")              params={}
  │      [1] Route("app")            params={}
  │      [2] Route("workspace/:id")  params={id: "abc"}
  │      [3] Route("project/:pid")   params={id: "abc", pid: "xyz"}
  │    ]
  │
  ├─ RouterView::render()
  │    └─ reset_outlet_depth()  → depth = 0
  │    └─ match_stack[0]        → Route("/"), рендерит builder
  │         │
  │         ├─ RouterOutlet::render()
  │         │    └─ claim_outlet_depth() → depth = 1
  │         │    └─ match_stack[1]       → Route("app"), O(1) lookup!
  │         │         │
  │         │         ├─ RouterOutlet::render()
  │         │         │    └─ claim_outlet_depth() → depth = 2
  │         │         │    └─ match_stack[2]       → Route("workspace/:id")
  │         │         │         params = {id: "abc"}
  │         │         │         │
  │         │         │         ├─ RouterOutlet::render()
  │         │         │         │    └─ claim_outlet_depth() → depth = 3
  │         │         │         │    └─ match_stack[3]       → Route("project/:pid")
  │         │         │         │         params = {id: "abc", pid: "xyz"}
  │         │         │         │         ✅ Leaf — рендерит контент
```

## Архитектура

### Новый модуль: `resolve.rs`

Содержит три компонента:

#### 1. `MatchStack` — результат резолва

```rust
pub struct MatchStack {
    entries: Vec<MatchEntry>,
}

pub struct MatchEntry {
    pub route: Arc<Route>,    // Matched route
    pub params: RouteParams,  // Accumulated params (parent + own)
    pub depth: usize,         // Position in hierarchy
}
```

#### 2. `resolve_match_stack()` — алгоритм резолва

Вызывается **один раз при навигации** (не при каждом рендере!).

Алгоритм:
1. Split path на сегменты: `/app/workspace/abc` → `["app", "workspace", "abc"]`
2. Для каждого top-level route, попробовать match первых сегментов
3. При match — consume сегменты, push в stack, recurse в children
4. При exhaustion сегментов — попробовать index route (empty path child)
5. Backtracking: если children не совпали, pop из stack и пробуем следующий route

Поддерживает:
- Static segments: `dashboard`, `settings`
- Parameter segments: `:id`, `:userId`
- Multi-segment routes: `api/v1`
- Layout routes: empty path `""` with children
- Index routes: empty path without children
- Deep nesting: до 16 уровней
- Backtracking: если первый match не ведёт к полному резолву

#### 3. Depth tracking — thread-local counter

```rust
thread_local! {
    static OUTLET_DEPTH: Cell<usize> = Cell::new(0);
}

pub fn reset_outlet_depth()      // RouterView вызывает при рендере
pub fn claim_outlet_depth() -> usize  // Outlet вызывает, получает свой depth
pub fn set_outlet_depth(depth)   // Восстановление после рендера child builder
```

### Изменения в `GlobalRouter` (context.rs)

```rust
pub struct GlobalRouter {
    state: RouterState,
    match_stack: MatchStack,  // ← НОВОЕ: кэш текущего резолва
    // ... existing fields
}

impl GlobalRouter {
    pub fn push(&mut self, path: String) -> RouteChangeEvent {
        let event = self.state.push(path);
        // Резолвим match stack СРАЗУ при навигации
        self.match_stack = resolve_match_stack(
            self.state.routes(),
            self.state.current_path(),
        );
        event
    }

    // Аналогично для replace(), back(), forward()

    /// Outlet читает свой entry из stack
    pub fn match_stack(&self) -> &MatchStack {
        &self.match_stack
    }
}
```

### Изменения в `RouterOutlet` (widgets.rs)

**БЫЛО** (200+ строк с RefCell):
```rust
impl Render for RouterOutlet {
    fn render(&mut self, ...) {
        let router = cx.try_global::<GlobalRouter>()?;
        let parent = find_parent_route_for_path(routes, path);  // 140 lines
        let (child, params) = resolve_child_route(parent, path); // recursive
        // RefCell state management, transition tracking...
        child.build(window, cx, &params)
    }
}
```

**СТАЛО** (~30 строк, без RefCell):
```rust
impl Render for RouterOutlet {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let router = cx.try_global::<GlobalRouter>();
        let Some(router) = router else {
            return div().into_any_element();
        };

        let depth = claim_outlet_depth();
        let stack = router.match_stack();

        let Some(entry) = stack.at_depth(depth) else {
            // No route at this depth — render nothing or 404
            return div().into_any_element();
        };

        // Save depth so nested outlets inside this builder get depth+1
        let saved_depth = current_outlet_depth();

        let element = if let Some(builder) = &entry.route.builder {
            set_outlet_depth(depth); // nested outlets will claim depth+1
            let result = builder(window, cx, &entry.params);
            set_outlet_depth(saved_depth); // restore for siblings
            result
        } else {
            div().child("Route has no builder").into_any_element()
        };

        element
    }
}
```

### Изменения в `render_router_outlet()` (widgets.rs)

**БЫЛО:**
```rust
pub fn render_router_outlet(window, cx, name) -> AnyElement {
    let router = cx.try_global::<GlobalRouter>()?;
    let parent = find_parent_route_for_path(...);
    let (child, params) = resolve_child_route(parent, path, ...)?;
    child.build(window, cx, &params)
}
```

**СТАЛО:**
```rust
pub fn render_router_outlet(
    window: &mut Window,
    cx: &mut App,
    name: Option<&str>,
) -> AnyElement {
    let router = cx.try_global::<GlobalRouter>();
    let Some(router) = router else {
        return div().into_any_element();
    };

    let stack = router.match_stack();

    if let Some(name) = name {
        // Named outlet: resolve from parent entry
        let depth = current_outlet_depth();
        if let Some((route, params)) = resolve_named_outlet(
            stack, depth, name, router.current_path()
        ) {
            if let Some(builder) = &route.builder {
                return builder(window, cx, &params);
            }
        }
        return div().into_any_element();
    }

    // Default outlet: claim next depth
    let depth = claim_outlet_depth();

    let Some(entry) = stack.at_depth(depth) else {
        return div().into_any_element();
    };

    let saved = current_outlet_depth();
    set_outlet_depth(depth);

    let element = entry.route.build(window, cx, &entry.params)
        .unwrap_or_else(|| div().into_any_element());

    set_outlet_depth(saved);
    element
}
```

## Что удаляется

| Модуль | Удаляется | Причина |
|--------|-----------|---------|
| `widgets.rs` | `find_parent_route_for_path()` (140 строк) | Заменён на MatchStack |
| `widgets.rs` | `find_parent_route_internal()` | Заменён на MatchStack |
| `widgets.rs` | `RefCell<OutletState>` | Больше не нужен — нет кэша в outlet |
| `nested.rs` | `resolve_child_route()` | Заменён на resolve_match_stack |
| `nested.rs` | `resolve_child_route_impl()` | Заменён на resolve_match_stack |
| `nested.rs` | `find_index_route()` | Встроен в resolve_recursive |
| `state.rs` | `current_route_for_rendering()` | RouterView читает match_stack[0] |
| `state.rs` | `find_matching_route_in_tree()` | Не нужен |
| `state.rs` | `find_parent_with_children()` | Уже deprecated, удаляем |
| `matching.rs` | весь файл | Дублирование, заменён resolve.rs |

## Что остаётся

| Модуль | Остаётся | Причина |
|--------|----------|---------|
| `nested.rs` | `normalize_path()` | Используется в resolve.rs |
| `nested.rs` | `extract_param_name()` | Используется в resolve.rs |
| `route.rs` | `match_path()` | Нужен для flat route matching и named routes |
| `matcher.rs` | весь файл | Может пригодиться для constraint validation (пока не интегрирован) |
| `cache.rs` | весь файл | Может стать полезным для кэширования MatchStack |

## Влияние на transitions

Transition система (`#[cfg(feature = "transition")]`) сейчас живёт в `OutletState`:

```rust
struct OutletState {
    previous_route: Option<PreviousRoute>,
    current_transition: Transition,
    animation_counter: u64,
}
```

С новой архитектурой transition state переезжает в `GlobalRouter`:

```rust
pub struct GlobalRouter {
    match_stack: MatchStack,
    previous_stack: Option<MatchStack>,  // для exit animations
    // ...
}
```

Outlet проверяет: если `match_stack.at_depth(my_depth)` отличается от `previous_stack.at_depth(my_depth)`, значит нужна transition animation.

## План миграции

### Phase 1: Добавить resolve.rs (неинвазивно)
- [x] Создать `resolve.rs` с MatchStack, resolve_match_stack, depth tracking
- [ ] Добавить `pub mod resolve;` в `lib.rs`
- [ ] Написать тесты для всех edge cases

### Phase 2: Интегрировать в GlobalRouter
- [ ] Добавить `match_stack: MatchStack` поле
- [ ] Вызывать `resolve_match_stack()` в push/replace/back/forward
- [ ] Добавить `re_resolve()` для `add_route()` (когда routes меняются)
- [ ] Добавить `match_stack()` getter

### Phase 3: Переписать outlets
- [ ] Обновить `render_router_outlet()` на чтение из MatchStack
- [ ] Упростить `RouterOutlet::render()` — убрать RefCell
- [ ] Обновить `router_view()` на reset_outlet_depth + match_stack[0]
- [ ] Убрать `find_parent_route_for_path` и `resolve_child_route`

### Phase 4: Cleanup
- [ ] Удалить `matching.rs`
- [ ] Очистить `nested.rs` (оставить normalize_path, extract_param_name)
- [ ] Удалить deprecated код из `state.rs`
- [ ] Удалить println!("DEBUG:...") из nested.rs
- [ ] Обновить lib.rs exports

### Phase 5: Transition migration (optional)
- [ ] Перенести transition state в GlobalRouter
- [ ] Outlet сравнивает match_stack vs previous_stack на своём depth

## Пример использования (без изменений для пользователя!)

```rust
// Определение routes — БЕЗ ИЗМЕНЕНИЙ
let routes = vec![
    Arc::new(
        Route::new("/", |window, cx, _params| {
            div()
                .child("App Layout")
                .child(render_router_outlet(window, cx, None))
        })
        .children(vec![
            Arc::new(Route::new("", |_, _, _| {
                div().child("Home")
            })),
            Arc::new(
                Route::new("workspace/:id", |window, cx, params| {
                    div()
                        .child(format!("Workspace {}", params.get("id").unwrap()))
                        .child(render_router_outlet(window, cx, None))
                })
                .children(vec![
                    Arc::new(Route::new("", |_, _, _| {
                        div().child("Workspace Overview")
                    })),
                    Arc::new(Route::new("project/:pid", |_, _, params| {
                        div().child(format!("Project {}", params.get("pid").unwrap()))
                    })),
                ]),
            ),
        ]),
    ),
];

// Навигация — БЕЗ ИЗМЕНЕНИЙ
Navigator::push(cx, "/workspace/abc/project/xyz");

// MatchStack внутри:
// [0] Route("/")              params={}
// [1] Route("workspace/:id")  params={id: "abc"}
// [2] Route("project/:pid")   params={id: "abc", pid: "xyz"}
```

## Performance

| Операция | Было | Стало |
|----------|------|-------|
| Navigation | O(1) push + cache clear | O(n) resolve + O(1) push |
| Outlet render (1 level) | O(n) find_parent + O(m) resolve_child | O(1) array index |
| Outlet render (3 levels) | O(n) × 3 | O(1) × 3 |
| Outlet render (k levels) | O(n×k) | O(1) × k |
| Total per frame | O(n×k) | O(n) once + O(k) renders |

Где n = количество routes, k = глубина вложенности, m = количество children.

Для типичного приложения с 50 routes и 3-4 уровнями вложенности:
- Было: ~200 comparisons per frame
- Стало: ~50 comparisons once + 4 array lookups per frame
