import { Button } from "@/components/ui/button";
import type { TraySettingsPreview } from "@/hooks/app/use-tray-icon";
import { getTimeFormatter } from "@/lib/reset-tooltip";
import {
  AUTO_UPDATE_OPTIONS,
  DISPLAY_MODE_OPTIONS,
  MENUBAR_ICON_STYLE_OPTIONS,
  MENUBAR_METRIC_OPTIONS,
  RESET_TIMER_DISPLAY_OPTIONS,
  THEME_OPTIONS,
  TIME_FORMAT_OPTIONS,
  type AutoUpdateIntervalMinutes,
  type DisplayMode,
  type MenubarIconStyle,
  type MenubarMetric,
  type ResetTimerDisplayMode,
  type ThemeMode,
  type TimeFormatMode,
} from "@/lib/settings";
import { cn } from "@/lib/utils";
import { MenubarIconStylePreview } from "@/pages/settings/menubar-icon-style-preview";
import { SegmentedRadioGroup } from "@/pages/settings/segmented-radio-group";

type DisplaySettingsSectionsProps = {
  readonly autoUpdateInterval: AutoUpdateIntervalMinutes;
  readonly onAutoUpdateIntervalChange: (value: AutoUpdateIntervalMinutes) => void;
  readonly themeMode: ThemeMode;
  readonly onThemeModeChange: (value: ThemeMode) => void;
  readonly displayMode: DisplayMode;
  readonly onDisplayModeChange: (value: DisplayMode) => void;
  readonly resetTimerDisplayMode: ResetTimerDisplayMode;
  readonly onResetTimerDisplayModeChange: (value: ResetTimerDisplayMode) => void;
  readonly timeFormatMode: TimeFormatMode;
  readonly onTimeFormatModeChange: (value: TimeFormatMode) => void;
  readonly menubarIconStyle: MenubarIconStyle;
  readonly onMenubarIconStyleChange: (value: MenubarIconStyle) => void;
  readonly menubarMetric: MenubarMetric;
  readonly onMenubarMetricChange: (value: MenubarMetric) => void;
  readonly traySettingsPreview: TraySettingsPreview;
};

