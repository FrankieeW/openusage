import { describe, expect, it, vi, beforeEach } from "vitest"
import { fireEvent, render, screen, waitFor } from "@testing-library/react"

const invokeMock = vi.fn()
const listenMock = vi.fn()

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}))
vi.mock("@tauri-apps/api/event", () => ({
  listen: (...args: unknown[]) => listenMock(...args),
}))

import { HubPage } from "./hub-page"
import {
  __resetHubSubscriptionsForTesting,
  useHubStore,
} from "@/lib/hub/cache"
import type { HubBrowseView, Source } from "@/lib/hub/types"

function sampleSource(id: string): Source {
  return {
    id,
    label: id,
    url: "https://github.com/foo/bar",
    kind: "Github",
    branch: null,
    pluginFilter: null,
    addedAt: 0,
    lastRefreshedAt: null,
    autoCheck: false,
  }
}

function sampleBrowse(sourceId: string): HubBrowseView {
  return {
    source: sampleSource(sourceId),
    available: [
      {
        id: "claude",
        name: "Claude",
        brandColor: "#FF0000",
        iconDataUrl: null,
        sourceId,
        installed: false,
        installedSourceId: null,
        unmanaged: false,
        installedVersion: null,
        availableVersion: "0.6.27",
        packageHash: "sha256:fixture",
        packageStatus: "notInstalled",
        updateAvailable: false,
      },
    ],
    skipped: [],
  }
}

describe("HubPage", () => {
  beforeEach(() => {
    invokeMock.mockReset()
    listenMock.mockReset()
    listenMock.mockResolvedValue(() => {})
    __resetHubSubscriptionsForTesting()
    // Default: list_sources returns whatever is already in the store so the
    // mount-time refreshSources() doesn't wipe pre-seeded state.
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "hub_list_sources") return useHubStore.getState().sources
      if (cmd === "hub_list_local_plugins") return []
      if (cmd === "hub_browse_source") return sampleBrowse("s1")
      return undefined
    })
    useHubStore.setState({
      sources: [],
      localPlugins: [],
      browseBySource: {},
      loading: { sources: false, perSource: {}, perPlugin: {} },
      error: null,
      highlightedSourceId: null,
    })
  })

  it("renders the empty state when no sources", async () => {
    invokeMock.mockResolvedValueOnce([])
    render(<HubPage />)
    expect(await screen.findByTestId("hub-empty-state")).toBeInTheDocument()
  })

  it("loads sources on mount and renders source cards", async () => {
    invokeMock.mockResolvedValueOnce([sampleSource("s1")])
    render(<HubPage />)
    await waitFor(() => {
      expect(screen.getByTestId("hub-source-card")).toBeInTheDocument()
    })
  })

  it("shows an error alert when store.error is set", async () => {
    invokeMock.mockResolvedValueOnce([])
    render(<HubPage />)
    useHubStore.setState({
      error: { code: "CloneFailed", message: "bad" },
    })
    expect(await screen.findByTestId("hub-error-alert")).toBeInTheDocument()
    expect(screen.getByText(/bad/)).toBeInTheDocument()
  })

  it("opens the add-source dialog when Add Source is clicked", async () => {
    invokeMock.mockResolvedValueOnce([])
    render(<HubPage />)
    fireEvent.click(await screen.findByTestId("hub-add-source-button"))
    expect(await screen.findByTestId("hub-add-source-dialog")).toBeInTheDocument()
  })

  it("expanding a source card triggers a browse", async () => {
    invokeMock.mockResolvedValueOnce([sampleSource("s1")])
    render(<HubPage />)
    await screen.findByTestId("hub-source-card")
    fireEvent.click(screen.getByTestId("hub-source-toggle"))
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("hub_browse_source", {
        sourceId: "s1",
      })
    })
  })

  it("clicking toggle on cached source does not re-browse", async () => {
    invokeMock.mockResolvedValueOnce([sampleSource("s1")])
    render(<HubPage />)
    const card = await screen.findByTestId("hub-source-card")
    const toggle = card.querySelector(
      "[data-testid='hub-source-toggle']",
    ) as HTMLElement
    // First expand — triggers browse
    fireEvent.click(toggle)
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("hub_browse_source", {
        sourceId: "s1",
      })
    })
    const browseCallCount = invokeMock.mock.calls.filter(
      (c) => c[0] === "hub_browse_source",
    ).length
    // Collapse + expand again — should NOT re-browse
    fireEvent.click(toggle)
    fireEvent.click(toggle)
    await new Promise((r) => setTimeout(r, 20))
    const browseCallCountAfter = invokeMock.mock.calls.filter(
      (c) => c[0] === "hub_browse_source",
    ).length
    expect(browseCallCountAfter).toBe(browseCallCount)
  })

  it("install button triggers hub_install", async () => {
    useHubStore.setState({
      sources: [sampleSource("s1")],
      browseBySource: { s1: sampleBrowse("s1") },
    })
    render(<HubPage />)
    const card = await screen.findByTestId("hub-source-card")
    const toggle = card.querySelector(
      "[data-testid='hub-source-toggle']",
    ) as HTMLElement
    fireEvent.click(toggle)
    const installBtn = await screen.findByTestId("hub-install-button")
    fireEvent.click(installBtn)
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("hub_install", {
        sourceId: "s1",
        pluginId: "claude",
      })
    })
  })

  it("delete source requires confirmation before invoking removeSource", async () => {
    useHubStore.setState({
      sources: [sampleSource("s1")],
      browseBySource: {},
    })
    render(<HubPage />)
    const deleteBtn = await screen.findByTestId("hub-source-delete")
    fireEvent.click(deleteBtn)
    const confirm = await screen.findByTestId("hub-confirm-delete")
    expect(confirm).toBeInTheDocument()
    fireEvent.click(screen.getByTestId("hub-confirm-delete-no"))
    expect(invokeMock).not.toHaveBeenCalledWith(
      "hub_remove_source",
      expect.anything(),
    )
    fireEvent.click(deleteBtn)
    fireEvent.click(screen.getByTestId("hub-confirm-delete-yes"))
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("hub_remove_source", {
        sourceId: "s1",
      })
    })
  })

  it("subscribes to plugins-changed and hub-orphans-detected on mount", async () => {
    render(<HubPage />)
    await waitFor(() => {
      expect(listenMock).toHaveBeenCalledWith(
        "plugins-changed",
        expect.any(Function),
      )
      expect(listenMock).toHaveBeenCalledWith(
        "hub-orphans-detected",
        expect.any(Function),
      )
    })
  })
})
