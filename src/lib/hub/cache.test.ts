import { describe, expect, it, vi, beforeEach } from "vitest"

const invokeMock = vi.fn()

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}))

import { useHubStore } from "./cache"
import type { HubBrowseView, Source } from "./types"

function sampleSource(id: string): Source {
  return {
    id,
    label: id,
    url: "https://github.com/foo/bar",
    kind: "Github",
    branch: null,
    addedAt: 0,
    lastRefreshedAt: null,
    autoCheck: false,
  }
}

function sampleBrowse(sourceId: string, pluginId: string): HubBrowseView {
  return {
    source: sampleSource(sourceId),
    available: [
      {
        id: pluginId,
        name: pluginId,
        brandColor: null,
        iconDataUrl: null,
        sourceId,
        installed: false,
        installedSourceId: null,
        unmanaged: false,
        installedVersion: null,
        availableVersion: "0.6.27",
        updateAvailable: false,
      },
    ],
    skipped: [],
  }
}

describe("useHubStore", () => {
  beforeEach(() => {
    invokeMock.mockReset()
    useHubStore.setState({
      sources: [],
      browseBySource: {},
      loading: { sources: false, perSource: {}, perPlugin: {} },
      error: null,
    })
  })

  it("refreshSources populates sources and clears loading", async () => {
    invokeMock.mockResolvedValueOnce([sampleSource("s1"), sampleSource("s2")])
    await useHubStore.getState().refreshSources()
    const s = useHubStore.getState()
    expect(s.sources.map((x) => x.id)).toEqual(["s1", "s2"])
    expect(s.loading.sources).toBe(false)
  })

  it("refreshSources captures errors into store.error", async () => {
    invokeMock.mockRejectedValueOnce({
      code: "IoError",
      message: "disk gone",
    })
    await useHubStore.getState().refreshSources()
    expect(useHubStore.getState().error).toEqual({
      code: "IoError",
      message: "disk gone",
    })
  })

  it("browseSource caches by sourceId and skips re-fetch when cached", async () => {
    invokeMock.mockResolvedValueOnce(sampleBrowse("s1", "p1"))
    const view = await useHubStore.getState().browseSource("s1")
    expect(view?.available[0].id).toBe("p1")
    expect(useHubStore.getState().browseBySource["s1"]).toBe(view)
    expect(invokeMock).toHaveBeenCalledTimes(1)

    // second call without force should NOT re-invoke
    const view2 = await useHubStore.getState().browseSource("s1")
    expect(view2).toBe(view)
    expect(invokeMock).toHaveBeenCalledTimes(1)
  })

  it("browseSource with force=true re-invokes", async () => {
    invokeMock.mockResolvedValue(sampleBrowse("s1", "p1"))
    await useHubStore.getState().browseSource("s1")
    await useHubStore.getState().browseSource("s1", true)
    expect(invokeMock).toHaveBeenCalledTimes(2)
  })

  it("install flips perPlugin loading flag on/off", async () => {
    invokeMock
      .mockResolvedValueOnce(undefined) // install
      .mockResolvedValueOnce(sampleBrowse("s1", "p1")) // browse refresh
    const promise = useHubStore.getState().install("s1", "p1")
    // Synchronously after dispatch, loading flag should be set
    expect(useHubStore.getState().loading.perPlugin["s1:p1"]).toBe("install")
    await promise
    expect(useHubStore.getState().loading.perPlugin["s1:p1"]).toBeNull()
  })

  it("install captures Conflict into store.error without throwing", async () => {
    invokeMock.mockRejectedValueOnce({
      code: "Conflict",
      message: "already installed",
      context: { otherSourceId: "src-other" },
    })
    await useHubStore.getState().install("s1", "p1")
    expect(useHubStore.getState().error?.code).toBe("Conflict")
  })

  it("uninstall clears installed flags for the entry from the matching source", async () => {
    useHubStore.setState({
      browseBySource: { s1: sampleBrowse("s1", "p1") },
    })
    // mark p1 installed from s1
    useHubStore.setState((s) => ({
      browseBySource: {
        s1: {
          ...s.browseBySource.s1,
          available: s.browseBySource.s1.available.map((p) => ({
            ...p,
            installed: true,
            installedSourceId: "s1",
            installedVersion: "0.6.27",
          })),
        },
      },
    }))
    invokeMock.mockResolvedValueOnce(undefined)
    await useHubStore.getState().uninstall("p1", "s1")
    const p1 = useHubStore.getState().browseBySource.s1.available[0]
    expect(p1.installed).toBe(false)
    expect(p1.installedSourceId).toBeNull()
  })

  it("uninstall from one source does not clear the same plugin id from a different source", async () => {
    // p1 is installed from s2; the user uninstalls it from s2. The s1 entry
    // (which shows p1 as "managed by s2" — not installed locally) must stay unchanged.
    useHubStore.setState({
      browseBySource: {
        s1: sampleBrowse("s1", "p1"),
        s2: sampleBrowse("s2", "p1"),
      },
    })
    useHubStore.setState((s) => ({
      browseBySource: {
        s1: {
          ...s.browseBySource.s1,
          available: s.browseBySource.s1.available.map((p) => ({
            ...p,
            installed: false,
            installedSourceId: "s2",
            installedVersion: "0.6.27",
          })),
        },
        s2: {
          ...s.browseBySource.s2,
          available: s.browseBySource.s2.available.map((p) => ({
            ...p,
            installed: true,
            installedSourceId: "s2",
            installedVersion: "0.6.27",
          })),
        },
      },
    }))
    invokeMock.mockResolvedValueOnce(undefined)
    await useHubStore.getState().uninstall("p1", "s2")
    const state = useHubStore.getState()
    // s2 entry: cleared
    expect(state.browseBySource.s2.available[0].installed).toBe(false)
    expect(state.browseBySource.s2.available[0].installedSourceId).toBeNull()
    // s1 entry: untouched (still shows "managed by s2")
    expect(state.browseBySource.s1.available[0].installed).toBe(false)
    expect(state.browseBySource.s1.available[0].installedSourceId).toBe("s2")
  })

  it("removeSource drops source and its browse cache", async () => {
    useHubStore.setState({
      sources: [sampleSource("s1")],
      browseBySource: { s1: sampleBrowse("s1", "p1") },
    })
    invokeMock.mockResolvedValueOnce(undefined)
    await useHubStore.getState().removeSource("s1")
    expect(useHubStore.getState().sources).toHaveLength(0)
    expect(useHubStore.getState().browseBySource["s1"]).toBeUndefined()
  })

  it("addSource appends to sources on success", async () => {
    invokeMock.mockResolvedValueOnce(sampleSource("new"))
    const result = await useHubStore.getState().addSource("https://github.com/x/y")
    expect(result?.id).toBe("new")
    expect(useHubStore.getState().sources.map((s) => s.id)).toContain("new")
  })

  it("addSource returns null and captures error on failure", async () => {
    invokeMock.mockRejectedValueOnce({
      code: "InvalidUrl",
      message: "no",
    })
    const result = await useHubStore.getState().addSource("not-a-url")
    expect(result).toBeNull()
    expect(useHubStore.getState().error?.code).toBe("InvalidUrl")
  })

  it("clearError resets error to null", async () => {
    useHubStore.setState({ error: { code: "IoError", message: "x" } })
    useHubStore.getState().clearError()
    expect(useHubStore.getState().error).toBeNull()
  })
})