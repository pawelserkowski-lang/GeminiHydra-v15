# Deployment

## Environment Variables

| Variable           | Required | Default          | Description                             |
|--------------------|----------|------------------|-----------------------------------------|
| `GOOGLE_API_KEY`   | yes      | --               | Google Gemini API key                   |
| `GEMINI_API_KEY`   | fallback | --               | Alternative name for `GOOGLE_API_KEY`   |
| `ANTHROPIC_API_KEY`| no       | --               | Anthropic Claude API key (optional)     |
| `PORT`             | no       | `8081`           | Backend listen port                     |
| `RUST_LOG`         | no       | `info`           | Tracing log level filter                |

The backend reads `.env` from its working directory via `dotenvy`. Place it in `backend/.env` for development.

---

## Build Commands

### Frontend

```bash
# Install dependencies
pnpm install

# Development server (port 5176, proxies /api to :8081)
pnpm dev

# Production build
pnpm build

# Preview production build locally
pnpm preview

# Lint and format
pnpm lint
pnpm lint:fix
pnpm format
```

The production build outputs to `dist/` as static files (HTML, JS, CSS).

### Backend

```bash
cd backend

# Development (debug build)
cargo run

# Release build
cargo build --release

# Run release binary
./target/release/geminihydra-backend

# Run tests
cargo test
```

The release binary is a single statically-linked executable at `backend/target/release/geminihydra-backend` (or `.exe` on Windows).

---

## Production Deployment

### Option 1: Standalone

1. Build the frontend:
   ```bash
   pnpm build
   ```

2. Build the backend:
   ```bash
   cd backend && cargo build --release
   ```

3. Serve `dist/` via any static file server (nginx, caddy, etc.)

4. Run the backend binary with environment variables:
   ```bash
   GOOGLE_API_KEY=your-key PORT=8081 ./backend/target/release/geminihydra-backend
   ```

5. Configure your reverse proxy to route `/api/*` to `http://localhost:8081`.

### Option 2: Docker

```dockerfile
# ---- Backend Build Stage ----
FROM rust:1.82-slim AS backend-build
WORKDIR /app/backend
COPY backend/ .
RUN cargo build --release

# ---- Frontend Build Stage ----
FROM node:20-slim AS frontend-build
RUN npm i -g pnpm@9
WORKDIR /app
COPY package.json pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY . .
RUN pnpm build

# ---- Runtime Stage ----
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=backend-build /app/backend/target/release/geminihydra-backend ./
COPY --from=frontend-build /app/dist ./dist

ENV PORT=8081
EXPOSE 8081

CMD ["./geminihydra-backend"]
```

> Note: In this Docker setup you would need the backend to also serve static files from `dist/`, or use a separate nginx container as a reverse proxy.

```bash
# Build
docker build -t geminihydra:v15 .

# Run
docker run -d \
  -p 8081:8081 \
  -e GOOGLE_API_KEY=your-key \
  -e ANTHROPIC_API_KEY=your-key \
  --name geminihydra \
  geminihydra:v15
```

### Option 3: Docker Compose

```yaml
version: "3.9"
services:
  geminihydra:
    build: .
    ports:
      - "8081:8081"
    environment:
      - GOOGLE_API_KEY=${GOOGLE_API_KEY}
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
      - PORT=8081
      - RUST_LOG=info
    restart: unless-stopped
```

---

## CORS Configuration

The backend allows requests from these origins by default:

- `http://localhost:5176` (Vite dev server)
- `http://localhost:5173` (Vite default port)
- `http://localhost:3000` (common dev port)

For production, update the `CorsLayer` configuration in `backend/src/main.rs` to include your production domain.

---

## Request Limits

- **Request body size**: 10 MB max (configured via `RequestBodyLimitLayer`)
- **History**: in-memory, not persisted to disk -- resets on backend restart
- **Memory/Knowledge Graph**: in-memory, same caveat

For persistent storage, consider adding a database layer (SQLite, PostgreSQL) in a future version.
