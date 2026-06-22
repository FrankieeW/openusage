import { useEffect } from "react"
import { isTauri } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { isLogLevel, type LogLevel } from "@/lib/settings"

type UseLogLevelSyncArgs = {
  setLogLevel: (value: LogLevel) => void
}

// Keep the Settings page in sync when the log level is changed from the tray
// menu. The tray persists the change and emits `tray:log-level`; here we mirror
// it into the frontend store so the Debug Level buttons reflect the new value.
export function useLogLevelSync({ setLogLevel }: UseLogLevelSyncArgs) {
  useEffect(() => {
    if (!isTauri()) return
    let cancelled = false
    let unlisten: (() => void) | undefined

    void listen<string>("tray:log-level", (event) => {
      if (isLogLevel(event.payload)) {
        setLogLevel(event.payload)
      }
    })
      .then((fn) => {
        if (cancelled) {
          fn()
          return
        }
        unlisten = fn
      })
      .catch((error) => {
        console.error("Failed to subscribe to tray debug level changes:", error)
      })

    return () => {
      cancelled = true
      unlisten?.()
    }
  }, [setLogLevel])
}
