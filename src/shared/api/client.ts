/** Jaskier Shared Pattern */
// src/shared/api/client.ts
/**
 * GeminiHydra v15 - Typed API Client
 * ====================================
 * Fetch wrapper for the Rust/Axum backend on port 8081.
 * Provides typed GET/POST/PATCH/DELETE with ApiError handling
 * and automatic retry with exponential backoff for network failures.
 */

const BASE_URL = import.meta.env.VITE_BACKEND_URL ?? (import.meta.env.PROD ? 'https://geminihydra-v15-backend.fly.dev' : '');
const AUTH_SECRET = import.meta.env.VITE_AUTH_SECRET as string | undefined;

const MAX_RETRIES = 3;
const RETRY_BASE_MS = 1000;

// -------------------------------------------------------------------
// Error class
// -------------------------------------------------------------------

export class ApiError extends Error {
  readonly status: number;
  readonly statusText: string;
  readonly body: unknown;

  constructor(status: number, statusText: string, body: unknown) {
    super(`API Error ${status}: ${statusText}`);
    this.name = 'ApiError';
    this.status = status;
    this.statusText = statusText;
    this.body = body;
  }
}

// -------------------------------------------------------------------
// Retry wrapper
// -------------------------------------------------------------------

/** Retry on network errors (TypeError = "Failed to fetch") with exponential backoff. */
async function fetchWithRetry(
  url: string,
  init: RequestInit,
  retries = MAX_RETRIES,
): Promise<Response> {
  for (let attempt = 0; attempt <= retries; attempt++) {
    try {
      const response = await fetch(url, init);
      // Don't retry on HTTP errors (4xx/5xx) — only network failures
      return response;
    } catch (err) {
      // TypeError = network failure ("Failed to fetch")
      if (attempt < retries && err instanceof TypeError) {
        const delay = RETRY_BASE_MS * 2 ** attempt;
        console.warn(
          `[api] Network error on ${init.method ?? 'GET'} ${url}, retrying in ${String(delay)}ms (${String(attempt + 1)}/${String(retries)})`,
        );
        await new Promise((r) => setTimeout(r, delay));
        continue;
      }
      throw err;
    }
  }
  // Should never reach here, but TypeScript needs it
  throw new TypeError('Failed to fetch after retries');
}

// -------------------------------------------------------------------
// Internal fetch helper
// -------------------------------------------------------------------

async function apiFetch<T>(path: string, options: RequestInit = {}): Promise<T> {
  const url = `${BASE_URL}${path}`;

  const response = await fetchWithRetry(url, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...(AUTH_SECRET ? { Authorization: `Bearer ${AUTH_SECRET}` } : {}),
      ...options.headers,
    },
  });

  if (!response.ok) {
    let body: unknown;
    try {
      body = await response.json();
    } catch {
      body = await response.text().catch(() => null);
    }
    throw new ApiError(response.status, response.statusText, body);
  }

  // Handle 204 No Content
  if (response.status === 204) {
    return undefined as T;
  }

  return response.json() as Promise<T>;
}

// -------------------------------------------------------------------
// Public API
// -------------------------------------------------------------------

export async function apiGet<T>(path: string): Promise<T> {
  return apiFetch<T>(path, { method: 'GET' });
}

export async function apiPost<T>(path: string, body?: unknown): Promise<T> {
  return apiFetch<T>(path, {
    method: 'POST',
    ...(body !== undefined && { body: JSON.stringify(body) }),
  });
}

export async function apiPatch<T>(path: string, body?: unknown): Promise<T> {
  return apiFetch<T>(path, {
    method: 'PATCH',
    ...(body !== undefined && { body: JSON.stringify(body) }),
  });
}

export async function apiDelete<T>(path: string): Promise<T> {
  return apiFetch<T>(path, { method: 'DELETE' });
}

// -------------------------------------------------------------------
// Health check
// -------------------------------------------------------------------

export interface HealthStatus {
  ready: boolean;
  uptime_seconds?: number;
}

/** Lightweight readiness check — no retries, short timeout. */
export async function checkHealth(): Promise<HealthStatus> {
  try {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 5000);

    const response = await fetch(`${BASE_URL}/api/health/ready`, {
      method: 'GET',
      signal: controller.signal,
    });

    clearTimeout(timeout);

    if (response.ok) {
      return (await response.json()) as HealthStatus;
    }

    // 503 = starting up
    if (response.status === 503) {
      const body = (await response.json()) as HealthStatus;
      return { ready: false, ...(body.uptime_seconds !== undefined && { uptime_seconds: body.uptime_seconds }) };
    }

    return { ready: false };
  } catch {
    return { ready: false };
  }
}
