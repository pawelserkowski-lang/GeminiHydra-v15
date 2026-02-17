/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_WITCHER_MODE?: string;
  readonly VITE_BACKEND_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
