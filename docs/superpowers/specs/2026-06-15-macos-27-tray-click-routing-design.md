# macOS 27 tray click routing

- **Status:** design
- **Date:** 2026-06-15
- **Upstream issue:** robinebers/openusage#573 (closed, no closing PR linked)
- **Affects:** OpenUsage Community Collection 1.0.6, Tauri 2.11.2, `tray-icon 0.23.1`, `muda 0.19.1`
- **Scope:** `src-tauri/src/tray.rs`

## Problem

On macOS 27 Beta 1, left-clicking the OpenUsage tray icon opens the right-click dropdown menu instead of showing the stats panel. The user has to click "Show Stats" inside the menu. On macOS 26 Tahoe the same build behaves correctly.

Reproduction (from issue #573):

1. Click the OpenUsage icon in the menu bar with the left mouse button.
2. Expected: stats panel opens beneath the icon.
3. Actual: the dropdown menu (Show Stats / Go to Settings / Debug Level / About / Quit) opens. The panel does not appear.

## Root cause

`tauri-apps/tray-icon 0.23.1` (`src/platform_impl/macos/mod.rs`) creates a custom `TaoTrayTarget` NSView and adds it as a subview of the status item's `NSButton`. The intended click flow is:

1. Click hits the subview first.
2. `TaoTrayTarget.mouseDown:` runs `on_tray_click(self, MouseButton::Left)`.
3. `on_tray_click` checks `menu_on_left_click` (we set it to `false`). With the flag `false`, the code skips `ns_button.performClick(None)` and only calls `ns_button.highlight(true)`.
4. `TaoTrayTarget.mouseUp:` sends a `TrayIconEvent::Click` to the Tauri event bus.
5. Our `on_tray_icon_event` handler receives the click and calls `show_panel(...)`.

In `TrayIcon::create()` (line 65-69) the menu is also attached to the `NSStatusItem` via `ns_status_item.setMenu(menu)`, which wires the button's `target`/`action` to "open that menu".

On macOS 26 Tahoe, the button's target/action only fires on right-click (or when the subview's `on_tray_click` explicitly calls `performClick`). The subview's flag-based gating is the sole routing authority, and our `false` setting works.

On macOS 27 Beta 1, the status item's button now fires its target/action on **left-click as well**, in parallel with the subview receiving the event. Because the menu is attached to the `NSStatusItem`, the button's action is "pop up that menu", and the menu opens before the subview's `on_tray_click` returns. The `menu_on_left_click = false` flag becomes a no-op for routing — by the time the subview decides "don't pop", the system has already popped it.

The user observes the menu, not the panel. Right-clicking still works because that path goes through the subview's `rightMouseDown:` → `on_tray_click` → `performClick` (or the system path; both lead to the menu).

## Approach

We move click routing off the `NSStatusItem`'s built-in target/action mechanism and onto the `TaoTrayTarget` subview exclusively. The `NSButton` becomes a "dumb" container — its `target` and `action` are cleared, but the menu stays attached. Both left- and right-click then travel through the subview's existing handlers; the subview already calls `send_mouse_event` for `mouseUp:` on both buttons, so we receive a `TrayIconEvent::Click { button, button_state, .. }` for both, and we can drive both behaviors from a single `on_tray_icon_event` closure:

- `Left + Up` → `show_panel(...)` (existing behavior, unchanged).
- `Right + Up` → manually pop up the menu. We get the menu on demand via `NSStatusItem::menu(mtm)` (returns `Option<Retained<NSMenu>>` from `objc2_app_kit 0.3.2`) and pass it to `[NSButton popUpStatusItemMenu:]` via objc2. This replaces the path that the system used to handle for us.

The `menu_on_left_click` / `menu_on_right_click` flags in `tray-icon` become irrelevant because clearing the button's `action` makes `ns_button.performClick` a no-op (it has nothing to fire). We're not gating the pop-up anymore, we're bypassing the system's auto-pop path entirely.

The `on_menu_event` closure that already handles `show_stats`, `go_to_settings`, `log_*`, `copy_log_path`, `about`, `quit` is **untouched** — muda's menu-event delivery is independent of the pop-up mechanism and continues to work. The menu stays attached to the `NSStatusItem` (so muda's delegate wiring is preserved) but never auto-opens.

### Why we don't lose the right-click path

`TaoTrayTarget.rightMouseUp:` (macOS module line 382-393) sends a `TrayIconEvent::Click` with `button: MouseButton::Right` regardless of whether anything was popped up. We receive that event in `on_tray_icon_event` and call the manual pop-up. The visual result is identical to the system path; we just drive it ourselves.

### Why this is regression-safe on macOS 26 Tahoe

