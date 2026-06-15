import { fireEvent, render, screen } from "@testing-library/react"
import { beforeEach, describe, expect, it, vi } from "vitest"

const { overviewPageMock, providerDetailPageMock, settingsPageMock, envPageMock } = vi.hoisted(() => ({
  overviewPageMock: vi.fn(),
  settingsPageMock: vi.fn(),
  providerDetailPageMock: vi.fn(),
  envPageMock: vi.fn(),
}))

vi.mock("@/pages/overview", () => ({
  OverviewPage: (props: unknown) => {
    overviewPageMock(props)
    return <div data-testid="overview-page" />
  },
}))

vi.mock("@/pages/settings", () => ({
  SettingsPage: (props: unknown) => {
    settingsPageMock(props)
    return <div data-testid="settings-page" />
  },
}))

vi.mock("@/pages/provider-detail", () => ({
  ProviderDetailPage: (props: { onRetry?: () => void }) => {
    providerDetailPageMock(props)
    return (
      <div data-testid="provider-detail-page">
        {props.onRetry ? <button onClick={props.onRetry}>retry-provider</button> : null}
      </div>
    )
  },
}))

vi.mock("@/pages/env-page", () => ({
  EnvPage: () => {
    envPageMock()
    return <div data-testid="env-page" />
  },
}))

import { AppContent, type AppContentProps } from "@/components/app/app-content"
import { useAppPreferencesStore } from "@/stores/app-preferences-store"
import { useAppUiStore } from "@/stores/app-ui-store"

function createProps(): AppContentProps {
  return {
    displayPlugins: [],
    settingsPlugins: [],
    selectedPlugin: {
      meta: {
        id: "codex",
        name: "Codex",
        iconUrl: "/codex.svg",
        brandColor: "#000000",
        lines: [],
        primaryCandidates: [],
      },
      data: null,
      loading: false,
      error: null,
      lastManualRefreshAt: null,
      lastUpdatedAt: null,
    },
    onRetryPlugin: vi.fn(),
    onReorder: vi.fn(),
    onToggle: vi.fn(),
    onAutoUpdateIntervalChange: vi.fn(),
    onThemeModeChange: vi.fn(),
    onDisplayModeChange: vi.fn(),
    onResetTimerDisplayModeChange: vi.fn(),
    onResetTimerDisplayModeToggle: vi.fn(),
    onGlobalShortcutChange: vi.fn(),
    onStartOnLoginChange: vi.fn(),
  }
}

describe("AppContent", () => {
  beforeEach(() => {
    overviewPageMock.mockReset()
    settingsPageMock.mockReset()
    providerDetailPageMock.mockReset()
    envPageMock.mockReset()
    useAppUiStore.getState().resetState()
    useAppPreferencesStore.getState().resetState()
  })

  it("renders overview page for home view", () => {
    useAppUiStore.getState().setActiveView("home")
    render(<AppContent {...createProps()} />)

    expect(screen.getByTestId("overview-page")).toBeInTheDocument()
    expect(overviewPageMock).toHaveBeenCalledTimes(1)
  })

  it("renders settings page for settings view", () => {
    useAppUiStore.getState().setActiveView("settings")
    render(<AppContent {...createProps()} />)

    expect(screen.getByTestId("settings-page")).toBeInTheDocument()
    expect(settingsPageMock).toHaveBeenCalledTimes(1)
  })

  it("renders env page for env view", () => {
    useAppUiStore.getState().setActiveView("env")
    render(<AppContent {...createProps()} />)

    expect(screen.getByTestId("env-page")).toBeInTheDocument()
    expect(envPageMock).toHaveBeenCalledTimes(1)
  })

  it("passes retry callback for provider detail view", () => {
    const props = createProps()
    useAppUiStore.getState().setActiveView("codex")
    render(<AppContent {...props} />)

    fireEvent.click(screen.getByRole("button", { name: "retry-provider" }))

    expect(providerDetailPageMock).toHaveBeenCalledTimes(1)
    expect(props.onRetryPlugin).toHaveBeenCalledWith("codex")
  })
})
