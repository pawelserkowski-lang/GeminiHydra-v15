# Rate Limiting & 429 Too Many Requests Handling

## Backend (Axum + tower_governor)
The backend imposes rate limits via the `tower_governor` layer. This protects against brute-force attacks and unintentional polling spam. By default, API routes are limited to a certain number of requests per IP over an interval.

## Frontend Mitigation
When an agent or polling hook hits the rate limit, it will receive an HTTP 429 status code.

1. **Backoff Mechanism**: Implementing exponential backoff is crucial. Do not blindly retry immediately.
2. **Polling Tweaks**: Hooks like `useHealthQuery` or `useSystemStatsQuery` must balance real-time updates against token usage. A standard refetch interval of 30 seconds is appropriate for most metrics.
3. **User Feedback**: The frontend must gracefully catch the 429 error and perhaps display a "Connecting..." or "Slow down" indicator instead of throwing a generic "Offline" error.
4. **Development Flags**: We can optionally disable the limit entirely in the `.env` via `RATE_LIMIT_ENABLED=false` or just drastically increase the burst capacity during local dev.