export function DisplaySettingsSections({
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
  traySettingsPreview,
}: DisplaySettingsSectionsProps) {
  return (
    <>
      <section>
        <h3 className="text-lg font-semibold mb-0">Auto Refresh</h3>
        <p className="text-sm text-muted-foreground mb-2">
          How obsessive are you
        </p>
        <div className="bg-muted/50 rounded-lg p-1">
          <SegmentedRadioGroup label="Auto-update interval">
            {AUTO_UPDATE_OPTIONS.map((option) => {
              const isActive = option.value === autoUpdateInterval;
              return (
                <Button
                  key={option.value}
                  type="button"
                  role="radio"
                  aria-checked={isActive}
                  tabIndex={isActive ? 0 : -1}
                  variant={isActive ? "default" : "outline"}
                  size="sm"
                  className="min-w-0 flex-1 px-1 sm:px-2.5"
                  onClick={() => onAutoUpdateIntervalChange(option.value)}
                >
                  {option.label}
                </Button>
              );
            })}
          </SegmentedRadioGroup>
        </div>
      </section>
      <section>
        <h3 className="text-lg font-semibold mb-0">Usage Mode</h3>
        <p className="text-sm text-muted-foreground mb-2">
          Glass half full or half empty
        </p>
        <div className="bg-muted/50 rounded-lg p-1">
          <SegmentedRadioGroup label="Usage display mode">
            {DISPLAY_MODE_OPTIONS.map((option) => {
              const isActive = option.value === displayMode;
              return (
                <Button
                  key={option.value}
                  type="button"
                  role="radio"
                  aria-checked={isActive}
                  tabIndex={isActive ? 0 : -1}
                  variant={isActive ? "default" : "outline"}
                  size="sm"
                  className="flex-1"
                  onClick={() => onDisplayModeChange(option.value)}
                >
                  {option.label}
                </Button>
              );
            })}
          </SegmentedRadioGroup>
        </div>
      </section>
      <section>
        <h3 className="text-lg font-semibold mb-0">Reset Timers</h3>
        <p className="text-sm text-muted-foreground mb-2">
          Countdown or clock time
        </p>
        <div className="bg-muted/50 rounded-lg p-1">
          <SegmentedRadioGroup label="Reset timer display mode">
            {RESET_TIMER_DISPLAY_OPTIONS.map((option) => {
              const isActive = option.value === resetTimerDisplayMode;
              const absoluteTimeExample = getTimeFormatter(timeFormatMode).format(new Date(2026, 1, 2, 11, 4));
              const example = option.value === "relative" ? "5h 12m" : `today at ${absoluteTimeExample}`;
              return (
                <Button
                  key={option.value}
                  type="button"
                  role="radio"
                  aria-checked={isActive}
                  tabIndex={isActive ? 0 : -1}
                  variant={isActive ? "default" : "outline"}
                  size="sm"
                  className="flex-1 flex flex-col items-center gap-0 py-2 h-auto"
                  onClick={() => onResetTimerDisplayModeChange(option.value)}
                >
                  <span>{option.label}</span>
                  <span
                    className={cn(
                      "text-xs font-normal",
                      isActive ? "text-primary-foreground/80" : "text-muted-foreground"
                    )}
                  >
                    {example}
                  </span>
                </Button>
              );
            })}
          </SegmentedRadioGroup>
        </div>
      </section>
      <section>
        <h3 className="text-lg font-semibold mb-0">Time Format</h3>
        <p className="text-sm text-muted-foreground mb-2">
          12-hour or 24-hour clock
        </p>
        <div className="bg-muted/50 rounded-lg p-1">
          <SegmentedRadioGroup label="Time format">
            {TIME_FORMAT_OPTIONS.map((option) => {
              const isActive = option.value === timeFormatMode;
              const example = getTimeFormatter(option.value).format(new Date(2026, 1, 2, 11, 4));
              return (
                <Button
                  key={option.value}
                  type="button"
                  role="radio"
                  aria-checked={isActive}
                  aria-label={option.label}
                  tabIndex={isActive ? 0 : -1}
                  variant={isActive ? "default" : "outline"}
                  size="sm"
                  className="flex-1 flex flex-col items-center gap-0 py-2 h-auto"
                  onClick={() => onTimeFormatModeChange(option.value)}
                >
                  <span>{option.label}</span>
                  <span
                    className={cn(
                      "text-xs font-normal",
                      isActive ? "text-primary-foreground/80" : "text-muted-foreground"
                    )}
                  >
                    {example}
                  </span>
                </Button>
              );
            })}
          </SegmentedRadioGroup>
        </div>
      </section>
      <section>
        <h3 className="text-lg font-semibold mb-0">Menubar Icon</h3>
        <p className="text-sm text-muted-foreground mb-2">
          What shows in the menu bar
        </p>
        <div className="bg-muted/50 rounded-lg p-1">
          <SegmentedRadioGroup label="Menubar icon style">
            {MENUBAR_ICON_STYLE_OPTIONS.map((option) => {
              const isActive = option.value === menubarIconStyle;
              return (
                <Button
                  key={option.value}
                  type="button"
                  role="radio"
                  aria-label={option.label}
                  aria-checked={isActive}
                  tabIndex={isActive ? 0 : -1}
                  variant={isActive ? "default" : "outline"}
                  size="sm"
                  className="flex-1 h-9 flex items-center justify-center"
                  onClick={() => onMenubarIconStyleChange(option.value)}
                >
                  <MenubarIconStylePreview
                    style={option.value}
                    isActive={isActive}
                    traySettingsPreview={traySettingsPreview}
                  />
                </Button>
              );
            })}
          </SegmentedRadioGroup>
        </div>
        <p className="text-sm text-muted-foreground mt-3 mb-2">Metric</p>
        <div className="bg-muted/50 rounded-lg p-1">
          <SegmentedRadioGroup label="Menubar metric">
            {MENUBAR_METRIC_OPTIONS.map((option) => {
              const isActive = option.value === menubarMetric;
              return (
                <Button
                  key={option.value}
                  type="button"
                  role="radio"
                  aria-label={option.label}
                  aria-checked={isActive}
                  tabIndex={isActive ? 0 : -1}
                  variant={isActive ? "default" : "outline"}
                  size="sm"
                  className="flex-1"
                  onClick={() => onMenubarMetricChange(option.value)}
                >
                  {option.label}
                </Button>
              );
            })}
          </SegmentedRadioGroup>
        </div>
      </section>
      <section>
        <h3 className="text-lg font-semibold mb-0">App Theme</h3>
        <p className="text-sm text-muted-foreground mb-2">
          How it looks around here
        </p>
        <div className="bg-muted/50 rounded-lg p-1">
          <SegmentedRadioGroup label="Theme mode">
            {THEME_OPTIONS.map((option) => {
              const isActive = option.value === themeMode;
              return (
                <Button
                  key={option.value}
                  type="button"
                  role="radio"
                  aria-checked={isActive}
                  tabIndex={isActive ? 0 : -1}
                  variant={isActive ? "default" : "outline"}
                  size="sm"
                  className="flex-1"
                  onClick={() => onThemeModeChange(option.value)}
                >
                  {option.label}
                </Button>
              );
            })}
          </SegmentedRadioGroup>
        </div>
      </section>
    </>
  );
}
