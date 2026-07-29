import { defineConfig } from "vite";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  root: "frontend",
  plugins: [tailwindcss()],
  build: {
    outDir: "dist",
    assetsDir: ".",
    rollupOptions: {
      input: "src/main.js",
      output: {
        entryFileNames: "main.js",
        assetFileNames: "[name][extname]",
      },
    },
  },
});
