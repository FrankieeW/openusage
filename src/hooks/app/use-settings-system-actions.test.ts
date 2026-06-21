import { act, renderHook, waitFor } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

const {
  copyLogPathMock,
  getEnabledPluginIdsMock,
  invokeMock,
  saveAutoUpdateIntervalMock,
  saveGlobalShortcutMock,
  saveLogLevelMock,
  saveStartOnLoginMock,
  saveUnsafeAllowAllEnvMock,
} = vi.hoisted(() => ({
  copyLogPathMock: vi.fn(),
  getEnabledPluginIdsMock: vi.fn(),
  saveAutoUpdateIntervalMock: vi.fn(),
  saveGlobalShortcutMock: vi.fn(),
  saveLogLevelMock: vi.fn(),
  saveStartOnLoginMock: vi.fn(),
  saveUnsafeAllowAllEnvMock: vi.fn(),
  invokeMock: vi.fn(),
}))

vi.mock("@tauri-apps/api/core", () => ({
  invoke: invokeMock,
}))

vi.mock("@/lib/settings", () => ({
  copyLogPath: copyLogPathMock,
  getEnabledPluginIds: getEnabledPluginIdsMock,
  saveAutoUpdateInterval: saveAutoUpdateIntervalMock,
  saveGlobalShortcut: saveGlobalShortcutMock,
  saveLogLevel: saveLogLevelMock,
  saveStartOnLogin: saveStartOnLoginMock,
  saveUnsafeAllowAllEnv: saveUnsafeAllowAllEnvMock,
}))

import { useSettingsSystemActions } from "@/hooks/app/use-settings-system-actions"

