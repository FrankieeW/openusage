import {
  DndContext,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  type DragEndEvent,
} from "@dnd-kit/core";
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  verticalListSortingStrategy,
} from "@dnd-kit/sortable";
import { RefreshCw } from "lucide-react";
import { Checkbox } from "@/components/ui/checkbox";
import { Button } from "@/components/ui/button";
import { GlobalShortcutSection } from "@/components/global-shortcut-section";
import {
  LOG_LEVEL_OPTIONS,
  type AutoUpdateIntervalMinutes,
  type DisplayMode,
  type GlobalShortcut,
  type LogLevel,
  type MenubarIconStyle,
  type MenubarMetric,
  type ResetTimerDisplayMode,
  type ThemeMode,
  type TimeFormatMode,
} from "@/lib/settings";
import type { TraySettingsPreview } from "@/hooks/app/use-tray-icon";
import { cn } from "@/lib/utils";
import { useState } from "react";
import { hubCommands } from "@/lib/hub/commands";
import { DisplaySettingsSections } from "@/pages/settings/display-settings-sections";
import { SegmentedRadioGroup } from "@/pages/settings/segmented-radio-group";
import {
  SortablePluginItem,
  type PluginConfig,
} from "@/pages/settings/sortable-plugin-item";

type CopyLogPathStatus = "idle" | "copying" | "copied" | "failed";

interface SettingsPageProps {
  plugins: PluginConfig[];
  onReorder: (orderedIds: string[]) => void;
  onToggle: (id: string) => void;
  autoUpdateInterval: AutoUpdateIntervalMinutes;
  onAutoUpdateIntervalChange: (value: AutoUpdateIntervalMinutes) => void;
  themeMode: ThemeMode;
  onThemeModeChange: (value: ThemeMode) => void;
  displayMode: DisplayMode;
  onDisplayModeChange: (value: DisplayMode) => void;
  resetTimerDisplayMode: ResetTimerDisplayMode;
  onResetTimerDisplayModeChange: (value: ResetTimerDisplayMode) => void;
  timeFormatMode: TimeFormatMode;
  onTimeFormatModeChange: (value: TimeFormatMode) => void;
  menubarIconStyle: MenubarIconStyle;
  onMenubarIconStyleChange: (value: MenubarIconStyle) => void;
  menubarMetric: MenubarMetric;
  onMenubarMetricChange: (value: MenubarMetric) => void;
  logLevel: LogLevel;
  onLogLevelChange: (value: LogLevel) => void;
  onCopyLogPath: () => Promise<void>;
  traySettingsPreview: TraySettingsPreview;
  globalShortcut: GlobalShortcut;
  onGlobalShortcutChange: (value: GlobalShortcut) => void;
  startOnLogin: boolean;
  onStartOnLoginChange: (value: boolean) => void;
  unsafeAllowAllEnv: boolean;
  onUnsafeAllowAllEnvChange: (value: boolean) => void;
}

