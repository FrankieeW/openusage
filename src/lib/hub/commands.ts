import { invoke } from "@tauri-apps/api/core"
import type {
  HubBrowseView,
  HubError,
  Source,
  UpdateInfo,
} from "./types"
import { isHubError } from "./types"

// Tauri throws the Rust HubError as the rejection value. We normalize the
// shape here so callers can rely on `HubError` consistently.

function toHubError(err: unknown): HubError {
  if (isHubError(err)) return err
  return {
    code: "IoError",
    message: typeof err === "string" ? err : String(err),
  }
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args)
  } catch (err) {
    throw toHubError(err)
  }
}

export const hubCommands = {
  listSources: () => call<Source[]>("hub_list_sources"),
  addSource: (url: string, label?: string) =>
    call<Source>("hub_add_source", { url, label: label ?? null }),
  removeSource: (sourceId: string) =>
    call<void>("hub_remove_source", { sourceId }),
  browseSource: (sourceId: string) =>
    call<HubBrowseView>("hub_browse_source", { sourceId }),
  install: (sourceId: string, pluginId: string) =>
    call<void>("hub_install", { sourceId, pluginId }),
  uninstall: (pluginId: string) =>
    call<void>("hub_uninstall", { pluginId }),
  refreshSource: (sourceId: string) =>
    call<HubBrowseView>("hub_refresh_source", { sourceId }),
  checkUpdates: () => call<UpdateInfo[]>("hub_check_updates"),
}