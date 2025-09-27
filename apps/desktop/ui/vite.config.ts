import path from "path"
import tailwindcss from "@tailwindcss/vite"
import { defineConfig } from "vitest/config"
import react from "@vitejs/plugin-react"

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  optimizeDeps: {
    exclude: ['@tauri-apps/api', '@tauri-apps/api/*'],
  },
  build: {
    commonjsOptions: {
      include: [/node_modules/, /tauri-apps\/.*/],
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    host: "localhost",
    hmr: {
      host: "localhost",
      port: 1420,
    },
  },
  preview: {
    port: 1420,
    strictPort: true,
  },
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: "./src/test/setup.ts",
    css: true,
  },
})



