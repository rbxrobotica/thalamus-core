import { defineConfig } from "vite";

export default defineConfig({
  root: "src",
  build: {
    outDir: "../dist",
    emptyOutDir: true,
  },
  resolve: {
    alias: {
      "@rbx/thalamus-sdk": "../sdks/typescript/src/index.ts",
    },
  },
});
