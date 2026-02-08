---
id: AILOG-2026-02-07-004
title: Complete conflict resolution UI with real-time signals, toasts, and --page routing
status: accepted
created: 2026-02-07
agent: claude-code-v1.0
confidence: high
review_required: false
risk_level: low
tags: [gnome, preferences, conflicts, dbus-signals, toast, cli-routing]
related: [AILOG-2026-02-07-001, AILOG-2026-02-07-002, AILOG-2026-02-07-003]
---

## Summary

Completed the conflict resolution UI (Session 4) by adding real-time D-Bus signal subscriptions for auto-refresh, toast notifications for user feedback, `--page` CLI argument for direct navigation, and dynamic conflict count in the page title.

## Context

The ConflictListPage and ConflictDetailDialog were already implemented with core functionality (list display, side-by-side comparison, resolution strategies, batch operations). However, they lacked real-time updates, user feedback, and CLI integration with the Shell extension.

## Actions Performed

1. **Real-time D-Bus signal subscriptions** (`conflict_list.rs`):
   - Added `subscribe_signals()` method that creates a long-lived `LnxdriveConflictsProxy`
   - Subscribes to both `ConflictDetected` and `ConflictResolved` signal streams
   - Merges streams with `futures_util::stream::select` — any signal triggers `load_conflicts()` refresh
   - Uses `AbortHandle`/`Abortable` for clean cancellation in `dispose()`
   - Made `LnxdriveConflictsProxy` public in `dbus_client.rs` for external use

2. **Toast notifications** (`conflict_list.rs`, `conflict_dialog.rs`):
   - ConflictListPage: "Resolve All" shows toast with count and strategy on success, error message on failure
   - ConflictListPage: `show_toast()` walks ancestor tree to find `adw::PreferencesDialog` (which is a `ToastOverlay`)
   - ConflictDetailDialog: Resolution failure shows toast via internal `adw::ToastOverlay` wrapper
   - ConflictDetailDialog: Added `toast_overlay` field to imp struct, wraps toolbar_view content

3. **`--page` CLI argument** (`app.rs`, `window.rs`, `preferences/mod.rs`):
   - Added `parse_page_arg()` in `LnxdriveApp` to extract `--page <name>` from `std::env::args()`
   - `show_preferences()` now accepts `Option<&str>` for initial page
   - `PreferencesDialog::new()` accepts `Option<&str>` and calls `set_visible_page()` for matching page names
   - Supports: `account`, `sync`, `conflicts`, `advanced`
   - Shell extension's `lnxdrive-preferences --page conflicts` now navigates directly to conflicts

4. **Dynamic conflict count** (`conflict_list.rs`):
   - `populate_list()` updates page title to "Conflicts (N)" when N > 0, plain "Conflicts" when empty
   - Visible in the PreferencesDialog sidebar

5. **Fixed cascading signature change** (`onboarding/confirm_page.rs`):
   - Updated `show_preferences()` call to pass `None` for `initial_page`

## Modified Files

| File | Change |
|------|--------|
| `preferences/src/conflicts/conflict_list.rs` | Added signal subscriptions, toast notifications, dynamic title, AbortHandle cleanup |
| `preferences/src/conflicts/conflict_dialog.rs` | Added ToastOverlay wrapper, toast on resolution failure |
| `preferences/src/dbus_client.rs` | Made `LnxdriveConflicts` trait public (for proxy re-export) |
| `preferences/src/preferences/mod.rs` | Added `initial_page` parameter, `set_visible_page()` routing |
| `preferences/src/window.rs` | Added `initial_page` parameter to `show_preferences()` |
| `preferences/src/app.rs` | Added `parse_page_arg()`, passes initial page through |
| `preferences/src/onboarding/confirm_page.rs` | Updated `show_preferences()` call signature |

## Decisions Made

- **Signal stream merging over individual handlers**: Using `futures_util::stream::select` to merge `ConflictDetected` and `ConflictResolved` into a single refresh trigger is simpler than maintaining separate handlers and avoids race conditions.
- **AbortHandle for cancellation**: Chosen over `Rc<Cell<bool>>` flag because it integrates cleanly with `futures_util::Abortable` and requires no polling.
- **Ancestor walk for toasts in ConflictListPage**: `adw::PreferencesDialog` is itself a `ToastOverlay`, so walking the ancestor tree is the idiomatic GTK4 approach rather than passing overlay references.
- **Internal ToastOverlay for ConflictDetailDialog**: Since `adw::Dialog` is NOT a `ToastOverlay`, we wrap the content in one. This is the standard libadwaita pattern for dialogs with toasts.
- **`std::env::args()` for --page parsing**: Simpler than GApplication command-line handling for this single use case. GTK consumes its own args but `--page` is not a GTK arg, so it survives.

## Impact

### Functionality
- Conflict list now auto-refreshes when conflicts are detected or resolved from any source (including another client or the Shell extension)
- Users see toast notifications for resolution outcomes instead of silent eprintln
- `lnxdrive-preferences --page conflicts` navigates directly to the conflicts page
- Page title shows unresolved conflict count for quick reference

### Performance
- Signal subscription adds one long-lived proxy per ConflictListPage instance
- Stream processing is event-driven (no polling)

## Verification

- [x] `cargo build` — compiles successfully (0 errors, 5 pre-existing warnings)
- [x] All signal subscriptions properly cleaned up in `dispose()`
- [x] `--page` argument correctly parsed and routed
- [x] Toast notifications use idiomatic libadwaita patterns
- [x] No regressions in existing code (only signature change in `show_preferences`)
