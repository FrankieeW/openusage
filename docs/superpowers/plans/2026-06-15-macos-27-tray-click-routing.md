# macOS 27 tray click routing — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restore left-click → stats panel behavior on macOS 27 Beta 1 (and keep macOS 26 Tahoe / Windows / Linux working unchanged) by clearing the `NSStatusItem.button` target/action and detaching the menu, then driving the right-click menu pop-up ourselves from `on_tray_icon_event`.

**Architecture:** Single file change in `src-tauri/src/tray.rs`, gated by `#[cfg(target_os = "macos")]`. We retain a `Retained<NSMenu>` from the local `tauri::menu::Menu` before `TrayIconBuilder::build()` consumes the borrow (muda 0.19.1's `ContextMenu::ns_menu` returns the raw pointer; `Retained::retain` upgrades it). After build, `tray.with_inner_tray_icon(|inner| ...)` runs on the main thread and detaches the menu from the `NSStatusItem` and clears the button's target/action. The existing `on_tray_icon_event` handler is extended to detect right-click `Up` and call `[NSButton popUpStatusItemMenu:]` (raw objc2 `msg_send!`) on the retained menu.

**Tech Stack:** Tauri 2.11.2, `tray-icon 0.23.1`, `muda 0.19.1`, `objc2 0.6`, `objc2-app-kit 0.3`, Rust 2024 edition.

**Reference spec:** `docs/superpowers/specs/2026-06-15-macos-27-tray-click-routing-design.md`

**Reference issue:** robinebers/openusage#573

---

## File Structure

| File | Change | Responsibility |
|------|--------|----------------|
| `src-tauri/src/tray.rs` | Modify | The whole fix lives here. Existing `create()` function gains two `#[cfg(target_os = "macos")]` blocks (capture retain, post-build override) and the `on_tray_icon_event` closure gains a `MouseButton::Right` arm. |
| (no new files) | — | — |

No other files change. No dependency bumps. No `Cargo.toml` edit. No `[patch.crates-io]`.

---

## Task 1: Capture `Retained<NSMenu>` from the local menu before `build()`

**Files:**
- Modify: `src-tauri/src/tray.rs:1-15` (imports) and `src-tauri/src/tray.rs:145-160` (around the existing `let menu = Menu::with_items(...)`)

This task only adds an `objc2` import and extracts a retained `NSMenu` reference from the `menu` we already build. No behavior change yet.

- [ ] **Step 1.1: Add the macOS-only imports**

In `src-tauri/src/tray.rs`, replace the top-of-file import block (lines 1-8) with:

```rust
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::path::BaseDirectory;
use tauri::tray::{MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};
use tauri_nspanel::ManagerExt;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_store::StoreExt;

#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2_app_kit::NSMenu;

use crate::log_path;
use crate::panel::{get_or_init_panel, position_panel_at_tray_icon, show_panel};
```

The `Retained` and `NSMenu` imports are only used on macOS, so the `#[cfg]` is essential — non-mac builds will fail to compile otherwise (`objc2-app-kit` is a mac-only dependency in `Cargo.toml`).

- [ ] **Step 1.2: Capture the `Retained<NSMenu>` immediately after building the `Menu`**

In `src-tauri/src/tray.rs`, find the line that builds the menu:

```rust
let menu = Menu::with_items(
    app_handle,
    &[
        &show_stats,
        &go_to_settings,
        &log_level_submenu,
        &separator,
        &about,
        &quit,
    ],
)?;
```

Immediately after the closing `)?;`, insert:

```rust
// macOS 27 click-routing override (issue #573): retain a handle to the raw
// NSMenu before the TrayIconBuilder consumes the &menu borrow. We need this
// retained handle later to pop up the menu on right-click via objc2, because
// the post-build override (see below) detaches the menu from NSStatusItem.
#[cfg(target_os = "macos")]
let ns_menu_retain: Option<Retained<NSMenu>> = {
    let raw = menu.ns_menu() as *mut NSMenu;
    unsafe { Retained::retain(raw) }
};
```

`menu.ns_menu()` is muda 0.19.1's `ContextMenu::ns_menu` (lib.rs line 455) — returns `*mut c_void`; we cast to `*mut NSMenu`. `Retained::retain` upgrades to a reference-counted handle. The `Option` wrapper handles the (theoretically impossible but type-safer) null case.

- [ ] **Step 1.3: Confirm `cargo check` on macOS still passes**

Run:
```bash
cd src-tauri && cargo check --target aarch64-apple-darwin
```

Expected: `Finished` with no errors. The new code path is unreachable at this point — we only captured a retain, no behavior changed.

- [ ] **Step 1.4: Confirm `cargo check` on Windows and Linux still passes**

Run:
```bash
cd src-tauri && cargo check --target x86_64-pc-windows-msvc
cd src-tauri && cargo check --target x86_64-unknown-linux-gnu
```

Expected: `Finished` on both. The `#[cfg(target_os = "macos")]` blocks must compile out cleanly on non-mac targets.

- [ ] **Step 1.5: Commit**

```bash
git add src-tauri/src/tray.rs
git commit -m "fix(tray): retain NSMenu handle for macOS 27 click-routing override

Captures a Retained<NSMenu> from the local tauri::menu::Menu before the
TrayIconBuilder consumes the borrow. This retain will be used in the
follow-up commit to pop up the menu on right-click via objc2, once the
status item's button target/action is cleared.

Refs robinebers/openusage#573."
```

---

## Task 2: Detach the menu and clear `NSStatusItem.button` target/action after `build()`

**Files:**
- Modify: `src-tauri/src/tray.rs:157-241` (the `TrayIconBuilder::with_id("tray")...build(app_handle)` chain)

This task inserts the `with_inner_tray_icon` block right after the existing `.build(app_handle)?` line. Behavior on macOS 27 changes here; macOS 26 Tahoe is unaffected at this point (we haven't added the manual pop-up yet — that's Task 3).

- [ ] **Step 2.1: Find the `build` site**

In `src-tauri/src/tray.rs`, locate the line:

```rust
.build(app_handle)?;
```

This is the terminal call of the `TrayIconBuilder` chain. We insert our override immediately after it.

- [ ] **Step 2.2: Insert the post-build override block**

Immediately after `.build(app_handle)?;` (and before the `Ok(())` that ends `create()`), insert:

```rust
// macOS 27 click-routing override (issue #573):
// In macOS 27 Beta 1, NSStatusItem.button now fires its target/action on
// left-click, bypassing the TaoTrayTarget subview's menu_on_left_click
// flag and opening the attached menu before our click handler runs.
// Fix: detach the menu from the status item and clear the button's
// target/action so the subview's mouseUp: → on_tray_icon_event path is
// the sole click router. We then manually pop up the menu on right-click
// in the on_tray_icon_event closure below.
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
            status.setMenu(None);
            button.setTarget(None);
            button.setAction(None);
        }
    });
    log::info!("Applied macOS 27 click-routing override");
}
```

`with_inner_tray_icon` (tauri 2.11.2 line 633) runs the closure on the main thread and discards the unit result; we use `let _ = ...` to silence the unused-warning. `tray` is the value returned by `.build(app_handle)?`; bind it explicitly if it isn't already — see step 2.3.

- [ ] **Step 2.3: Bind the `build` result to a `tray` local**

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

- [ ] **Step 2.4: Confirm `cargo check` on macOS still passes**

Run:
```bash
cd src-tauri && cargo check --target aarch64-apple-darwin
```

Expected: `Finished` with no errors. **At this point on macOS 27, left-click does nothing (no menu, no panel) and right-click does nothing** — that's expected, Task 3 wires up the manual pop-up.

- [ ] **Step 2.5: Confirm `cargo check` on Windows and Linux still passes**

Run:
```bash
cd src-tauri && cargo check --target x86_64-pc-windows-msvc
cd src-tauri && cargo check --target x86_64-unknown-linux-gnu
```

Expected: `Finished` on both. The new `#[cfg(target_os = "macos")]` block must compile out on non-mac targets.

- [ ] **Step 2.6: Commit**

```bash
git add src-tauri/src/tray.rs
git commit -m "fix(tray): clear NSStatusItem.button target/action on macOS 27

In macOS 27 Beta 1, NSStatusItem.button fires its target/action on
left-click, bypassing the TaoTrayTarget subview's menu_on_left_click
flag and opening the menu before our click handler runs (issue #573).
Detach the menu and clear target/action via with_inner_tray_icon so
the subview is the sole click router. The right-click pop-up is added
in the next commit.

Refs robinebers/openusage#573."
```

---

## Task 3: Drive the right-click pop-up from `on_tray_icon_event`

**Files:**
- Modify: `src-tauri/src/tray.rs:214-238` (the `on_tray_icon_event` closure)

The closure currently only handles left-click → show panel. We extend it to also handle right-click → pop up the menu via objc2, using the `ns_menu_retain` we captured in Task 1.

- [ ] **Step 3.1: Replace the `on_tray_icon_event` closure**

Find the existing closure:

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

Wait — the current closure does *not* use the `MouseButton::Left` discriminator at all. It treats every click as a left-click toggle on the panel. That's actually the existing behavior pre-fix on Tahoe (left-click toggles the panel, right-click pops the menu). Our override means the menu no longer pops on right-click automatically, so we need to add the right-click pop-up arm. Left-click behavior stays exactly as it is.

Replace the closure with:

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
                    // macOS 27 click-routing override (issue #573): the button's
                    // target/action is no longer wired to a menu, so we drive
                    // the pop-up ourselves via objc2.
                    #[cfg(target_os = "macos")]
                    {
                        let Some(menu_retain) = ns_menu_retain.clone() else {
                            return;
                        };
                        let _ = tray.with_inner_tray_icon(move |inner| {
                            let Some(status) = inner.ns_status_item() else {
                                return;
                            };
                            unsafe {
                                let button = status.button(inner.mtm()).unwrap();
                                let _: () = msg_send![&button, popUpStatusItemMenu: &*menu_retain];
                            }
                        });
                    }
                }
                MouseButton::Middle => {}
            }
        })
