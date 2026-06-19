import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { useHubStore, pluginLoadingKey } from "@/lib/hub/cache"
import {
  descriptionForSourceTrust,
  labelForPluginUpdatedAt,
  labelForSourceTrust,
  shortPackageHash,
} from "@/lib/hub/labels"
import type { PluginInfo, SkippedPlugin, Source } from "@/lib/hub/types"
import { cn } from "@/lib/utils"
import { useState } from "react"

interface PluginBrowserProps {
  sourceId: string
  source?: Source
  available: PluginInfo[]
  skipped: SkippedPlugin[]
}

export function PluginBrowser({
  sourceId,
  source,
  available,
  skipped,
}: PluginBrowserProps) {
  const loading = useHubStore((s) => s.loading.perSource[sourceId])
  const install = useHubStore((s) => s.install)
  const switchSource = useHubStore((s) => s.switchSource)
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
          {available.length} Plugin{available.length === 1 ? "" : "s"}
          {skipped.length > 0 ? `, ${skipped.length} Skipped` : ""}
        </span>
      </div>

      {skipped.length > 0 && (
        <Alert variant="destructive" className="mx-4">
          <AlertTitle>Plugins Skipped</AlertTitle>
          <AlertDescription>
            <details>
              <summary className="cursor-pointer text-xs">
                Show Skipped Reasons
              </summary>
              <ul className="mt-2 list-disc pl-4 text-xs">
                {skipped.map((skippedPlugin) => (
                  <li key={skippedPlugin.path}>
                    <code>
                      {skippedPlugin.path.split("/").slice(-2).join("/")}
                    </code>
                    : {skippedPlugin.reason}
                  </li>
                ))}
              </ul>
            </details>
          </AlertDescription>
        </Alert>
      )}

      {available.length === 0 && (
        <div className="px-4 py-3 text-sm text-muted-foreground">
          No Plugins Found In This Source.
        </div>
      )}

      <div className="space-y-1.5 px-4">
        {available.map((plugin) => (
          <PluginCard
            key={plugin.id}
            plugin={plugin}
            source={source}
            loading={loading}
            onInstall={() => install(sourceId, plugin.id)}
            onSwitchSource={() => switchSource(sourceId, plugin.id)}
            onUninstall={() => uninstall(plugin.id, plugin.sourceId || undefined)}
          />
        ))}
      </div>
    </div>
  )
}

interface PluginCardProps {
  plugin: PluginInfo
  source?: Source
  loading: boolean
  onInstall: () => void
  onSwitchSource: () => void
  onUninstall: () => void
}

type PreviewAction = "install" | "switch"

