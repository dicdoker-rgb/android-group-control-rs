import { useState, useEffect, useRef } from "react";
import { apiUrl } from "../api";

export function useHostStats(hostId) {
  const [stats, setStats] = useState(null);
  const sourceRef = useRef(null);

  useEffect(() => {
    if (!hostId) { setStats(null); return; }
    const es = new EventSource(apiUrl(`/api/hosts/${hostId}/stats/stream`));
    sourceRef.current = es;
    es.addEventListener("stats", (e) => {
      try { setStats(JSON.parse(e.data)); } catch {}
    });
    es.onerror = () => { setStats(null); };
    return () => { es.close(); sourceRef.current = null; };
  }, [hostId]);

  return stats;
}
