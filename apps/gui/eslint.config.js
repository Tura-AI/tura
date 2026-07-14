import js from "@eslint/js";
import prettier from "eslint-config-prettier";
import tseslint from "typescript-eslint";

const browserGlobals = {
  AbortController: "readonly",
  Blob: "readonly",
  BroadcastChannel: "readonly",
  CSS: "readonly",
  CustomEvent: "readonly",
  DOMParser: "readonly",
  EventSource: "readonly",
  File: "readonly",
  FormData: "readonly",
  HTMLElement: "readonly",
  IntersectionObserver: "readonly",
  KeyboardEvent: "readonly",
  MediaRecorder: "readonly",
  MessageEvent: "readonly",
  MouseEvent: "readonly",
  MutationObserver: "readonly",
  Notification: "readonly",
  PointerEvent: "readonly",
  ResizeObserver: "readonly",
  Response: "readonly",
  URL: "readonly",
  URLSearchParams: "readonly",
  WebSocket: "readonly",
  Window: "readonly",
  clearInterval: "readonly",
  clearTimeout: "readonly",
  console: "readonly",
  document: "readonly",
  fetch: "readonly",
  localStorage: "readonly",
  navigator: "readonly",
  requestAnimationFrame: "readonly",
  setInterval: "readonly",
  setTimeout: "readonly",
  window: "readonly",
};

const nodeGlobals = {
  Buffer: "readonly",
  console: "readonly",
  process: "readonly",
  setTimeout: "readonly",
};

export default tseslint.config(
  {
    ignores: [
      "**/*.test.ts",
      "**/*.test.tsx",
      "app/dist/**",
      "app/node_modules/**",
      "app/public/**",
      "app/.vite/**",
      "node_modules/**",
      "test-results/**",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["app/src/**/*.{ts,tsx}", "sdk/**/*.ts"],
    languageOptions: {
      parserOptions: {
        project: ["./tsconfig.eslint.json"],
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      "no-unused-expressions": "off",
      "@typescript-eslint/no-unused-expressions": "off",
      "@typescript-eslint/consistent-type-imports": [
        "error",
        { fixStyle: "inline-type-imports" },
      ],
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_" },
      ],
    },
  },
  {
    files: ["app/src/**/*.{ts,tsx}"],
    languageOptions: {
      globals: browserGlobals,
    },
  },
  {
    files: ["app/vite.config.ts"],
    languageOptions: {
      globals: nodeGlobals,
      parserOptions: {
        project: ["./tsconfig.eslint.json"],
        tsconfigRootDir: import.meta.dirname,
      },
    },
  },
  prettier,
);
