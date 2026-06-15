import { beforeEach, describe, expect, it, vi } from "vitest"
import { render, screen, fireEvent, within, waitFor } from "@testing-library/react"

const invokeMock = vi.fn(async () => {})
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
  isTauri: () => true,
}))

const storeState = new Map<string, unknown>()
vi.mock("@tauri-apps/plugin-store", () => ({
  LazyStore: class {
    async get<T>(key: string): Promise<T | null> {
      if (!storeState.has(key)) return null
      return storeState.get(key) as T | null
    }
    async set<T>(key: string, value: T): Promise<void> { storeState.set(key, value) }
    async delete(key: string): Promise<void> { storeState.delete(key) }
    async save(): Promise<void> {}
  },
}))

import { EnvPage } from "@/pages/env-page"
import { useEnvOverridesStore } from "@/stores/env-overrides-store"

describe("EnvPage", () => {
  beforeEach(() => {
    storeState.clear()
    invokeMock.mockClear()
    useEnvOverridesStore.setState({ groups: [], activeGroupIds: [], loaded: false })
  })

  it("shows an empty state when there are no groups", async () => {
    render(<EnvPage />)
    expect(await screen.findByTestId("env-empty-state")).toBeInTheDocument()
  })

  it("adds a group when New Group is clicked", async () => {
    render(<EnvPage />)
    fireEvent.click(await screen.findByTestId("env-new-group-button"))
    expect(screen.getAllByTestId(/^env-group-/).length).toBeGreaterThan(0)
  })

  it("persists a $ reference row to the backend with kind=reference", async () => {
    render(<EnvPage />)
    fireEvent.click(await screen.findByTestId("env-new-group-button"))
    const group = (await screen.findAllByTestId(/^env-group-/))[0]
    fireEvent.click(within(group).getByTestId("env-group-add-button"))
    fireEvent.change(within(group).getByTestId("env-row-name-0"), { target: { value: "A" } })
    fireEvent.change(within(group).getByTestId("env-row-value-0"), { target: { value: "$B" } })
    await waitFor(() => {
      const lastCall = invokeMock.mock.calls.at(-1)
      expect(lastCall?.[0]).toBe("set_env_overrides")
      expect(lastCall?.[1]).toEqual({
        overrides: [{ name: "A", kind: "reference", value: "B" }],
      })
    })
  })

  it("persists a $$ literal row to the backend as kind=literal value starting with $", async () => {
    render(<EnvPage />)
    fireEvent.click(await screen.findByTestId("env-new-group-button"))
    const group = (await screen.findAllByTestId(/^env-group-/))[0]
    fireEvent.click(within(group).getByTestId("env-group-add-button"))
    fireEvent.change(within(group).getByTestId("env-row-name-0"), { target: { value: "A" } })
    fireEvent.change(within(group).getByTestId("env-row-value-0"), { target: { value: "$$B" } })
    await waitFor(() => {
      const lastCall = invokeMock.mock.calls.at(-1)
      expect(lastCall?.[1]).toEqual({
        overrides: [{ name: "A", kind: "literal", value: "$B" }],
      })
    })
  })
})
