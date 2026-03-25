import { useState, useEffect, useCallback } from "react";
import { hostApi } from "../api";

export function useHosts() {
  const [hosts, setHosts] = useState([]);
  const [activeHostId, setActiveHostId] = useState(null);
  const [vms, setVms] = useState([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState(null);

  const refreshHosts = useCallback(async () => {
    try {
      const data = await hostApi.getHosts();
      setHosts(data.hosts || []);
      if (!activeHostId && data.hosts?.length) {
        setActiveHostId(data.hosts[0].id);
      }
    } catch (e) {
      console.error("refreshHosts", e);
      setError(e.message);
    }
  }, [activeHostId]);

  const refreshVms = useCallback(async () => {
    if (!activeHostId) return;
    setLoading(true);
    try {
      const data = await hostApi.getVms(activeHostId);
      setVms(data.vms || []);
      setError(null);
    } catch (e) {
      console.error("refreshVms", e);
      setError(e.message);
    }
    setLoading(false);
  }, [activeHostId]);

  useEffect(() => { refreshHosts(); }, []);
  useEffect(() => { if (activeHostId) refreshVms(); }, [activeHostId, refreshVms]);

  const addHost = async (config) => {
    try {
      const data = await hostApi.addHost(config);
      setHosts(data.hosts || []);
      if (!activeHostId && data.hosts?.length) {
        setActiveHostId(data.hosts[0].id);
      }
    } catch (e) { setError(e.message); throw e; }
  };

  const deleteHost = async (id) => {
    try {
      const data = await hostApi.deleteHost(id);
      setHosts(data.hosts || []);
      if (activeHostId === id) {
        setActiveHostId(data.hosts?.[0]?.id || null);
      }
    } catch (e) { setError(e.message); throw e; }
  };

  const testHost = async (id) => {
    try {
      return await hostApi.testHost(id);
    } catch (e) { setError(e.message); throw e; }
  };

  const createVms = async (count) => {
    if (!activeHostId) return;
    setLoading(true);
    try {
      const data = await hostApi.createVms(activeHostId, count);
      setVms(data.vms || []);
      setError(null);
    } catch (e) { setError(e.message); }
    setLoading(false);
  };

  const vmAction = async (name, action) => {
    if (!activeHostId) return;
    try {
      const fn = {
        start: hostApi.startVm,
        stop: hostApi.stopVm,
        restart: hostApi.restartVm,
        delete: hostApi.deleteVm,
      }[action];
      if (!fn) return;
      const data = await fn(activeHostId, name);
      if (data.vms) setVms(data.vms);
    } catch (e) { setError(e.message); }
  };

  const batchAction = async (names, action) => {
    if (!activeHostId) return;
    setLoading(true);
    try {
      const data = await hostApi.batchVmAction(activeHostId, names, action);
      if (data.vms) setVms(data.vms);
    } catch (e) { setError(e.message); }
    setLoading(false);
  };

  return {
    hosts, activeHostId, setActiveHostId, vms, loading, error,
    refreshHosts, refreshVms, addHost, deleteHost, testHost,
    createVms, vmAction, batchAction,
  };
}
