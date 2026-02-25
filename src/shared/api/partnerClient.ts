/**
 * Partner API client — fetches sessions from ClaudeHydra backend.
 * Dev: proxied via Vite /partner-api → http://localhost:8082/api
 * Prod: direct call to ClaudeHydra fly.io backend
 */

const PARTNER_BASE = import.meta.env.PROD
  ? 'https://claudehydra-v4-backend.fly.dev/api'
  : '/partner-api';

export interface PartnerSessionSummary {
  id: string;
  title: string;
  created_at: string;
  message_count: number;
  updated_at?: string;
  preview?: string;
}

export interface PartnerMessage {
  id: string;
  role: string;
  content: string;
  model?: string | null;
  timestamp: string;
  agent?: string | null;
}

export interface PartnerSession {
  id: string;
  title: string;
  created_at: string;
  messages: PartnerMessage[];
}

export async function fetchPartnerSessions(): Promise<PartnerSessionSummary[]> {
  const res = await fetch(`${PARTNER_BASE}/sessions`, {
    signal: AbortSignal.timeout(5000),
  });
  if (!res.ok) throw new Error(`Partner API error: ${res.status}`);
  return res.json();
}

export async function fetchPartnerSession(id: string): Promise<PartnerSession> {
  const res = await fetch(`${PARTNER_BASE}/sessions/${id}`, {
    signal: AbortSignal.timeout(10000),
  });
  if (!res.ok) throw new Error(`Partner API error: ${res.status}`);
  return res.json();
}
