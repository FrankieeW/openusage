import { useEffect } from "react"
import { Plus, Trash2 } from "lucide-react"
import { Button } from "@/components/ui/button"
import { useEnvOverridesStore } from "@/stores/env-overrides-store"

const inputClass =
  "h-8 rounded-md border bg-background px-2 text-sm outline-none focus:ring-1 focus:ring-ring"

export function EnvPage() {
  const groups = useEnvOverridesStore((s) => s.groups)
  const activeGroupIds = useEnvOverridesStore((s) => s.activeGroupIds)
  const init = useEnvOverridesStore((s) => s.init)
  const addGroup = useEnvOverridesStore((s) => s.addGroup)
  const updateGroup = useEnvOverridesStore((s) => s.updateGroup)
  const removeGroup = useEnvOverridesStore((s) => s.removeGroup)
  const addOverride = useEnvOverridesStore((s) => s.addOverride)
  const updateOverride = useEnvOverridesStore((s) => s.updateOverride)
  const removeOverride = useEnvOverridesStore((s) => s.removeOverride)
  const setActiveGroupIds = useEnvOverridesStore((s) => s.setActiveGroupIds)

  useEffect(() => {
    void init()
  }, [init])

  // The store's addGroup creates the group with enabled=true but does not add
  // it to activeGroupIds. Since the UI's "Active" checkbox is the source of
  // truth for both, new groups must be active from the start so variables in
  // them reach the backend immediately.
  const handleAddGroup = () => {
    addGroup()
    const state = useEnvOverridesStore.getState()
    const newest = state.groups[state.groups.length - 1]
    if (newest && !state.activeGroupIds.includes(newest.id)) {
      setActiveGroupIds([...state.activeGroupIds, newest.id])
    }
  }

  const toggleGroupActive = (groupId: string, active: boolean) => {
    updateGroup(groupId, { enabled: active })
    if (active) {
      setActiveGroupIds(Array.from(new Set([...activeGroupIds, groupId])))
    } else {
      setActiveGroupIds(activeGroupIds.filter((id) => id !== groupId))
    }
  }

  return (
    <div className="py-3 px-1 space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-lg font-semibold mb-0">Environment Variables</h1>
          <p className="text-sm text-muted-foreground mb-1">
            Group variables into cards and enable the ones you want active.
            A value starting with "$" maps to another variable from your shell; a
            value starting with "$$" is a literal that begins with "$".
          </p>
        </div>
        <Button size="sm" onClick={handleAddGroup} data-testid="env-new-group-button">
          <Plus size={14} />
          New Group
        </Button>
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
        {groups.map((group) => {
          const isActive = activeGroupIds.includes(group.id)
          return (
            <section
              key={group.id}
              data-testid={`env-group-${group.id}`}
              className="rounded-md border bg-card p-3 space-y-2"
            >
              <header className="flex items-center gap-2">
                <input
                  data-testid={`env-group-name-${group.id}`}
                  aria-label="Group name"
                  className={`${inputClass} flex-1`}
                  value={group.name}
                  onChange={(e) => updateGroup(group.id, { name: e.target.value })}
                />
                <label className="flex items-center gap-1 text-sm">
                  <input
                    type="checkbox"
                    data-testid={`env-group-enabled-${group.id}`}
                    checked={isActive}
                    onChange={(e) => toggleGroupActive(group.id, e.target.checked)}
                  />
                  Active
                </label>
                <Button
                  size="icon"
                  variant="ghost"
                  aria-label="Delete group"
                  data-testid={`env-group-delete-${group.id}`}
                  onClick={() => removeGroup(group.id)}
                >
                  <Trash2 size={14} />
                </Button>
              </header>

              <div className="space-y-2">
                {group.overrides.map((entry, index) => (
                  <div key={index} className="flex items-center gap-2">
                    <input
                      data-testid={`env-row-name-${index}`}
                      aria-label="Variable name"
                      className={`${inputClass} w-40`}
                      placeholder="NAME"
                      value={entry.name}
                      onChange={(e) => updateOverride(group.id, index, { name: e.target.value })}
                    />
                    <input
                      data-testid={`env-row-value-${index}`}
                      aria-label="Variable value"
                      className={`${inputClass} flex-1`}
                      placeholder="value or $OTHER"
                      value={entry.value}
                      onChange={(e) => updateOverride(group.id, index, { value: e.target.value })}
                    />
                    <Button
                      size="icon"
                      variant="ghost"
                      aria-label="Remove variable"
                      data-testid={`env-row-delete-${index}`}
                      onClick={() => removeOverride(group.id, index)}
                    >
                      <Trash2 size={14} />
                    </Button>
                  </div>
                ))}
              </div>

              <Button
                size="xs"
                variant="ghost"
                data-testid="env-group-add-button"
                onClick={() => addOverride(group.id)}
              >
                <Plus size={12} />
                Add Variable
              </Button>
            </section>
          )
        })}
      </div>
    </div>
  )
}