function PluginCard({
  plugin,
  source,
  loading,
  onInstall,
  onSwitchSource,
  onUninstall,
}: PluginCardProps) {
  const [previewAction, setPreviewAction] = useState<PreviewAction | null>(null)
  const installKey = pluginLoadingKey(plugin.sourceId, plugin.id)
  const uninstallKey = `uninstall:${plugin.id}`
  const installLoading = useHubStore(
    (s) => s.loading.perPlugin[installKey] === "install",
  )
  const uninstallLoading = useHubStore(
    (s) => s.loading.perPlugin[uninstallKey] === "uninstall",
  )
  const statusLabel = packageStatusLabel(plugin)
  const statusDescription = packageStatusDescription(plugin)
  const primaryAction = packagePrimaryAction(plugin)
  const updatedAtLabel = labelForPluginUpdatedAt(plugin.updatedAt)
  const actionLoading = installLoading

  function confirmPreview() {
    const action = previewAction
    setPreviewAction(null)
    if (action === "switch") {
      onSwitchSource()
    } else if (action === "install") {
      onInstall()
    }
  }

  return (
    <div
      data-testid="hub-plugin-card"
      data-plugin-id={plugin.id}
      className={cn(
        "space-y-1.5 rounded-md border border-border bg-card px-3 py-2",
        (installLoading || uninstallLoading) && "opacity-60",
        plugin.unmanaged && "border-dashed",
      )}
    >
      <div className="min-w-0">
        <span className="block truncate text-sm font-medium" title={plugin.name}>
          {plugin.name}
        </span>
      </div>
      {statusDescription && (
        <p className="text-xs text-muted-foreground">{statusDescription}</p>
      )}
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-xs text-muted-foreground">
          v{plugin.availableVersion}
        </span>
        {updatedAtLabel && (
          <span className="text-xs text-muted-foreground">
            {updatedAtLabel}
          </span>
        )}
        {statusLabel && (
          <span
            className={cn(
              "min-w-0 max-w-full truncate text-xs",
              plugin.packageStatus === "updateAvailable" && "text-blue-600",
              plugin.packageStatus === "sourceChanged" && "text-amber-600",
              plugin.packageStatus === "installedNewerThanSource" &&
                "text-violet-600",
              (plugin.packageStatus === "samePackageFromOtherSource" ||
                plugin.packageStatus === "differentPackageSamePluginId") &&
                "text-muted-foreground",
            )}
            title={statusLabel}
          >
            {statusLabel}
          </span>
        )}
        {primaryAction && (
          <Button
            size="xs"
            variant="default"
            onClick={() => setPreviewAction(primaryAction.kind)}
            disabled={actionLoading || loading}
            data-testid={
              primaryAction.kind === "switch"
                ? "hub-switch-source-button"
                : "hub-install-button"
            }
          >
            {actionLoading ? "Working..." : primaryAction.label}
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
            {uninstallLoading ? "Removing..." : "Uninstall"}
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
      {previewAction && (
        <InstallPreviewDialog
          action={previewAction}
          plugin={plugin}
          source={source}
          confirming={actionLoading}
          onCancel={() => setPreviewAction(null)}
          onConfirm={confirmPreview}
        />
      )}
    </div>
  )
}

function packageStatusLabel(plugin: PluginInfo) {
  switch (plugin.packageStatus) {
    case "installed":
      return "Installed"
    case "updateAvailable":
      return "Update Available"
    case "sourceChanged":
      return "Installed From Another Source"
    case "installedNewerThanSource":
      return "Installed Newer Than Source"
    case "samePackageFromOtherSource":
      return "Same Package, Already Installed"
    case "differentPackageSamePluginId":
      return "Different Package, Same Plugin ID"
    case "unmanagedInstalled":
      return "Local Install"
    case "orphanedSource":
      return "Source Removed"
    case "notInstalled":
      return null
  }
}

function packageStatusDescription(plugin: PluginInfo): string | null {
  switch (plugin.packageStatus) {
    case "updateAvailable":
      return plugin.installedVersion
        ? `Installed v${plugin.installedVersion}; source provides v${plugin.availableVersion}.`
        : `Source provides v${plugin.availableVersion}.`
    case "sourceChanged":
      return "The source now provides a different package hash for the installed version."
    case "installedNewerThanSource":
      return "The installed copy is newer, so downgrade is not offered by default."
    case "samePackageFromOtherSource":
      return "The installed package hash matches this source."
    case "differentPackageSamePluginId":
      return "This source uses the same plugin ID but a different package hash."
    case "orphanedSource":
      return "The source was removed, but the local plugin copy remains."
    case "unmanagedInstalled":
      return "This plugin exists locally without Hub source metadata."
    case "installed":
    case "notInstalled":
      return null
  }
}

function packagePrimaryAction(
  plugin: PluginInfo,
): { kind: PreviewAction; label: string } | null {
  switch (plugin.packageStatus) {
    case "notInstalled":
      return { kind: "install", label: "Install" }
    case "updateAvailable":
      return { kind: "install", label: "Update" }
    case "sourceChanged":
      return { kind: "install", label: "Review Package" }
    case "differentPackageSamePluginId":
      return { kind: "switch", label: "Switch Source" }
    case "installed":
    case "installedNewerThanSource":
    case "samePackageFromOtherSource":
    case "unmanagedInstalled":
    case "orphanedSource":
      return null
  }
}

interface InstallPreviewDialogProps {
  action: PreviewAction
  plugin: PluginInfo
  source?: Source
  confirming: boolean
  onCancel: () => void
  onConfirm: () => void
}

function InstallPreviewDialog({
  action,
  plugin,
  source,
  confirming,
  onCancel,
  onConfirm,
}: InstallPreviewDialogProps) {
  const sourceLabel = source?.label ?? "Local"
  const sourceUrl = source?.url ?? "Local Plugin Directory"
  const branch = source?.branch ?? "Default"
  const trustLabel = labelForSourceTrust(source)
  const replacementSummary = plugin.installedVersion
    ? "Replaces Existing Plugin"
    : "No Existing Hub Install"
  const replacementText = plugin.installedVersion
    ? `v${plugin.installedVersion} -> v${plugin.availableVersion}`
    : `Install v${plugin.availableVersion}`
  const isDifferentPackage = plugin.packageStatus === "differentPackageSamePluginId"
  const isUnknownGit = trustLabel === "Unknown Git Source"
  const confirmLabel = action === "switch" ? "Confirm Switch" : "Confirm Install"

  return (
    <div
      data-testid="hub-install-preview"
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 px-4"
      onClick={(event) => {
        if (event.target === event.currentTarget) onCancel()
      }}
    >
      <div className="w-full max-w-lg space-y-4 rounded-md border border-border bg-card p-5 shadow-lg">
        <div>
          <h2 className="text-base font-semibold">Install Preview</h2>
          <p className="text-xs text-muted-foreground">
            Review The Package And Source Before Continuing.
          </p>
        </div>

        {(isDifferentPackage || isUnknownGit) && (
          <div
            data-testid="hub-install-preview-warning"
            className="rounded-md border border-amber-300 bg-amber-50 px-3 py-2 text-xs text-amber-900"
          >
            {isDifferentPackage
              ? "Different Package: This source has the same plugin ID but a different package hash."
              : "Unknown Git Source: Confirm that you trust this repository before installing."}
          </div>
        )}

        <dl className="grid grid-cols-[120px_1fr] gap-x-3 gap-y-2 text-sm">
          <dt className="text-muted-foreground">Plugin ID</dt>
          <dd className="font-mono text-xs">{plugin.id}</dd>
          <dt className="text-muted-foreground">Name</dt>
          <dd>{plugin.name}</dd>
          <dt className="text-muted-foreground">Version</dt>
          <dd>{plugin.availableVersion}</dd>
          <dt className="text-muted-foreground">Source</dt>
          <dd>{sourceLabel}</dd>
          <dt className="text-muted-foreground">Trust</dt>
          <dd>
            {trustLabel}
            <span className="ml-2 text-xs text-muted-foreground">
              {descriptionForSourceTrust(source)}
            </span>
          </dd>
          <dt className="text-muted-foreground">URL</dt>
          <dd className="break-all font-mono text-xs">{sourceUrl}</dd>
          <dt className="text-muted-foreground">Branch</dt>
          <dd>{branch}</dd>
          <dt className="text-muted-foreground">Package Hash</dt>
          <dd className="font-mono text-xs">
            {shortPackageHash(plugin.packageHash)}
          </dd>
          <dt className="text-muted-foreground">Replacement</dt>
          <dd>{replacementSummary}</dd>
          <dt className="text-muted-foreground">Change</dt>
          <dd>{replacementText}</dd>
        </dl>

        <div className="flex justify-end gap-2">
          <Button size="sm" variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
          <Button
            size="sm"
            onClick={onConfirm}
            disabled={confirming}
            data-testid="hub-install-preview-confirm"
          >
            {confirming ? "Working..." : confirmLabel}
          </Button>
        </div>
      </div>
    </div>
  )
}