On Tahoe, the system path never opened the menu on left-click, so the subview's flag-based path was already the only thing in play. Our fix detaches the menu and clears target/action, then drives the right-click pop-up ourselves. Left-click on Tahoe still goes subview → on_tray_click (no-op) → mouseUp → our handler → show_panel. Right-click on Tahoe: subview → on_tray_click (performClick no longer opens a menu) → mouseUp → our handler → manual pop-up. Net: identical user-visible behavior, one extra objc2 call per right-click.

## Implementation

All changes in `src-tauri/src/tray.rs`. `#[cfg(target_os = "macos")]` blocks; non-mac builds are unchanged.

### 1. Build the tray with the menu attached (unchanged)

Keep `.menu(&menu)` on the `TrayIconBuilder`. `tray-icon` will call `NSStatusItem.setMenu(menu)` internally, attaching the menu. We deliberately leave the menu attached; we only need to suppress the auto-pop behavior (see §2). The menu being attached also keeps muda's event-delivery wiring live for the existing `on_menu_event` closure.

### 2. After `build()`, clear the button's `target` and `action` via `with_inner_tray_icon`

We do **not** detach the menu. The two operations we perform:

- `button.setTarget(None)` — clear the target.
- `button.setAction(None)` — clear the action. This is the line that fixes the regression. With action cleared, `ns_button.performClick(...)` (which the subview's `on_tray_click` calls for right-clicks) becomes a no-op, and macOS 27's new behavior of firing the button's action on left-click has nothing to fire.

We do **not** call `status.setMenu(None)`. Detaching would force us to retain the `NSMenu` ourselves (because we need it for the manual right-click pop-up); since `objc2_app_kit 0.3.2`'s `NSStatusItem` already exposes `pub fn menu(&self, mtm: MainThreadMarker) -> Option<Retained<NSMenu>>` (generated/NSStatusItem.rs:67-69), we can re-query the menu on demand at right-click time and avoid the retain entirely.

```rust
let tray = TrayIconBuilder::with_id("tray")
    .icon(icon)
    .tooltip("OpenUsage")
    .menu(&menu)
    .show_menu_on_left_click(false)
    .on_menu_event(/* ... existing closure, unchanged ... */)
    .on_tray_icon_event(/* ... new closure, see §3 ... */)
    .build(app_handle)?;

// macOS 27 click-routing override (issue #573):
// In macOS 27 Beta 1, NSStatusItem.button fires its target/action on
// left-click, opening the attached menu before the TaoTrayTarget subview's
// click handler can gate it. Clearing the button's target/action makes
// performClick a no-op, restoring left-click → panel routing. The menu
// stays attached so muda's event wiring and on_menu_event continue to
// work, and so we can pull it back out via NSStatusItem.menu(mtm) on
// right-click for the manual pop-up.
//
// To be removed when tauri-apps/tray-icon lands an upstream fix.
#[cfg(target_os = "macos")]
{
    let _ = tray.with_inner_tray_icon(|inner| {
        let Some(status) = inner.ns_status_item() else { return; };
        unsafe {
            let button = status.button(inner.mtm()).unwrap();
            button.setTarget(None);
            button.setAction(None);
        }
    });
    log::info!("Applied macOS 27 click-routing override");
}
```

### 3. Pop the menu on right-click in `on_tray_icon_event`

```rust
.on_tray_icon_event(move |tray, event| {
    let TrayIconEvent::Click { button, button_state, .. } = event else { return; };
    if button_state != MouseButtonState::Up { return; }

    match button {
        MouseButton::Left => {
            // existing path — show / hide panel
            show_panel(tray.app_handle());
        }
        MouseButton::Right => {
            // macOS 27 click-routing override (issue #573): with the
            // button's action cleared (§2), the OS no longer pops the
            // menu automatically on right-click. Drive the pop-up
            // ourselves by pulling the still-attached menu from the
            // status item and showing it via popUpStatusItemMenu:.
            #[cfg(target_os = "macos")]
            {
                let _ = tray.with_inner_tray_icon(|inner| {
                    let Some(status) = inner.ns_status_item() else { return; };
                    unsafe {
                        let Some(menu) = status.menu(inner.mtm()) else { return; };
                        let button = status.button(inner.mtm()).unwrap();
                        let _: () = msg_send![&button, popUpStatusItemMenu: &*menu];
                    }
                });
            }
        }
        MouseButton::Middle => {}
    }
})
```

`popUpStatusItemMenu:` is an `NSButton` method (inherited from `NSStatusBarButton` / `NSStatusItem`) that displays the menu at the status item's screen position. It does not depend on `target`/`action`; it just shows the menu. This is exactly the system-call the button's wired action used to make.

### 4. Log a one-time info message

`log::info!("Applied macOS 27 click-routing override")` in step 2, so an operator inspecting logs can confirm the path was taken. No `cfg!` or runtime version check — the override is harmless on Tahoe and required on 27+, and a version check would be a YAGNI branch.

## What is NOT changed

- The `menu` builder call (still on `TrayIconBuilder`).
- The `on_menu_event` closure that handles item clicks.
- The `on_tray_icon_event` signature or its left-click → `show_panel` body.
- The `panel.rs` module — positioning, `setFrameTopLeftPoint:`, the `tauri-nspanel` setup are all out of scope.
- The Cargo dependencies — no new crates, no version bumps, no `[patch]`.
- macOS-26-only code paths or feature flags.

## Risks

1. **`with_inner_tray_icon` lifecycle.** Tauri 2.11.2 documents that the closure runs on the main thread and that the recommended practice is to pin a Tauri minor. We're not pinning — if a future 2.12 changes the closure's return semantics, the override step could break. Mitigation: the fix is small and self-contained; reverting it is one file.
2. **objc2_app_kit API drift.** `NSButton.setTarget`, `NSButton.setAction`, `NSStatusItem.menu`, and `popUpStatusItemMenu:` are all stable AppKit since macOS 10.0; we are not relying on macOS 27-specific symbols. The objc2 Rust bindings are the only moving piece. We confirmed `NSStatusItem.menu(mtm)` is exposed in `objc2-app-kit 0.3.2` (generated/NSStatusItem.rs:67-69).
3. **Right-click menu visual position.** The system path used to display the menu at the status item's button; `popUpStatusItemMenu:` does the same. If a future macOS release changes the visual placement, we notice and re-evaluate. Not a 27-Beta-1-specific risk.
4. **Concurrent clicks on the same icon.** `with_inner_tray_icon` queues onto the main thread; the per-click `Retained<NSMenu>` returned by `status.menu(mtm)` is dropped at the end of the right-click arm. No cross-thread state, no global storage.
5. **Lost work if the upstream `tauri-apps/tray-icon` lands a fix.** When the upstream crate patches this properly (likely an `update_tracking_areas` / `hitTest` change in `TaoTrayTarget`), we can revert this whole block. Marked with a comment pointing to issue #573 so the future-us knows the context.

## Testing

This is a macOS-runtime behavior fix. The unit-test surface in this repo (`vitest` for the JS side) does not cover the Rust tray code. Test strategy:

1. **Manual smoke test on macOS 27 Beta 1** (the regression target): left-click icon → panel appears; right-click icon → menu appears; both behave as on Tahoe.
2. **Manual smoke test on macOS 26 Tahoe** (the regression-avoidance target): same matrix, no change in behavior.
3. **Manual smoke test on Windows / Linux** (compile-time gate): `#[cfg(target_os = "macos")]` keeps the change from compiling outside macOS, so the `cargo check --target x86_64-pc-windows-msvc` and `--target x86_64-unknown-linux-gnu` runs are the cross-platform test.
4. **Static log assertion** in dev: the new `log::info!("Applied macOS 27 click-routing override")` line should appear once on startup.
5. **No regression test in the strict sense.** A real regression test would need a virtualized macOS 27 host, which we don't have. The issue's repro steps become the manual test plan; closing the issue with "verified on 27.0 beta 1, build N" is the artifact.

## Doc updates

- `AGENTS.md` — no change (this is a fix, not a new convention).
- `CHANGELOG.md` — add a "Fixed" entry under the next version, referencing issue #573.
- `docs/superpowers/specs/` — this file.
- No new `docs/` page; the change is internal and the fix is documented in code.

## Open items

- (Resolved) Whether `tray.ns_status_item()` is reachable via the public Tauri 2 `Tray` API. **Yes**, via `with_inner_tray_icon` (tauri 2.11.2 line 633).
- (Resolved) Whether the fix can live entirely in `tray.rs`. **Yes**; no upstream crate fork required.
- (Resolved) How to obtain the `NSMenu` for the right-click pop-up. **By re-querying `NSStatusItem.menu(mtm)` at click time** (objc2_app_kit 0.3.2 generated/NSStatusItem.rs:67-69 returns `Option<Retained<NSMenu>>`). No retain storage, no dependency on `tauri::menu::Menu`'s private `attrs` field, no need to add `muda` as a direct dep.
- (Open, deferred to implementation) Exact `NSButton::setTarget` / `setAction` signatures from `objc2_app_kit 0.3` (probably `Option<&NSObject>` for target, `Option<Sel>` for action). Standard AppKit; the call site will need minor adaptation if the Rust signatures differ.
