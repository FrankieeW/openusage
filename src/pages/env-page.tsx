import { useEffect } from "react"
import { Plus, Trash2 } from "lucide-react"
import { Button } from "@/components/ui/button"
import { useEnvOverridesStore } from "@/stores/env-overrides-store"
import type { EnvOverrideKind } from "@/lib/env-overrides"

const inputClass =
  "h-8 rounded-md border bg-background px-2 text-sm outline-none focus:ring-1 focus:ring-ring"

export function EnvPage() {
  const overrides = useEnvOverridesStore((s) => s.overrides)
  const init = useEnvOverridesStore((s) => s.init)
  const addOverride = useEnvOverridesStore((s) => s.addOverride)
  const updateOverride = useEnvOverridesStore((s) => s.updateOverride)
  const removeOverride = useEnvOverridesStore((s) => s.removeOverride)

  useEffect(() => {
    void init()
  }, [init])

  return (
    <div className="py-3 px-1 space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-lg font-semibold mb-0">Environment Variables</h1>
          <p className="text-sm text-muted-foreground mb-1">
            Map a name to a literal value, or to another variable from your shell
            environment. These take precedence over the real environment.
          </p>
        </div>
        <Button size="sm" onClick={addOverride} data-testid="env-add-button">
          <Plus size={14} />
          Add Variable
        </Button>
      </div>

      {overrides.length === 0 && (
        <div
          data-testid="env-empty-state"
          className="bg-muted/50 rounded-lg border border-dashed/30 px-6 py-12 text-center"
        >
          <p className="text-sm text-muted-foreground">
            No variables yet. Add one to map a name to a value or another variable.
          </p>
        </div>
      )}

      <div className="space-y-2">
        {overrides.map((entry, index) => (
          <div key={index} className="flex items-center gap-2">
            <input
              data-testid={`env-name-input-${index}`}
              aria-label="Variable name"
              className={`${inputClass} w-40`}
              placeholder="NAME"
              value={entry.name}
              onChange={(e) => updateOverride(index, { name: e.target.value })}
            />
            <select
              data-testid={`env-kind-select-${index}`}
              aria-label="Variable kind"
              className={`${inputClass} w-32`}
              value={entry.kind}
              onChange={(e) =>
                updateOverride(index, { kind: e.target.value as EnvOverrideKind })
              }
            >
              <option value="literal">Value</option>
              <option value="reference">Reference</option>
            </select>
            <div className="flex flex-1 items-center gap-1">
              {entry.kind === "reference" && (
                <span className="text-sm text-muted-foreground">$</span>
              )}
              <input
                data-testid={`env-value-input-${index}`}
                aria-label="Variable value"
                className={`${inputClass} flex-1`}
                placeholder={entry.kind === "reference" ? "SOURCE_NAME" : "value"}
                value={entry.value}
                onChange={(e) => updateOverride(index, { value: e.target.value })}
              />
            </div>
            <Button
              size="icon"
              variant="ghost"
              aria-label="Remove variable"
              data-testid={`env-remove-button-${index}`}
              onClick={() => removeOverride(index)}
            >
              <Trash2 size={14} />
            </Button>
          </div>
        ))}
      </div>
    </div>
  )
}
