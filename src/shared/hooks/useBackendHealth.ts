import { useEffect, useRef, useState } from 'react';
import { checkHealth } from '@/shared/api/client';

export type BackendStatus = 'connected' | 'starting' | 'disconnected';

const POLL_INTERVAL_MS = 15_000; // 15s when healthy
const FAST_POLL_MS = 5_000; // 5s when unhealthy

export function useBackendHealth() {
  const [status, setStatus] = useState<BackendStatus>('disconnected');
  const [uptime, setUptime] = useState<number | undefined>();
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);

  useEffect(() => {
    let mounted = true;

    async function poll() {
      const health = await checkHealth();

      if (!mounted) return;

      if (health.ready) {
        setStatus('connected');
        setUptime(health.uptime_seconds);
      } else if (health.uptime_seconds !== undefined) {
        // Got a response but not ready yet — starting up
        setStatus('starting');
        setUptime(health.uptime_seconds);
      } else {
        setStatus('disconnected');
        setUptime(undefined);
      }

      const interval = health.ready ? POLL_INTERVAL_MS : FAST_POLL_MS;
      timerRef.current = setTimeout(() => void poll(), interval);
    }

    void poll();

    return () => {
      mounted = false;
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, []);

  return { status, uptime };
}