export function SettingsPage({
  plugins,
  onReorder,
  onToggle,
  autoUpdateInterval,
  onAutoUpdateIntervalChange,
  themeMode,
  onThemeModeChange,
  displayMode,
  onDisplayModeChange,
  resetTimerDisplayMode,
  onResetTimerDisplayModeChange,
  timeFormatMode,
  onTimeFormatModeChange,
  menubarIconStyle,
  onMenubarIconStyleChange,
  menubarMetric,
  onMenubarMetricChange,
  logLevel,
  onLogLevelChange,
  onCopyLogPath,
  traySettingsPreview,
  globalShortcut,
  onGlobalShortcutChange,
  startOnLogin,
  onStartOnLoginChange,
  unsafeAllowAllEnv,
  onUnsafeAllowAllEnvChange,
}: SettingsPageProps) {
  const sensors = useSensors(
    useSensor(PointerSensor),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    })
  );

  const [reloadingPlugins, setReloadingPlugins] = useState(false)
  const [copyLogPathStatus, setCopyLogPathStatus] = useState<CopyLogPathStatus>("idle")

  const copyLogPathLabel = copyLogPathStatus === "copying"
    ? "Copying"
    : copyLogPathStatus === "copied"
      ? "Copied"
      : copyLogPathStatus === "failed"
        ? "Copy Failed"
        : "Copy Log Path"

  const handleCopyLogPath = async () => {
    if (copyLogPathStatus === "copying") return
    setCopyLogPathStatus("copying")
    try {
      await onCopyLogPath()
      setCopyLogPathStatus("copied")
    } catch {
      setCopyLogPathStatus("failed")
    }
  }

  const handleReloadPlugins = async () => {
    if (reloadingPlugins) return
    setReloadingPlugins(true)
    try {
      await hubCommands.reloadPlugins()
    } catch (error) {
      console.error("Failed to reload plugins:", error)
    } finally {
      setReloadingPlugins(false)
    }
  }

  const handleDragEnd = (event: DragEndEvent) => {
    const { active, over } = event;

    if (over && active.id !== over.id) {
      const oldIndex = plugins.findIndex((item) => item.id === active.id);
      const newIndex = plugins.findIndex((item) => item.id === over.id);
      if (oldIndex === -1 || newIndex === -1) return;
      const next = arrayMove(plugins, oldIndex, newIndex);
      onReorder(next.map((item) => item.id));
    }
  };

  return (
    <div className="py-3 space-y-4">
      <DisplaySettingsSections
        autoUpdateInterval={autoUpdateInterval}
        onAutoUpdateIntervalChange={onAutoUpdateIntervalChange}
        themeMode={themeMode}
        onThemeModeChange={onThemeModeChange}
        displayMode={displayMode}
        onDisplayModeChange={onDisplayModeChange}
        resetTimerDisplayMode={resetTimerDisplayMode}
        onResetTimerDisplayModeChange={onResetTimerDisplayModeChange}
        timeFormatMode={timeFormatMode}
        onTimeFormatModeChange={onTimeFormatModeChange}
        menubarIconStyle={menubarIconStyle}
        onMenubarIconStyleChange={onMenubarIconStyleChange}
        menubarMetric={menubarMetric}
        onMenubarMetricChange={onMenubarMetricChange}
        traySettingsPreview={traySettingsPreview}
      />
      <GlobalShortcutSection
        globalShortcut={globalShortcut}
        onGlobalShortcutChange={onGlobalShortcutChange}
      />
      <section>
        <h3 className="text-lg font-semibold mb-0">Debug Level</h3>
        <p className="text-sm text-muted-foreground mb-2">
          How much detail goes into logs
        </p>
        <div className="bg-muted/50 rounded-lg p-1">
          <div className="space-y-1" role="group" aria-label="Debug tools">
            <SegmentedRadioGroup label="Debug level" className="grid grid-cols-3">
              {LOG_LEVEL_OPTIONS.map((option) => {
                const isActive = option.value === logLevel;
                return (
                  <Button
                    key={option.value}
                    type="button"
                    role="radio"
                    aria-checked={isActive}
                    tabIndex={isActive ? 0 : -1}
                    variant={isActive ? "default" : "outline"}
                    size="sm"
                    className="min-w-0"
                    onClick={() => onLogLevelChange(option.value)}
                  >
                    {option.label}
                  </Button>
                );
              })}
            </SegmentedRadioGroup>
            <Button
              type="button"
              variant="outline"
              size="sm"
              className={cn(
                "w-full min-w-0 px-1 text-[11px] leading-tight whitespace-normal",
                "border-emerald-500/50 text-emerald-700 hover:bg-emerald-500/10 hover:text-emerald-800",
                "dark:border-emerald-400/50 dark:text-emerald-300 dark:hover:bg-emerald-400/10",
                copyLogPathStatus === "copied" && "bg-emerald-500/10",
                copyLogPathStatus === "failed" && "border-destructive/50 text-destructive hover:bg-destructive/10 hover:text-destructive"
              )}
              disabled={copyLogPathStatus === "copying"}
              onClick={handleCopyLogPath}
            >
              {copyLogPathLabel}
            </Button>
          </div>
        </div>
      </section>
      <section>
        <h3 className="text-lg font-semibold mb-0">Start on Login</h3>
        <p className="text-sm text-muted-foreground mb-2">
          OpenUsage starts when you sign in
        </p>
        <label className="flex items-center gap-2 text-sm select-none text-foreground">
          <Checkbox
            key={`start-on-login-${startOnLogin}`}
            checked={startOnLogin}
            onCheckedChange={(checked) => onStartOnLoginChange(checked === true)}
          />
          Start on login
        </label>
      </section>
      <section>
        <h3 className="text-lg font-semibold mb-0">Plugin Environment Access</h3>
        <p className="text-sm text-muted-foreground mb-2">
          Off by default. Only enable for sources you trust.
        </p>
        <label className="flex items-center gap-2 text-sm select-none text-foreground">
          <Checkbox
            key={`unsafe-allow-all-env-${unsafeAllowAllEnv}`}
            checked={unsafeAllowAllEnv}
            onCheckedChange={(checked) => onUnsafeAllowAllEnvChange(checked === true)}
          />
          <span className="text-red-600 dark:text-red-400 font-medium">
            Allow plugins to read all environment variables (unsafe)
          </span>
        </label>
      </section>
      <section>
        <div className="flex items-center justify-between mb-1">
          <h3 className="text-lg font-semibold mb-0">Plugins</h3>
          <Button
            size="xs"
            variant="ghost"
            onClick={handleReloadPlugins}
            disabled={reloadingPlugins}
            data-testid="settings-reload-plugins"
            aria-label="Reload Plugins"
          >
            <RefreshCw size={12} className={reloadingPlugins ? "animate-spin" : undefined} />
            {reloadingPlugins ? "Reloading" : "Reload Plugins"}
          </Button>
        </div>
        <p className="text-sm text-muted-foreground mb-2">
          Your AI coding lineup
        </p>
        <div className="bg-muted/50 rounded-lg p-1 space-y-1">
          <DndContext
            sensors={sensors}
            collisionDetection={closestCenter}
            onDragEnd={handleDragEnd}
          >
            <SortableContext
              items={plugins.map((p) => p.id)}
              strategy={verticalListSortingStrategy}
            >
              {plugins.map((plugin) => (
                <SortablePluginItem
                  key={plugin.id}
                  plugin={plugin}
                  onToggle={onToggle}
                />
              ))}
            </SortableContext>
          </DndContext>
        </div>
      </section>
    </div>
  );
}