```

Important: this change is **not** gated by `#[cfg(target_os = "macos")]` for the whole closure. The `MouseButton::Left` arm is platform-agnostic and unchanged in behavior. Only the `MouseButton::Right` arm has a `#[cfg]` block (Windows / Linux don't use the objc2 pop-up path — their native menu pop-up is still wired by the OS). The match against `MouseButton::Middle` is intentional to make the `match` exhaustive; the existing code ignored middle-click silently, and we preserve that.

- [ ] **Step 3.2: Import `MouseButton` into scope**

The closure's `match button { MouseButton::Left, MouseButton::Right, MouseButton::Middle }` references `MouseButton`, but the existing `use tauri::tray::{MouseButtonState, TrayIconBuilder, TrayIconEvent};` does not import it. Update the line at the top of `src-tauri/src/tray.rs` to:

```rust
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
```

`MouseButton` is re-exported by `tauri::tray` (tauri 2.11.2 line 19: `pub use tray_icon::TrayIconId;` and the surrounding module re-exports the mouse types).

- [ ] **Step 3.3: Import `msg_send` at the top of the file**

The new closure uses `msg_send!`. In `src-tauri/src/tray.rs`, extend the `#[cfg(target_os = "macos")]` import block from Task 1.1:

```rust
#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2::msg_send;
#[cfg(target_os = "macos")]
use objc2_app_kit::NSMenu;
```

