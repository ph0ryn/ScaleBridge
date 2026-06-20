import oxlint from "eslint-plugin-oxlint";
import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";

export default defineConfig(
  {
    ignores: ["src-tauri/**", "crates/**"],
  },
  {
    files: ["src/**/*.{ts,tsx}"],
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    plugins: {
      "@typescript-eslint": tseslint.plugin,
    },
    rules: {
      "@typescript-eslint/naming-convention": [
        "error",
        {
          selector: "class",
          format: ["StrictPascalCase"],
        },
        {
          selector: "variable",
          format: ["strictCamelCase", "UPPER_CASE"],
        },
        {
          selector: "parameter",
          format: ["strictCamelCase"],
        },
      ],
    },
  },
  oxlint.buildFromOxlintConfigFile("./oxlint.config.ts"),
);
