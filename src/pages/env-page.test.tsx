import { beforeEach, describe, expect, it, vi } from "vitest"
import { render, screen, fireEvent, waitFor } from "@testing-library/react"

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
    async set<T>(key: string, value: T): Promise<void> {
      storeState.set(key, value)
    }
    async save(): Promise<void> {}
  },
}))

import { EnvPage } from "@/pages/env-page"
import { useEnvOverridesStore } from "@/stores/env-overrides-store"

describe("EnvPage", () => {
  beforeEach(() => {
    storeState.clear()
    invokeMock.mockClear()
    useEnvOverridesStore.setState({ overrides: [], loaded: false })
  })

  it("shows an empty state when there are no overrides", async () => {
    render(<EnvPage />)
    expect(await screen.findByTestId("env-empty-state")).toBeInTheDocument()
  })

  it("adds a row when Add Variable is clicked", async () => {
    render(<EnvPage />)
    fireEvent.click(await screen.findByTestId("env-add-button"))
    expect(screen.getByTestId("env-name-input-0")).toBeInTheDocument()
  })

  it("persists a completed literal override to the backend", async () => {
    render(<EnvPage />)
    fireEvent.click(await screen.findByTestId("env-add-button"))
    fireEvent.change(screen.getByTestId("env-name-input-0"), {
      target: { value: "A" },
    })
    fireEvent.change(screen.getByTestId("env-value-input-0"), {
      target: { value: "api" },
    })

    await waitFor(() => {
      const lastCall = invokeMock.mock.calls.at(-1)
      expect(lastCall?.[0]).toBe("set_env_overrides")
      expect(lastCall?.[1]).toEqual({
        overrides: [{ name: "A", kind: "literal", value: "api" }],
      })
    })
  })

  it("removes a row when delete is clicked", async () => {
    render(<EnvPage />)
    fireEvent.click(await screen.findByTestId("env-add-button"))
    expect(screen.getByTestId("env-name-input-0")).toBeInTheDocument()
    fireEvent.click(screen.getByTestId("env-remove-button-0"))
    expect(screen.queryByTestId("env-name-input-0")).not.toBeInTheDocument()
  })
})