`objc2::msg_send` is the macro used in `panel.rs` and `tray.rs` already; importing it via the path keeps the local scope explicit.

- [ ] **Step 3.4: Confirm `cargo check` on macOS passes**

Run:
```bash
cd src-tauri && cargo check --target aarch64-apple-darwin
```

Expected: `Finished` with no errors.

- [ ] **Step 3.5: Confirm `cargo check` on Windows and Linux passes**

Run:
```bash
cd src-tauri && cargo check --target x86_64-pc-windows-msvc
cd src-tauri && cargo check --target x86_64-unknown-linux-gnu
```

Expected: `Finished` on both. The `MouseButton::Right` arm's body is `#[cfg(target_os = "macos")]`-gated, so the macOS-only `msg_send!` and `ns_menu_retain` references compile out cleanly.

- [ ] **Step 3.6: Commit**

```bash
git add src-tauri/src/tray.rs
git commit -m "fix(tray): pop up menu on right-click via objc2 (macOS 27)

Completes the issue #573 fix. With the status item's button target/action
now cleared (prior commit), the OS no longer pops the menu automatically
on right-click. Drive the pop-up ourselves through
[NSButton popUpStatusItemMenu:] using the NSMenu retain captured earlier.

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

Expected: all existing tests pass. This plan does not add Rust unit tests (the bug is a runtime AppKit behavior; see "What is NOT tested" below).

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
- **No `Cargo.toml` bump.** No new dependencies, no version changes. `objc2` and `objc2-app-kit` are already present in `[target.'cfg(target_os = "macos")'.dependencies]`.
- **No upstream PR.** The fix is local to our fork; the upstream `tauri-apps/tray-icon` issue/PR can be filed separately and our override can be reverted once an upstream fix lands (the comment in Task 2.2 documents this).

---

## Self-Review

**1. Spec coverage** — checked each spec section against the plan:

| Spec section | Plan task |
|---|---|
| Approach (detach menu, clear target/action, drive pop-up ourselves) | Tasks 1, 2, 3 |
| Implementation §1 (build with menu attached unchanged) | Implicit; no code change needed |
| Implementation §2 (capture retain before build) | Task 1.2 |
| Implementation §2 (post-build override via `with_inner_tray_icon`) | Task 2.2 |
| Implementation §3 (right-click pop-up via `popUpStatusItemMenu:`) | Task 3.1 |
| Implementation §4 (one-time `log::info!`) | Task 2.2 |
| What is NOT changed | Documented in "What is NOT in this plan" above |
| Risks | Addressed in Task 4.1 (cross-platform compile) and Task 4.4 (manual test) |
| Testing (manual smoke matrix, cross-platform compile gate) | Task 4 |
| Doc updates (CHANGELOG handled by `release-please`; spec) | Task 4.5 |

**2. Placeholder scan** — no `TBD`, no `TODO`, no `add appropriate error handling`, no `similar to task N`. All code blocks are complete; all commands are exact; all expected outputs are stated.

**3. Type consistency** — `ns_menu_retain: Option<Retained<NSMenu>>` is the single source of truth. Captured in Task 1.2, cloned in Task 3.1, `msg_send!`-consumed in Task 3.1. No name drift. The `tray` local is introduced in Task 2.3 and used in Tasks 2.2 and 3.1.

**4. Step 2.3 originally contained three restated variations of the binding change during writing; cleaned up to a single one-line instruction. The `MouseButton` import was missing from the original Task 3; added as Step 3.2 with subsequent renumbering.**

No outstanding issues. Plan is self-consistent and covers the spec.
