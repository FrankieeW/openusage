import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { cn } from "@/lib/utils"
import { useHubStore, pluginLoadingKey } from "@/lib/hub/cache"
import type { PluginInfo, SkippedPlugin } from "@/lib/hub/types"

interface PluginBrowserProps {
  sourceId: string
  available: PluginInfo[]
  skipped: SkippedPlugin[]
}

export function PluginBrowser({ sourceId, available, skipped }: PluginBrowserProps) {
  const loading = useHubStore((s) => s.loading.perSource[sourceId])
  const install = useHubStore((s) => s.install)
  const uninstall = useHubStore((s) => s.uninstall)

  if (loading && available.length === 0) {
    return (
      <div className="space-y-2 px-4 pb-4">
        <Skeleton className="h-10 w-full" />
        <Skeleton className="h-10 w-full" />
      </div>
    )
  }

  return (
    <div className="space-y-1 pb-3">
      <div className="px-1 py-1.5">
        <span className="text-xs text-muted-foreground">
          {available.length} plugin{available.length === 1 ? "" : "s"}
          {skipped.length > 0 ? `, ${skipped.length} skipped` : ""}
        </span>
      </div>

      {skipped.length > 0 && (
        <Alert variant="destructive" className="mx-4">
          <AlertTitle>Plugins Skipped</AlertTitle>
          <AlertDescription>
            <ul className="list-disc pl-4 text-xs">
              {skipped.map((s) => (
                <li key={s.path}>
                  <code>{s.path.split("/").slice(-2).join("/")}</code>: {s.reason}
                </li>
              ))}
            </ul>
          </AlertDescription>
        </Alert>
      )}

      {available.length === 0 && (
        <div className="px-4 py-3 text-sm text-muted-foreground">
          No plugins found in this source.
        </div>
      )}

      <div className="space-y-1.5 px-4">
        {available.map((plugin) => (
          <PluginCard
            key={plugin.id}
            plugin={plugin}
            loading={loading}
            onInstall={() => install(sourceId, plugin.id)}
            onUninstall={() => uninstall(plugin.id, plugin.sourceId || undefined)}
          />
        ))}
      </div>
    </div>
  )
}

interface PluginCardProps {
  plugin: PluginInfo
  loading: boolean
  onInstall: () => void
  onUninstall: () => void
}

function PluginCard({ plugin, loading, onInstall, onUninstall }: PluginCardProps) {
  const installKey = pluginLoadingKey(plugin.sourceId, plugin.id)
  const uninstallKey = `uninstall:${plugin.id}`
  const installLoading = useHubStore(
    (s) => s.loading.perPlugin[installKey] === "install",
  )
  const uninstallLoading = useHubStore(
    (s) => s.loading.perPlugin[uninstallKey] === "uninstall",
  )
  const statusLabel = packageStatusLabel(plugin)
  const canInstall = plugin.packageStatus === "notInstalled"
  const canUpdate =
    plugin.installed &&
    (plugin.packageStatus === "updateAvailable" ||
      plugin.packageStatus === "sourceChanged")
  const installButtonLabel =
    plugin.packageStatus === "sourceChanged" ? "Reinstall" : "Update"

  return (
    <div
      data-testid="hub-plugin-card"
      data-plugin-id={plugin.id}
      className={cn(
        "rounded-md border border-border bg-card px-3 py-2 space-y-1.5",
        (installLoading || uninstallLoading) && "opacity-60",
        plugin.unmanaged && "border-dashed",
      )}
    >
      <div className="flex items-center gap-2">
        <span className="truncate text-sm font-medium">{plugin.name}</span>
        {statusLabel && (
          <span
            className={cn(
              "shrink-0 text-xs",
              plugin.packageStatus === "updateAvailable" && "text-blue-600",
              plugin.packageStatus === "sourceChanged" && "text-amber-600",
              plugin.packageStatus === "installedNewerThanSource" && "text-violet-600",
              (plugin.packageStatus === "samePackageFromOtherSource" ||
                plugin.packageStatus === "differentPackageSamePluginId") &&
                "text-muted-foreground",
            )}
          >
            {statusLabel}
          </span>
        )}
      </div>
      <div className="flex items-center gap-2">
        <span className="text-xs text-muted-foreground">v{plugin.availableVersion}</span>
        {canInstall && (
          <Button
            size="xs"
            variant="default"
            onClick={onInstall}
            disabled={installLoading || loading}
            data-testid="hub-install-button"
          >
            {installLoading ? "Installing…" : "Install"}
          </Button>
        )}
        {canUpdate && (
          <Button
            size="xs"
            variant="default"
            onClick={onInstall}
            disabled={installLoading || loading}
            data-testid="hub-install-button"
          >
            {installLoading ? "Installing…" : installButtonLabel}
          </Button>
        )}
        {plugin.installed && !plugin.unmanaged && (
          <Button
            size="xs"
            variant="outline"
            onClick={onUninstall}
            disabled={uninstallLoading || loading}
            data-testid="hub-uninstall-button"
          >
            {uninstallLoading ? "Removing…" : "Uninstall"}
          </Button>
        )}
        {plugin.unmanaged && (
          <Button
            size="xs"
            variant="outline"
            onClick={onUninstall}
            disabled={uninstallLoading || loading}
          >
            Remove
          </Button>
        )}
      </div>
    </div>
  )
}

function packageStatusLabel(plugin: PluginInfo) {
  switch (plugin.packageStatus) {
    case "installed":
      return "Installed"
    case "updateAvailable":
      return plugin.installedVersion
        ? `Update From v${plugin.installedVersion}`
        : "Update Available"
    case "sourceChanged":
      return "Package Changed"
    case "installedNewerThanSource":
      return "Installed Newer"
    case "samePackageFromOtherSource":
      return "Same Package Installed"
    case "differentPackageSamePluginId":
      return "Different Package Installed"
    case "unmanagedInstalled":
      return "Local Install"
    case "orphanedSource":
      return "Source Removed"
    case "notInstalled":
      return null
  }
}
