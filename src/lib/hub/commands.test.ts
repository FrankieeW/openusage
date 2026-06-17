import { describe, expect, it, vi } from "vitest"

const invokeMock = vi.fn()

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}))

import { hubCommands } from "./commands"
import type { HubError } from "./types"

describe("hubCommands", () => {
  it("listSources invokes hub_list_sources with no args", async () => {
    invokeMock.mockResolvedValueOnce([])
    await hubCommands.listSources()
    expect(invokeMock).toHaveBeenCalledWith("hub_list_sources", undefined)
  })

  it("addSource invokes hub_add_source with url and label", async () => {
    invokeMock.mockResolvedValueOnce({
      id: "src-1",
      label: "Foo",
      url: "https://github.com/foo/bar",
      kind: "Github",
      addedAt: 0,
      lastRefreshedAt: null,
      autoCheck: false,
    })
    await hubCommands.addSource("https://github.com/foo/bar", "Foo")
    expect(invokeMock).toHaveBeenCalledWith("hub_add_source", {
      url: "https://github.com/foo/bar",
      label: "Foo",
      branch: null,
      pluginFilter: null,
    })
  })

  it("addSource passes null label when omitted", async () => {
    invokeMock.mockResolvedValueOnce({} as never)
    await hubCommands.addSource("https://github.com/foo/bar")
    expect(invokeMock).toHaveBeenCalledWith("hub_add_source", {
      url: "https://github.com/foo/bar",
      label: null,
      branch: null,
      pluginFilter: null,
    })
  })

  it("install throws a typed HubError on Conflict rejection", async () => {
    const err: HubError = {
      code: "Conflict",
      message: "already installed from src-other",
      context: { otherSourceId: "src-other" },
    }
    invokeMock.mockRejectedValueOnce(err)
    await expect(hubCommands.install("src-x", "foo")).rejects.toMatchObject(err)
  })

  it("switchSource invokes hub_switch_source with sourceId and pluginId", async () => {
    invokeMock.mockResolvedValueOnce(undefined)
    await hubCommands.switchSource("src-new", "foo")
    expect(invokeMock).toHaveBeenCalledWith("hub_switch_source", {
      sourceId: "src-new",
      pluginId: "foo",
    })
  })

  it("uninstall calls hub_uninstall with pluginId and sourceId", async () => {
    invokeMock.mockResolvedValueOnce(undefined)
    await hubCommands.uninstall("foo", "src-1")
    expect(invokeMock).toHaveBeenCalledWith("hub_uninstall", { pluginId: "foo", sourceId: "src-1" })
  })

  it("uninstall calls hub_uninstall with pluginId and null sourceId", async () => {
    invokeMock.mockResolvedValueOnce(undefined)
    await hubCommands.uninstall("foo")
    expect(invokeMock).toHaveBeenCalledWith("hub_uninstall", { pluginId: "foo", sourceId: null })
  })

  it("browseSource passes sourceId", async () => {
    invokeMock.mockResolvedValueOnce({} as never)
    await hubCommands.browseSource("src-1")
    expect(invokeMock).toHaveBeenCalledWith("hub_browse_source", {
      sourceId: "src-1",
    })
  })

  it("wraps non-HubError rejections into IoError", async () => {
    invokeMock.mockRejectedValueOnce("plain string")
    await expect(hubCommands.listSources()).rejects.toMatchObject({
      code: "IoError",
      message: "plain string",
    })
  })
})
