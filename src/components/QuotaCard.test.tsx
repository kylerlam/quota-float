// @vitest-environment jsdom

import { act, cleanup, fireEvent, render } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ProviderSnapshot, WidgetPreferences } from "../types";
import { QuotaCard, QuotaOrb } from "./QuotaCard";

const snapshot: ProviderSnapshot = {
  provider: "codex",
  displayName: "CODEX",
  plan: "PRO",
  shortWindow: { remainingPercent: 74, resetsAt: null, windowSeconds: 18_000 },
  weeklyWindow: { remainingPercent: 42, resetsAt: null, windowSeconds: 604_800 },
  resetCredits: 1,
  updatedAt: "2026-07-28T00:00:00Z",
  status: "ok",
  message: null,
};

const lockedPreferences: WidgetPreferences = {
  locked: true,
  alwaysOnTop: true,
  windowBehaviorVersion: 1,
  stayExpanded: false,
  pinnedProvider: null,
  autoRotateSeconds: 12,
  language: "zh-CN",
  appearance: "light",
  selectedSkin: "default",
};

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("locked widget interactions", () => {
  it("uses the idle appearance and ignores compact interactions while locked", () => {
    vi.useFakeTimers();
    const onHover = vi.fn();
    const onDrag = vi.fn();
    const { container } = render(<QuotaOrb snapshot={snapshot} onHover={onHover} onDrag={onDrag} locked />);
    const orb = container.querySelector(".quota-orb");
    expect(orb).not.toBeNull();

    fireEvent.mouseEnter(orb!);
    fireEvent.mouseLeave(orb!);
    fireEvent.mouseDown(orb!, { button: 0 });
    act(() => vi.advanceTimersByTime(2_500));

    expect(onHover).not.toHaveBeenCalled();
    expect(onDrag).not.toHaveBeenCalled();
    expect(orb?.classList.contains("quota-orb--idle")).toBe(true);
  });

  it("switches to the compact idle appearance when locking", () => {
    vi.useFakeTimers();
    const props = { snapshot, onHover: vi.fn(), onDrag: vi.fn() };
    const { container, rerender } = render(<QuotaOrb {...props} />);
    const orb = container.querySelector(".quota-orb");

    expect(orb?.classList.contains("quota-orb--idle")).toBe(false);

    rerender(<QuotaOrb {...props} locked />);
    expect(orb?.classList.contains("quota-orb--idle")).toBe(true);
  });

  it("ignores expanded hover and drag events while locked", () => {
    const onHover = vi.fn();
    const onDrag = vi.fn();
    const { container } = render(
      <QuotaCard
        snapshot={snapshot}
        preferences={lockedPreferences}
        providerCount={1}
        onPrevious={vi.fn()}
        onNext={vi.fn()}
        onTogglePin={vi.fn()}
        onLock={vi.fn()}
        onToggleStayExpanded={vi.fn()}
        onDrag={onDrag}
        onHover={onHover}
      />,
    );
    const card = container.querySelector(".quota-card");
    expect(card).not.toBeNull();

    fireEvent.mouseEnter(card!);
    fireEvent.mouseLeave(card!);
    fireEvent.mouseDown(card!, { button: 0 });

    expect(card?.classList.contains("quota-card--locked")).toBe(true);
    expect(onHover).not.toHaveBeenCalled();
    expect(onDrag).not.toHaveBeenCalled();
  });
});
