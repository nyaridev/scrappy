import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 4430, strictPort: true, watch: { ignored: ["**/src-tauri/**", "**/crates/**", "**/extension/**"] } },
  build: { target: "es2022" },
});
