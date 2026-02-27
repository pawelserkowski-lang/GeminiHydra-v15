"""Environment configuration for GeminiHydra ADK sidecar."""

import os
from dotenv import load_dotenv

load_dotenv()

GOOGLE_API_KEY = os.environ.get("GOOGLE_API_KEY", "")
DATABASE_URL = os.environ.get("DATABASE_URL", "postgres://gemini:gemini_local@localhost:5432/geminihydra")
RUST_BACKEND_URL = os.environ.get("RUST_BACKEND_URL", "http://localhost:8081")
DEFAULT_MODEL = os.environ.get("DEFAULT_MODEL", "gemini-2.0-flash")
PORT = int(os.environ.get("PORT", "8000"))
AUTH_SECRET = os.environ.get("AUTH_SECRET", "")
