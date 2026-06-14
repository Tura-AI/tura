import js from "@eslint/js";
import prettier from "eslint-config-prettier";
import tseslint from "typescript-eslint";

const nodeGlobals = {
  AbortController: "readonly",
  Buffer: "readonly",
  URL: "readonly",
  URLSearchParams: "readonly",
  console: "readonly",
  clearInterval: "readonly",
  clearTimeout: "readonly",
  fetch: "readonly",
  process: "readonly",
  setInterval: "readonly",
  setTimeout: "readonly",
};

const browserGlobals = {
  DataTransfer: "readonly",
  DragEvent: "readonly",
  File: "readonly",
  document: "readonly",
  getComputedStyle: "readonly",
  navigator: "readonly",
  window: "readonly",
};

export default tseslint.config(
  {
    ignores: ["dist/**", "node_modules/**", "test-results/**", ".tura/**"],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["src/**/*.ts", "tests/**/*.ts"],
    languageOptions: {
      globals: nodeGlobals,
      parserOptions: {
        project: ["./tsconfig.eslint.json"],
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      "no-control-regex": "off",
      "@typescript-eslint/consistent-type-imports": ["error", { fixStyle: "inline-type-imports" }],
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_", ignoreRestSiblings: true },
      ],
    },
  },
  {
    files: ["scripts/**/*.mjs", "tests/**/*.mjs"],
    languageOptions: {
      ecmaVersion: 2022,
      sourceType: "module",
      globals: nodeGlobals,
    },
    rules: {
      "@typescript-eslint/no-var-requires": "off",
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_", varsIgnorePattern: "^_", ignoreRestSiblings: true },
      ],
      "no-control-regex": "off",
    },
  },
  {
    files: ["tests/e2e/**/*.mjs", "tests/live/**/*.mjs"],
    languageOptions: {
      globals: {
        ...nodeGlobals,
        ...browserGlobals,
      },
    },
  },
  prettier,
);
