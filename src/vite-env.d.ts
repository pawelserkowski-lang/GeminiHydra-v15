/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_BACKEND_URL?: string;
  readonly VITE_AUTH_SECRET?: string;
  readonly VITE_PARTNER_BACKEND_URL?: string;
  readonly VITE_PARTNER_AUTH_SECRET?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