describe("useSettingsSystemActions", () => {
  beforeEach(() => {
    copyLogPathMock.mockReset()
    getEnabledPluginIdsMock.mockReset()
    saveAutoUpdateIntervalMock.mockReset()
    saveGlobalShortcutMock.mockReset()
    saveLogLevelMock.mockReset()
    saveStartOnLoginMock.mockReset()
    saveUnsafeAllowAllEnvMock.mockReset()
    invokeMock.mockReset()

    getEnabledPluginIdsMock.mockImplementation((settings: { order: string[]; disabled: string[] }) =>
      settings.order.filter((id) => !settings.disabled.includes(id))
    )
    saveAutoUpdateIntervalMock.mockResolvedValue(undefined)
    saveGlobalShortcutMock.mockResolvedValue(undefined)
    saveLogLevelMock.mockResolvedValue(undefined)
    saveStartOnLoginMock.mockResolvedValue(undefined)
    saveUnsafeAllowAllEnvMock.mockResolvedValue(undefined)
    copyLogPathMock.mockResolvedValue(undefined)
    invokeMock.mockResolvedValue(undefined)
  })

  it("updates auto refresh schedule when at least one plugin is enabled", () => {
    const nowSpy = vi.spyOn(Date, "now").mockReturnValue(10_000)
    const setAutoUpdateInterval = vi.fn()
    const setAutoUpdateNextAt = vi.fn()

    const { result } = renderHook(() =>
      useSettingsSystemActions({
        pluginSettings: { order: ["codex"], disabled: [] },
        setAutoUpdateInterval,
        setAutoUpdateNextAt,
        setGlobalShortcut: vi.fn(),
        setStartOnLogin: vi.fn(),
        setLogLevel: vi.fn(),
        setUnsafeAllowAllEnv: vi.fn(),
        applyStartOnLogin: vi.fn().mockResolvedValue(undefined),
        applyUnsafeAllowAllEnv: vi.fn().mockResolvedValue(undefined),
      })
    )

    act(() => {
      result.current.handleAutoUpdateIntervalChange(15)
    })

    expect(setAutoUpdateInterval).toHaveBeenCalledWith(15)
    expect(setAutoUpdateNextAt).toHaveBeenCalledWith(910_000)
    expect(saveAutoUpdateIntervalMock).toHaveBeenCalledWith(15)
    nowSpy.mockRestore()
  })

  it("clears next refresh when no enabled plugins remain", () => {
    const setAutoUpdateNextAt = vi.fn()

    const { result } = renderHook(() =>
      useSettingsSystemActions({
        pluginSettings: { order: ["codex"], disabled: ["codex"] },
        setAutoUpdateInterval: vi.fn(),
        setAutoUpdateNextAt,
        setGlobalShortcut: vi.fn(),
        setStartOnLogin: vi.fn(),
        setLogLevel: vi.fn(),
        setUnsafeAllowAllEnv: vi.fn(),
        applyStartOnLogin: vi.fn().mockResolvedValue(undefined),
        applyUnsafeAllowAllEnv: vi.fn().mockResolvedValue(undefined),
      })
    )

    act(() => {
      result.current.handleAutoUpdateIntervalChange(30)
    })

    expect(setAutoUpdateNextAt).toHaveBeenCalledWith(null)
  })

  it("updates shortcut and start-on-login settings", () => {
    const setGlobalShortcut = vi.fn()
    const setStartOnLogin = vi.fn()
    const setLogLevel = vi.fn()
    const applyStartOnLogin = vi.fn().mockResolvedValue(undefined)

    const { result } = renderHook(() =>
      useSettingsSystemActions({
        pluginSettings: null,
        setAutoUpdateInterval: vi.fn(),
        setAutoUpdateNextAt: vi.fn(),
        setGlobalShortcut,
        setStartOnLogin,
        setLogLevel,
        setUnsafeAllowAllEnv: vi.fn(),
        applyStartOnLogin,
        applyUnsafeAllowAllEnv: vi.fn().mockResolvedValue(undefined),
      })
    )

    act(() => {
      result.current.handleGlobalShortcutChange("CommandOrControl+Shift+O")
      result.current.handleStartOnLoginChange(true)
      result.current.handleLogLevelChange("debug")
    })

    expect(setGlobalShortcut).toHaveBeenCalledWith("CommandOrControl+Shift+O")
    expect(saveGlobalShortcutMock).toHaveBeenCalledWith("CommandOrControl+Shift+O")
    expect(invokeMock).toHaveBeenCalledWith("update_global_shortcut", {
      shortcut: "CommandOrControl+Shift+O",
    })

    expect(setStartOnLogin).toHaveBeenCalledWith(true)
    expect(saveStartOnLoginMock).toHaveBeenCalledWith(true)
    expect(applyStartOnLogin).toHaveBeenCalledWith(true)

    expect(setLogLevel).toHaveBeenCalledWith("debug")
    expect(saveLogLevelMock).toHaveBeenCalledWith("debug")
  })

  it("copies the log path", async () => {
    const { result } = renderHook(() =>
      useSettingsSystemActions({
        pluginSettings: null,
        setAutoUpdateInterval: vi.fn(),
        setAutoUpdateNextAt: vi.fn(),
        setGlobalShortcut: vi.fn(),
        setStartOnLogin: vi.fn(),
        setLogLevel: vi.fn(),
        setUnsafeAllowAllEnv: vi.fn(),
        applyStartOnLogin: vi.fn().mockResolvedValue(undefined),
        applyUnsafeAllowAllEnv: vi.fn().mockResolvedValue(undefined),
      })
    )

    await act(async () => {
      await result.current.handleCopyLogPath()
    })

    expect(copyLogPathMock).toHaveBeenCalledTimes(1)
  })

  it("logs persistence/update failures", async () => {
    const autoError = new Error("auto save failed")
    const shortcutSaveError = new Error("shortcut save failed")
    const shortcutInvokeError = new Error("shortcut invoke failed")
    const startOnLoginSaveError = new Error("start on login save failed")
    const startOnLoginApplyError = new Error("start on login apply failed")
    const logLevelSaveError = new Error("debug level save failed")
    const copyLogPathError = new Error("copy log path failed")
    const errorSpy = vi.spyOn(console, "error").mockImplementation(() => {})

    saveAutoUpdateIntervalMock.mockRejectedValueOnce(autoError)
    saveGlobalShortcutMock.mockRejectedValueOnce(shortcutSaveError)
    invokeMock.mockRejectedValueOnce(shortcutInvokeError)
    saveStartOnLoginMock.mockRejectedValueOnce(startOnLoginSaveError)
    saveLogLevelMock.mockRejectedValueOnce(logLevelSaveError)
    copyLogPathMock.mockRejectedValueOnce(copyLogPathError)
    const applyStartOnLogin = vi.fn().mockRejectedValueOnce(startOnLoginApplyError)

    const { result } = renderHook(() =>
      useSettingsSystemActions({
        pluginSettings: null,
        setAutoUpdateInterval: vi.fn(),
        setAutoUpdateNextAt: vi.fn(),
        setGlobalShortcut: vi.fn(),
        setStartOnLogin: vi.fn(),
        setLogLevel: vi.fn(),
        setUnsafeAllowAllEnv: vi.fn(),
        applyStartOnLogin,
        applyUnsafeAllowAllEnv: vi.fn().mockResolvedValue(undefined),
      })
    )

    act(() => {
      result.current.handleAutoUpdateIntervalChange(5)
      result.current.handleGlobalShortcutChange(null)
      result.current.handleStartOnLoginChange(false)
      result.current.handleLogLevelChange("trace")
      void result.current.handleCopyLogPath().catch(() => {})
    })

    await waitFor(() => {
      expect(errorSpy).toHaveBeenCalledWith("Failed to save auto-update interval:", autoError)
      expect(errorSpy).toHaveBeenCalledWith("Failed to save global shortcut:", shortcutSaveError)
      expect(errorSpy).toHaveBeenCalledWith("Failed to update global shortcut:", shortcutInvokeError)
      expect(errorSpy).toHaveBeenCalledWith("Failed to save start on login:", startOnLoginSaveError)
      expect(errorSpy).toHaveBeenCalledWith("Failed to update start on login:", startOnLoginApplyError)
      expect(errorSpy).toHaveBeenCalledWith("Failed to save debug level:", logLevelSaveError)
      expect(errorSpy).toHaveBeenCalledWith("Failed to copy log path:", copyLogPathError)
    })

    errorSpy.mockRestore()
  })
})
