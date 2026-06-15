# macOS 27 tray click routing — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore left-click → stats panel behavior on macOS 27 Beta 1 (and keep macOS 26 Tahoe / Windows / Linux working unchanged) by clearing `NSStatusItem.button`'s target and action, then driving the right-click menu pop-up ourselves by re-querying the still-attached menu from the status item.

**Architecture:** Single file change in `src-tauri/src/tray.rs`, gated by `#[cfg(target_os = "macos")]`. The `tauri::menu::Menu` is attached to the `NSStatusItem` exactly as today (via `TrayIconBuilder::menu(&menu)`). After `build()`, `tray.with_inner_tray_icon(|inner| ...)` runs on the main thread and calls `button.setTarget(None)` and `button.setAction(None)`. With the action cleared, `ns_button.performClick(...)` (which the subview's `on_tray_click` calls for right-clicks) becomes a no-op, and macOS 27's new behavior of firing the button's action on left-click has nothing to fire. The menu stays attached. On right-click, our `on_tray_icon_event` handler calls `tray.with_inner_tray_icon(...)` again, pulls `Retained<NSMenu>` from `status.menu(mtm)`, and shows the menu via `[NSButton popUpStatusItemMenu:]` (raw objc2 `msg_send!`).

**Tech Stack:** Tauri 2.11.2, `tray-icon 0.23.1`, `objc2 0.6`, `objc2-app-kit 0.3.2`, Rust 2024 edition.

**Reference spec:** `docs/superpowers/specs/2026-06-15-macos-27-tray-click-routing-design.md`

**Reference issue:** robinebers/openusage#573

---

## File Structure

| File | Change | Responsibility |
|------|--------|----------------|
| `src-tauri/src/tray.rs` | Modify | The whole fix lives here. The existing `create()` function gains two `#[cfg(target_os = "macos")]` blocks (post-build override of button target/action, right-click pop-up arm in `on_tray_icon_event`). One new top-level import. |
| (no new files) | — | — |

No other files change. No dependency bumps. No `Cargo.toml` edit. No `[patch.crates-io]`.

---

## Task 1: Add the macOS-only imports

**Files:**
- Modify: `src-tauri/src/tray.rs:1-10` (import block)

This task adds the imports we'll need for Tasks 2 and 3. No behavior change yet.

- [ ] **Step 1.1: Extend the import block**

In `src-tauri/src/tray.rs`, replace the top-of-file import block (lines 1-10) with:

```rust
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::path::BaseDirectory;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};
use tauri_nspanel::ManagerExt;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_store::StoreExt;

#[cfg(target_os = "macos")]
use objc2::msg_send;
#[cfg(target_os = "macos")]
use objc2_app_kit::NSMenu;

use crate::log_path;
use crate::panel::{get_or_init_panel, position_panel_at_tray_icon, show_panel};
```

Three changes:
1. Add `MouseButton` to the `tauri::tray::{...}` use (it's already re-exported by tauri 2.11.2).
2. Add the `#[cfg(target_os = "macos")]` line for `objc2::msg_send` — used in Task 3 for `popUpStatusItemMenu:`.
3. Add the `#[cfg(target_os = "macos")]` line for `objc2_app_kit::NSMenu` — used implicitly via `Retained<NSMenu>` from `status.menu(mtm)`.

No `Retained` import is needed at the top of the file: the `Retained<NSMenu>` returned by `status.menu(mtm)` is consumed within the `with_inner_tray_icon` closure and never stored in a `let` binding outside.

- [ ] **Step 1.2: Confirm `cargo check` on macOS still passes**

Run:
```bash
cd src-tauri && cargo check --target aarch64-apple-darwin
```

Expected: `Finished` with no errors. We only added imports; nothing references them yet.

- [ ] **Step 1.3: Confirm `cargo check` on Windows and Linux still passes**

Run:
```bash
cd src-tauri && cargo check --target x86_64-pc-windows-msvc
cd src-tauri && cargo check --target x86_64-unknown-linux-gnu
```

Expected: `Finished` on both. The `#[cfg(target_os = "macos")]` blocks must compile out cleanly on non-mac targets.

- [ ] **Step 1.4: Commit**

```bash
git add src-tauri/src/tray.rs
git commit -m "fix(tray): add imports for macOS 27 click-routing override

Adds MouseButton (used by the new on_tray_icon_event right-click arm),
objc2::msg_send (for popUpStatusItemMenu:), and objc2_app_kit::NSMenu
(for the menu's Rust type). The actual override is added in the next
commits.

Refs robinebers/openusage#573."
```

---

## Task 2: After `build()`, clear `NSStatusItem.button` target and action

**Files:**
- Modify: `src-tauri/src/tray.rs` — the `TrayIconBuilder::with_id("tray")...build(app_handle)` chain (one line: prepend `let tray = `) and the post-build `#[cfg]` block (inserted after `.build(app_handle)?;`)

This task is the actual fix on macOS 27. Behavior change: on macOS 27, left-click no longer opens the menu (because the button's action is nil). On macOS 26 Tahoe this is a no-op since the button's action was already not firing on left-click. Right-click is also broken at this point — Task 3 wires the manual pop-up.

- [ ] **Step 2.1: Bind the `build` result to a `tray` local**

The current code in `src-tauri/src/tray.rs` discards the return value of `.build(app_handle)?`. We need to keep it so we can call `tray.with_inner_tray_icon(...)` in Step 2.2.

Find the start of the builder chain in `create()`:

```rust
    TrayIconBuilder::with_id("tray")
```

Prepend `let tray = ` so the chain becomes:

```rust
    let tray = TrayIconBuilder::with_id("tray")
```

The chain itself is unchanged. The trailing `.build(app_handle)?;` already propagates the `Result`; it now binds the success value to `tray`. The function still ends with `Ok(())` after the new `#[cfg]` block from Step 2.2.

This is a one-line change. Do not rewrite any of the intermediate `.icon(...)`, `.menu(...)`, `.on_menu_event(...)`, `.on_tray_icon_event(...)`, or `.show_menu_on_left_click(...)` calls — they stay byte-for-byte as in `src-tauri/src/tray.rs` today.

- [ ] **Step 2.2: Insert the post-build override block**

Immediately after `.build(app_handle)?;` (and before the `Ok(())` that ends `create()`), insert:

```rust
// macOS 27 click-routing override (issue #573):
// In macOS 27 Beta 1, NSStatusItem.button now fires its target/action on
// left-click, bypassing the TaoTrayTarget subview's menu_on_left_click
// flag and opening the attached menu before our click handler runs.
// Fix: clear the button's target and action so performClick is a no-op
// and the subview's mouseUp: -> on_tray_icon_event path becomes the
// sole click router. The menu stays attached (so muda's event wiring
// and the existing on_menu_event continue to work), and we pull it back
// out via NSStatusItem.menu(mtm) in the right-click pop-up path.
//
// To be removed when tauri-apps/tray-icon lands an upstream fix.
#[cfg(target_os = "macos")]
{
    let _ = tray.with_inner_tray_icon(|inner| {
        let Some(status) = inner.ns_status_item() else {
            return;
        };
        unsafe {
            let button = status.button(inner.mtm()).unwrap();
            button.setTarget(None);
            button.setAction(None);
        }
    });
    log::info!("Applied macOS 27 click-routing override");
}
```

`with_inner_tray_icon` (tauri 2.11.2 line 633) runs the closure on the main thread and discards the unit result; `let _ = ...` silences the unused-warning. `tray` is the value from Step 2.1.

- [ ] **Step 2.3: Confirm `cargo check` on macOS still passes**

Run:
```bash
cd src-tauri && cargo check --target aarch64-apple-darwin
```

Expected: `Finished` with no errors. **At this point on macOS 27, left-click does nothing (no menu, no panel) and right-click does nothing** — that's expected, Task 3 wires up the manual pop-up.

- [ ] **Step 2.4: Confirm `cargo check` on Windows and Linux still passes**

Run:
```bash
cd src-tauri && cargo check --target x86_64-pc-windows-msvc
cd src-tauri && cargo check --target x86_64-unknown-linux-gnu
```

Expected: `Finished` on both. The new `#[cfg(target_os = "macos")]` block must compile out on non-mac targets.

- [ ] **Step 2.5: Commit**

```bash
git add src-tauri/src/tray.rs
git commit -m "fix(tray): clear NSStatusItem.button target/action on macOS 27

In macOS 27 Beta 1, NSStatusItem.button fires its target/action on
left-click, bypassing the TaoTrayTarget subview's menu_on_left_click
flag and opening the menu before our click handler runs (issue #573).
Clear the button's target and action via with_inner_tray_icon so
performClick is a no-op and the subview becomes the sole click router.

The menu is intentionally left attached: muda's event wiring and the
existing on_menu_event handler still work, and we'll pull the menu
back out via NSStatusItem.menu(mtm) in the next commit to drive the
right-click pop-up ourselves.

Refs robinebers/openusage#573."
```

---

## Task 3: Drive the right-click pop-up from `on_tray_icon_event`

**Files:**
- Modify: `src-tauri/src/tray.rs` — the `on_tray_icon_event` closure inside the `TrayIconBuilder` chain

The closure currently treats every click as a left-click toggle on the panel. We restructure it to `match button` and add a `MouseButton::Right` arm that calls `tray.with_inner_tray_icon(...)` to re-query the menu from the `NSStatusItem` and show it via `popUpStatusItemMenu:`.

- [ ] **Step 3.1: Replace the `on_tray_icon_event` closure**

Find the existing closure in the `TrayIconBuilder` chain. It currently looks like:

```rust
        .on_tray_icon_event(|tray, event| {
            let app_handle = tray.app_handle();

            if let TrayIconEvent::Click {
                button_state, rect, ..
            } = event
            {
                if button_state == MouseButtonState::Up {
                    let Some(panel) = get_or_init_panel!(app_handle) else {
                        return;
                    };

                    if panel.is_visible() {
                        log::debug!("tray click: hiding panel");
                        panel.hide();
                        return;
                    }
                    log::debug!("tray click: showing panel");

                    // macOS quirk: must show window before positioning to another monitor
                    panel.show_and_make_key();
                    position_panel_at_tray_icon(app_handle, rect.position, rect.size);
                }
            }
        })
```

Replace it with:

```rust
        .on_tray_icon_event(move |tray, event| {
            let TrayIconEvent::Click {
                button, button_state, rect, ..
            } = event
            else {
                return;
            };
            if button_state != MouseButtonState::Up {
                return;
            }

            match button {
                MouseButton::Left => {
                    let app_handle = tray.app_handle();
                    let Some(panel) = get_or_init_panel!(app_handle) else {
                        return;
                    };
                    if panel.is_visible() {
                        log::debug!("tray click: hiding panel");
                        panel.hide();
                        return;
                    }
                    log::debug!("tray click: showing panel");
                    panel.show_and_make_key();
                    position_panel_at_tray_icon(app_handle, rect.position, rect.size);
                }
                MouseButton::Right => {
                    // macOS 27 click-routing override (issue #573): with the
                    // button's action cleared (Task 2), the OS no longer pops
                    // the menu automatically on right-click. Drive the pop-up
                    // ourselves by pulling the still-attached menu from the
                    // status item and showing it via popUpStatusItemMenu:.
                    #[cfg(target_os = "macos")]
                    {
                        let _ = tray.with_inner_tray_icon(|inner| {
                            let Some(status) = inner.ns_status_item() else {
                                return;
                            };
                            unsafe {
                                let Some(menu) = status.menu(inner.mtm()) else {
                                    return;
                                };
                                let button = status.button(inner.mtm()).unwrap();
                                let _: () =
                                    msg_send![&button, popUpStatusItemMenu: &*menu];
                            }
                        });
                    }
                }
                MouseButton::Middle => {}
            }
        })
```

Notes on the change:
- The closure becomes `move |tray, event|` — required so the `with_inner_tray_icon` closure inside the `Right` arm can own its captures (`inner` etc. are all owned by the closure, no `&'static` is needed for that part). The `move` does NOT change the outer `tray: &TrayIcon<R>` parameter capture; that parameter is passed at call time as before.
- The `Left` arm is platform-agnostic and unchanged in behavior (left-click still toggles the panel).
- The `Right` arm's body is `#[cfg(target_os = "macos")]`-gated. On non-mac targets, right-click still goes through the OS's built-in pop-up path, which is the pre-fix behavior.
- The `Middle` arm is intentionally a no-op (preserves the existing behavior of silently ignoring middle-click).
- The `let-else` early return replaces the nested `if let` for a flat, easy-to-read structure.

- [ ] **Step 3.2: Confirm `cargo check` on macOS passes**

Run:
```bash
cd src-tauri && cargo check --target aarch64-apple-darwin
```

Expected: `Finished` with no errors. **At this point on macOS 27 the bug is fixed**: left-click shows the panel, right-click pops the menu.

- [ ] **Step 3.3: Confirm `cargo check` on Windows and Linux passes**

Run:
```bash
cd src-tauri && cargo check --target x86_64-pc-windows-msvc
cd src-tauri && cargo check --target x86_64-unknown-linux-gnu
```

Expected: `Finished` on both. The `MouseButton::Right` arm's body is `#[cfg(target_os = "macos")]`-gated, so the macOS-only `msg_send!` and `NSMenu` references compile out cleanly.

- [ ] **Step 3.4: Commit**

```bash
git add src-tauri/src/tray.rs
git commit -m "fix(tray): pop up menu on right-click via objc2 (macOS 27)

Completes the issue #573 fix. With the status item's button target and
action now cleared (prior commit), the OS no longer pops the menu
automatically on right-click. Drive the pop-up ourselves through
[NSButton popUpStatusItemMenu:] using the NSMenu pulled from
NSStatusItem.menu(mtm).

The left-click path is unchanged: subview mouseUp -> on_tray_icon_event
-> show_panel.

Refs robinebers/openusage#573."
```

---

## Task 4: Final cross-platform compile gate and manual test plan

**Files:** none modified

- [ ] **Step 4.1: Run the full cross-platform compile gate**

Run, in order:
```bash
cd src-tauri && cargo check --target aarch64-apple-darwin
cd src-tauri && cargo check --target x86_64-pc-windows-msvc
cd src-tauri && cargo check --target x86_64-unknown-linux-gnu
```

Expected: `Finished` on all three. If any fails, the regression is a missing `#[cfg]` or an objc2_app_kit signature mismatch; the build error is the diagnostic.

- [ ] **Step 4.2: Run the existing Rust test suite**

Run:
```bash
cd src-tauri && cargo test --lib
```

Expected: all existing tests pass. This plan does not add Rust unit tests (the bug is a runtime AppKit behavior; see "What is NOT in this plan" below).

- [ ] **Step 4.3: Run the JS / Vitest suite**

Run:
```bash
npx vitest run --reporter=dot
```

Expected: all existing JS tests pass. The Rust change has no JS counterpart.

- [ ] **Step 4.4: Document the manual test plan in the spec's `## Testing` section**

The spec already lists a manual smoke matrix. No edit needed; the spec at `docs/superpowers/specs/2026-06-15-macos-27-tray-click-routing-design.md` is the authoritative manual test plan. Print it for the operator:

> **Manual test plan (operator, not automated):**
>
> 1. **macOS 27 Beta 1 (the regression target):**
>    - Left-click tray icon → stats panel appears.
>    - Right-click tray icon → dropdown menu appears (Show Stats / Go to Settings / Debug Level / About / Quit).
>    - Click "Show Stats" in the menu → panel appears.
>    - Click "Quit" → app exits.
>    - Check logs for: `Applied macOS 27 click-routing override`.
>
> 2. **macOS 26 Tahoe (regression-avoidance target):**
>    - Same matrix. Behavior must be visually indistinguishable from pre-fix.
>
> 3. **Windows / Linux (compile gate):** `cargo check` passes; no runtime test required.

- [ ] **Step 4.5: Commit (no changes — but tag the spec as verified)**

If the spec is going to be referenced from the eventual PR, add a single line at the top of `docs/superpowers/specs/2026-06-15-macos-27-tray-click-routing-design.md` under the existing `**Status:** design` line:

```markdown
- **Status:** design — implementation verified on <macOS version>
```

(The `<macOS version>` is filled in after the manual test pass.) This is a docs-only commit; use `git add -f` because the file is gitignored.

```bash
git add -f docs/superpowers/specs/2026-06-15-macos-27-tray-click-routing-design.md
git commit -m "docs: mark macOS 27 tray click routing spec as verified"
```

(Only commit if you actually ran the manual test and the result was green. Don't fabricate a verification.)

---

## What is NOT in this plan

- **No Rust unit test.** The bug is in AppKit's runtime behavior on a specific macOS version; we don't have a CI matrix that can reproduce it. The unit-testable surface (e.g. "given a `MouseButton::Right` event, the handler should call `with_inner_tray_icon`") would mock the AppKit layer, which is the same mocking that already covers the panel module and adds no real signal.
- **No JS test.** The fix is entirely in the Rust tray handler; no JS path is touched.
- **No CHANGELOG edit.** The repo's `CHANGELOG.md` is auto-generated from PR titles by `release-please` / Dependabot; the conventional `fix:` commit messages here will be picked up automatically.
- **No `Cargo.toml` bump.** No new dependencies, no version changes. `objc2` and `objc2-app-kit` are already present in `[target.'cfg(target_os = "macos")'.dependencies]`. Notably we do NOT need to add `muda` as a direct dep — the menu retain is pulled from `NSStatusItem.menu(mtm)`, not from a `tauri::menu::Menu` accessor.
- **No upstream PR.** The fix is local to our fork; the upstream `tauri-apps/tray-icon` issue/PR can be filed separately and our override can be reverted once an upstream fix lands (the comment in Task 2.2 documents this).

---

## Self-Review

**1. Spec coverage** — checked each spec section against the plan:

| Spec section | Plan task |
|---|---|
| Approach (clear target/action, keep menu attached, drive pop-up ourselves) | Tasks 2 and 3 |
| Implementation §1 (build with menu attached unchanged) | Implicit; no code change needed |
| Implementation §2 (post-build override via `with_inner_tray_icon`, clear target/action) | Task 2.2 |
| Implementation §3 (right-click pop-up via `NSStatusItem.menu(mtm)` and `popUpStatusItemMenu:`) | Task 3.1 |
| Implementation §4 (one-time `log::info!`) | Task 2.2 |
| What is NOT changed | Documented in "What is NOT in this plan" above |
| Risks | Addressed in Task 4.1 (cross-platform compile) and Task 4.4 (manual test) |
| Testing (manual smoke matrix, cross-platform compile gate) | Task 4 |
| Doc updates (CHANGELOG handled by `release-please`; spec) | Task 4.5 |

**2. Placeholder scan** — no `TBD`, no `TODO`, no `add appropriate error handling`, no `similar to task N`. All code blocks are complete; all commands are exact; all expected outputs are stated.

**3. Type consistency** — no long-lived state carries across tasks. The `tray: TrayIcon<R>` local introduced in Task 2.1 is used in Tasks 2.2 and 3.1. The `Retained<NSMenu>` returned by `status.menu(mtm)` in Task 3.1 is local to the `with_inner_tray_icon` closure and dropped at its end. No name drift, no cross-task captures beyond what the plan states.

**4. Plan revision history** — the original plan proposed retaining the `NSMenu` from the local `tauri::menu::Menu` before `build()` consumed the borrow, on the assumption that `tauri::menu::Menu` exposes muda's `ContextMenu::ns_menu` via the public API. Investigation showed the access path is blocked by a `pub(crate) sealed::ContextMenuBase` supertrait. The revised approach — re-querying the menu from `NSStatusItem.menu(mtm)` after `build()` — is simpler and avoids the access problem entirely. No dep change, no menu-type swap, and the menu stays attached to the `NSStatusItem` so muda's event wiring continues to work.

No outstanding issues. Plan is self-consistent and covers the spec.
