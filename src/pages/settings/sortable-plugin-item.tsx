import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { GripVertical } from "lucide-react";
import { Checkbox } from "@/components/ui/checkbox";
import { cn } from "@/lib/utils";

export type PluginConfig = {
  readonly id: string;
  readonly name: string;
  readonly enabled: boolean;
  readonly sourceLabel: string | null;
  readonly version: string | null;
};

function sourceVersionLabel(sourceLabel: string | null, version: string | null): string | null {
  if (sourceLabel && version) return `${sourceLabel} · v${version}`;
  if (sourceLabel) return sourceLabel;
  if (version) return `v${version}`;
  return null;
}

export function SortablePluginItem({
  plugin,
  onToggle,
}: {
  readonly plugin: PluginConfig;
  readonly onToggle: (id: string) => void;
}) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging,
  } = useSortable({ id: plugin.id });

  const style = {
    transform: CSS.Transform.toString(transform),
    transition,
  };

  return (
    <div
      ref={setNodeRef}
      style={style}
      onClick={() => onToggle(plugin.id)}
      className={cn(
        "flex items-center gap-3 px-3 py-2 rounded-md bg-card cursor-pointer",
        "border border-transparent",
        isDragging && "opacity-50 border-border"
      )}
    >
      <button
        type="button"
        onClick={(e) => e.stopPropagation()}
        className="touch-none cursor-grab active:cursor-grabbing text-muted-foreground hover:text-foreground transition-colors"
        {...attributes}
        {...listeners}
      >
        <GripVertical className="h-4 w-4" />
      </button>

      <span className="flex-1 text-sm flex flex-col">
        <span className={cn(!plugin.enabled && "text-muted-foreground")}>{plugin.name}</span>
        {sourceVersionLabel(plugin.sourceLabel, plugin.version) && (
          <span className="text-xs text-muted-foreground">
            {sourceVersionLabel(plugin.sourceLabel, plugin.version)}
          </span>
        )}
      </span>

      {/* Wrap to stop Base UI's internal input.click() from bubbling to the row div */}
      <span onClick={(e) => e.stopPropagation()}>
        <Checkbox
          key={`${plugin.id}-${plugin.enabled}`}
          checked={plugin.enabled}
          onCheckedChange={() => onToggle(plugin.id)}
        />
      </span>
    </div>
  );
}
