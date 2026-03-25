import { describe, it, expect, beforeEach, vi } from "vitest";
import { readStoredPrefs, writeStoredPrefs } from "../hooks/usePrefs";
import { UI_PREFS_KEY } from "../constants";

const store = {};
const mockStorage = {
  getItem: (key) => store[key] ?? null,
  setItem: (key, val) => { store[key] = String(val); },
  removeItem: (key) => { delete store[key]; },
  clear: () => { Object.keys(store).forEach((k) => delete store[k]); },
};

beforeEach(() => {
  Object.keys(store).forEach((k) => delete store[k]);
  vi.stubGlobal("localStorage", mockStorage);
  Object.defineProperty(window, "localStorage", { value: mockStorage, writable: true });
});

describe("readStoredPrefs", () => {
  it("returns empty object when nothing stored", () => {
    expect(readStoredPrefs()).toEqual({});
  });

  it("returns parsed prefs from localStorage", () => {
    window.localStorage.setItem(UI_PREFS_KEY, JSON.stringify({ filterMode: "online" }));
    expect(readStoredPrefs()).toEqual({ filterMode: "online" });
  });

  it("returns empty object on invalid JSON", () => {
    window.localStorage.setItem(UI_PREFS_KEY, "not-json{{{");
    expect(readStoredPrefs()).toEqual({});
  });
});

describe("writeStoredPrefs", () => {
  it("writes prefs to localStorage", () => {
    writeStoredPrefs({ gridDensityMode: "compact" });
    const stored = JSON.parse(window.localStorage.getItem(UI_PREFS_KEY));
    expect(stored).toEqual({ gridDensityMode: "compact" });
  });

  it("overwrites previous prefs", () => {
    writeStoredPrefs({ a: 1 });
    writeStoredPrefs({ b: 2 });
    const stored = JSON.parse(window.localStorage.getItem(UI_PREFS_KEY));
    expect(stored).toEqual({ b: 2 });
  });
});
