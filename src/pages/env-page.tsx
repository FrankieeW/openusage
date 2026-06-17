import { useEffect, useState } from "react"
import { Button } from "@/components/ui/button"
import { useEnvOverridesStore } from "@/stores/env-overrides-store"

const inputClass =
  "h-8 rounded-md border bg-background px-2 text-sm outline-none focus:ring-1 focus:ring-ring"

export function EnvPage() {
  const groups = useEnvOverridesStore((s) => s.groups)
  const init = useEnvOverridesStore((s) => s.init)
  const addGroup = useEnvOverridesStore((s) => s.addGroup)
  const updateGroup = useEnvOverridesStore((s) => s.updateGroup)
  const removeGroup = useEnvOverridesStore((s) => s.removeGroup)
  const addOverride = useEnvOverridesStore((s) => s.addOverride)
  const updateOverride = useEnvOverridesStore((s) => s.updateOverride)
  const removeOverride = useEnvOverridesStore((s) => s.removeOverride)
  const saveAndReload = useEnvOverridesStore((s) => s.saveAndReload)

  const [saving, setSaving] = useState(false)
  const [saved, setSaved] = useState(false)

  useEffect(() => {
    void init()
  }, [init])

  const handleSave = async () => {
    setSaving(true)
    setSaved(false)
    try {
      await saveAndReload()
      setSaved(true)
      setTimeout(() => setSaved(false), 3000)
    } finally {
      setSaving(false)
    }
  }

  const handleAddGroup = () => {
    addGroup()
  }

  const toggleGroupActive = (groupId: string, active: boolean) => {
    updateGroup(groupId, { enabled: active })
  }

  return (
    <div className="py-3 px-1 space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-lg font-semibold mb-0">Environment Variables</h1>
          <p className="text-sm text-muted-foreground mb-1">
            Map a variable to a literal or to another variable from your shell.{" "}
            <a
              href="https://github.com/FrankieeW/openusage/blob/main/docs/env-overrides.md"
              target="_blank"
              rel="noreferrer"
              className="underline"
            >
              Learn more
            </a>
            .
          </p>
        </div>
        <div className="flex flex-col gap-2">
          <Button
            size="sm"
            variant={saved ? "secondary" : "default"}
            onClick={handleSave}
            disabled={saving}
            data-testid="env-reload-button"
          >
            {saving ? "Reloading…" : saved ? "Reloaded" : "Reload"}
          </Button>
          <Button size="sm" onClick={handleAddGroup} data-testid="env-new-group-button">
            New Group
          </Button>
        </div>
      </div>

      {groups.length === 0 && (
        <div
          data-testid="env-empty-state"
          className="bg-muted/50 rounded-lg border border-dashed/30 px-6 py-12 text-center"
        >
          <p className="text-sm text-muted-foreground">
            No groups yet. Add one to start defining environment variables.
          </p>
        </div>
      )}

      <div className="space-y-3">
        {groups.map((group) => (
          <section
            key={group.id}
            data-testid={`env-group-${group.id}`}
            className="rounded-md border bg-card p-3 space-y-2"
          >
            <header className="space-y-2">
              <input
                data-testid={`env-group-name-${group.id}`}
                aria-label="Group name"
                className={`${inputClass} w-full`}
                value={group.name}
                onChange={(e) => updateGroup(group.id, { name: e.target.value })}
              />
              <div className="flex items-center justify-between gap-2">
                <label className="flex items-center gap-1 text-sm">
                  <input
                    type="checkbox"
                    data-testid={`env-group-enabled-${group.id}`}
                    checked={group.enabled}
                    onChange={(e) => toggleGroupActive(group.id, e.target.checked)}
                  />
                  Active
                </label>
                <Button
                  size="xs"
                  variant="ghost"
                  aria-label="Delete group"
                  data-testid={`env-group-delete-${group.id}`}
                  onClick={() => removeGroup(group.id)}
                >
                  Delete
                </Button>
              </div>
            </header>

            <div className="space-y-2">
              {group.overrides.map((entry, index) => (
                <div key={index} className="relative pr-16">
                  <input
                    data-testid={`env-row-name-${index}`}
                    aria-label="Variable name"
                    className={`${inputClass} w-full`}
                    placeholder="NAME"
                    value={entry.name}
                    onChange={(e) => updateOverride(group.id, index, { name: e.target.value })}
                  />
                  <input
                    data-testid={`env-row-value-${index}`}
                    aria-label="Variable value"
                    className={`${inputClass} w-full mt-1`}
                    placeholder="value or $OTHER"
                    value={entry.value}
                    onChange={(e) => updateOverride(group.id, index, { value: e.target.value })}
                  />
                  <div className="absolute right-0 top-0">
                    <Button
                      size="xs"
                      variant="ghost"
                      aria-label="Remove variable"
                      data-testid={`env-row-delete-${index}`}
                      onClick={() => removeOverride(group.id, index)}
                    >
                      Remove
                    </Button>
                  </div>
                </div>
              ))}
            </div>

            <Button
              size="xs"
              variant="ghost"
              data-testid="env-group-add-button"
              onClick={() => addOverride(group.id)}
            >
              Add Variable
            </Button>
          </section>
        ))}
      </div>
    </div>
  )
}
