import { useCallback } from "react"
import { invoke } from "@tauri-apps/api/core"
import {
  copyLogPath,
  getEnabledPluginIds,
  saveAutoUpdateInterval,
  saveGlobalShortcut,
  saveLogLevel,
  saveStartOnLogin,
  saveUnsafeAllowAllEnv,
  type AutoUpdateIntervalMinutes,
  type GlobalShortcut,
  type LogLevel,
  type PluginSettings,
} from "@/lib/settings"

type UseSettingsSystemActionsArgs = {
  pluginSettings: PluginSettings | null
  setAutoUpdateInterval: (value: AutoUpdateIntervalMinutes) => void
  setAutoUpdateNextAt: (value: number | null) => void
  setGlobalShortcut: (value: GlobalShortcut) => void
  setStartOnLogin: (value: boolean) => void
  setLogLevel: (value: LogLevel) => void
  setUnsafeAllowAllEnv: (value: boolean) => void
  applyStartOnLogin: (value: boolean) => Promise<void>
  applyUnsafeAllowAllEnv: (value: boolean) => Promise<void>
}

export function useSettingsSystemActions({
  pluginSettings,
  setAutoUpdateInterval,
  setAutoUpdateNextAt,
  setGlobalShortcut,
  setStartOnLogin,
  setLogLevel,
  setUnsafeAllowAllEnv,
  applyStartOnLogin,
  applyUnsafeAllowAllEnv,
}: UseSettingsSystemActionsArgs) {
  const handleAutoUpdateIntervalChange = useCallback((value: AutoUpdateIntervalMinutes) => {
    setAutoUpdateInterval(value)

    if (pluginSettings) {
      const enabledIds = getEnabledPluginIds(pluginSettings)
      if (enabledIds.length > 0) {
        setAutoUpdateNextAt(Date.now() + value * 60_000)
      } else {
        setAutoUpdateNextAt(null)
      }
    }

    void saveAutoUpdateInterval(value).catch((error) => {
      console.error("Failed to save auto-update interval:", error)
    })
  }, [pluginSettings, setAutoUpdateInterval, setAutoUpdateNextAt])

  const handleGlobalShortcutChange = useCallback((value: GlobalShortcut) => {
    setGlobalShortcut(value)
    void saveGlobalShortcut(value).catch((error) => {
      console.error("Failed to save global shortcut:", error)
    })
    invoke("update_global_shortcut", { shortcut: value }).catch((error) => {
      console.error("Failed to update global shortcut:", error)
    })
  }, [setGlobalShortcut])

  const handleStartOnLoginChange = useCallback((value: boolean) => {
    setStartOnLogin(value)
    void saveStartOnLogin(value).catch((error) => {
      console.error("Failed to save start on login:", error)
    })
    void applyStartOnLogin(value).catch((error) => {
      console.error("Failed to update start on login:", error)
    })
  }, [applyStartOnLogin, setStartOnLogin])

  const handleLogLevelChange = useCallback((value: LogLevel) => {
    setLogLevel(value)
    void saveLogLevel(value).catch((error) => {
      console.error("Failed to save debug level:", error)
    })
  }, [setLogLevel])

  const handleCopyLogPath = useCallback(async () => {
    try {
      await copyLogPath()
    } catch (error) {
      console.error("Failed to copy log path:", error)
      throw error
    }
  }, [])

  const handleUnsafeAllowAllEnvChange = useCallback((value: boolean) => {
    setUnsafeAllowAllEnv(value)
    void saveUnsafeAllowAllEnv(value).catch((error) => {
      console.error("Failed to save unsafe allow-all-env:", error)
    })
    void applyUnsafeAllowAllEnv(value).catch((error) => {
      console.error("Failed to apply unsafe allow-all-env:", error)
    })
  }, [applyUnsafeAllowAllEnv, setUnsafeAllowAllEnv])

  return {
    handleAutoUpdateIntervalChange,
    handleGlobalShortcutChange,
    handleStartOnLoginChange,
    handleLogLevelChange,
    handleCopyLogPath,
    handleUnsafeAllowAllEnvChange,
  }
}
