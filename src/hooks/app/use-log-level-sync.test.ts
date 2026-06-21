import { renderHook } from "@testing-library/react"
import { describe, expect, it, vi, beforeEach } from "vitest"

const { listeners, listenMock, isTauriMock } = vi.hoisted(() => ({
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
  listenMock: vi.fn(),
  isTauriMock: vi.fn(),
}))

vi.mock("@tauri-apps/api/event", () => ({
  listen: listenMock,
}))

vi.mock("@tauri-apps/api/core", () => ({
  isTauri: isTauriMock,
}))

import { useLogLevelSync } from "@/hooks/app/use-log-level-sync"

describe("useLogLevelSync", () => {
  beforeEach(() => {
    listeners.clear()
    listenMock.mockReset()
    isTauriMock.mockReset()
    isTauriMock.mockReturnValue(true)
    listenMock.mockImplementation(async (event: string, cb: (event: { payload: unknown }) => void) => {
      listeners.set(event, cb)
      return () => listeners.delete(event)
    })
  })

  it("mirrors a valid tray log level into the store", async () => {
    const setLogLevel = vi.fn()
    renderHook(() => useLogLevelSync({ setLogLevel }))
    await Promise.resolve()

    listeners.get("tray:log-level")?.({ payload: "debug" })
    expect(setLogLevel).toHaveBeenCalledWith("debug")
  })

  it("ignores invalid payloads", async () => {
    const setLogLevel = vi.fn()
    renderHook(() => useLogLevelSync({ setLogLevel }))
    await Promise.resolve()

    listeners.get("tray:log-level")?.({ payload: "nonsense" })
    expect(setLogLevel).not.toHaveBeenCalled()
  })

  it("does not subscribe outside Tauri", () => {
    isTauriMock.mockReturnValue(false)
    const setLogLevel = vi.fn()
    renderHook(() => useLogLevelSync({ setLogLevel }))
    expect(listenMock).not.toHaveBeenCalled()
  })

  it("unsubscribes on unmount", async () => {
    const setLogLevel = vi.fn()
    const { unmount } = renderHook(() => useLogLevelSync({ setLogLevel }))
    await Promise.resolve()
    expect(listeners.has("tray:log-level")).toBe(true)

    unmount()
    expect(listeners.has("tray:log-level")).toBe(false)
  })
})
