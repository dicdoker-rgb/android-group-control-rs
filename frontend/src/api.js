export const IS_TAURI_DESKTOP =
  typeof window !== "undefined" && window.__TAURI_INTERNALS__;

export const API_BASE =
  IS_TAURI_DESKTOP ? "http://127.0.0.1:8876" : "";

export function apiUrl(path) {
  return `${API_BASE}${path}`;
}

const DEFAULT_TIMEOUT = 30_000; // 30 seconds
const UPLOAD_TIMEOUT = 120_000; // 2 minutes for uploads

function fetchWithTimeout(url, options = {}, timeout = DEFAULT_TIMEOUT) {
  const controller = new AbortController();
  const id = setTimeout(() => controller.abort(), timeout);
  return fetch(url, { ...options, signal: controller.signal }).finally(() =>
    clearTimeout(id)
  );
}

export const api = {
  async get(path, init) {
    const response = await fetchWithTimeout(apiUrl(path), init);
    const text = await response.text();
    const data = text ? JSON.parse(text) : {};
    if (!response.ok) {
      throw new Error(data.error || "请求失败");
    }
    return data;
  },
  post(path, body) {
    return this.get(path, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
  },
  delete(path) {
    return this.get(path, { method: "DELETE" });
  },
  async upload(path, file) {
    const response = await fetchWithTimeout(
      apiUrl(path),
      {
        method: "POST",
        headers: {
          "Content-Type": file.type || "application/octet-stream",
          "X-File-Name": file.name || "upload.apk",
        },
        body: file,
      },
      UPLOAD_TIMEOUT
    );
    const text = await response.text();
    const data = text ? JSON.parse(text) : {};
    if (!response.ok) {
      throw new Error(data.error || "上传失败");
    }
    return data;
  },
};

export const hostApi = {
  getHosts: () => api.get("/api/hosts"),
  addHost: (data) => api.post("/api/hosts", data),
  updateHost: (id, data) =>
    fetch(apiUrl(`/api/hosts/${id}`), {
      method: "PUT",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(data),
    }).then(async (r) => {
      const text = await r.text();
      const d = text ? JSON.parse(text) : {};
      if (!r.ok) throw new Error(d.error || "更新失败");
      return d;
    }),
  deleteHost: (id) => api.delete(`/api/hosts/${id}`),
  testHost: (id) => api.post(`/api/hosts/${id}/test`),
  getVms: (hostId) => api.get(`/api/hosts/${hostId}/vms`),
  createVms: (hostId, count) => api.post(`/api/hosts/${hostId}/vms`, { count }),
  startVm: (hostId, name) => api.post(`/api/hosts/${hostId}/vms/${name}/start`),
  stopVm: (hostId, name) => api.post(`/api/hosts/${hostId}/vms/${name}/stop`),
  restartVm: (hostId, name) => api.post(`/api/hosts/${hostId}/vms/${name}/restart`),
  deleteVm: (hostId, name) => api.delete(`/api/hosts/${hostId}/vms/${name}`),
  batchVmAction: (hostId, names, action) =>
    api.post(`/api/hosts/${hostId}/vms/batch`, { names, action }),
  getStats: (hostId) => api.get(`/api/hosts/${hostId}/stats`),
};
