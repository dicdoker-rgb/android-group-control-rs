import { describe, it, expect, vi, beforeEach } from "vitest";
import { api, apiUrl, API_BASE } from "../api";

beforeEach(() => {
  vi.restoreAllMocks();
});

describe("apiUrl", () => {
  it("prepends API_BASE to path", () => {
    expect(apiUrl("/api/status")).toBe(`${API_BASE}/api/status`);
  });
});

describe("api.get", () => {
  it("returns parsed JSON on success", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({
      ok: true,
      text: () => Promise.resolve('{"connected":true}'),
    }));
    const data = await api.get("/api/status");
    expect(data).toEqual({ connected: true });
    expect(fetch).toHaveBeenCalledWith(apiUrl("/api/status"), undefined);
  });

  it("throws on non-ok response", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({
      ok: false,
      text: () => Promise.resolve('{"error":"not found"}'),
    }));
    await expect(api.get("/api/missing")).rejects.toThrow("not found");
  });

  it("returns empty object for empty body", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({
      ok: true,
      text: () => Promise.resolve(""),
    }));
    const data = await api.get("/api/empty");
    expect(data).toEqual({});
  });
});

describe("api.post", () => {
  it("sends JSON body with POST method", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({
      ok: true,
      text: () => Promise.resolve('{"ok":true}'),
    }));
    await api.post("/api/action", { command: "home" });
    expect(fetch).toHaveBeenCalledWith(
      apiUrl("/api/action"),
      expect.objectContaining({
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: '{"command":"home"}',
      })
    );
  });
});

describe("api.delete", () => {
  it("sends DELETE method", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({
      ok: true,
      text: () => Promise.resolve("{}"),
    }));
    await api.delete("/api/item/1");
    expect(fetch).toHaveBeenCalledWith(
      apiUrl("/api/item/1"),
      expect.objectContaining({ method: "DELETE" })
    );
  });
});

describe("api.upload", () => {
  it("sends file with correct headers", async () => {
    vi.stubGlobal("fetch", vi.fn().mockResolvedValue({
      ok: true,
      text: () => Promise.resolve('{"file_name":"test.apk"}'),
    }));
    const file = new File(["data"], "test.apk", { type: "application/vnd.android.package-archive" });
    const data = await api.upload("/api/upload-apk", file);
    expect(data).toEqual({ file_name: "test.apk" });
    expect(fetch).toHaveBeenCalledWith(
      apiUrl("/api/upload-apk"),
      expect.objectContaining({
        method: "POST",
        headers: {
          "Content-Type": "application/vnd.android.package-archive",
          "X-File-Name": "test.apk",
        },
      })
    );
  });
});
