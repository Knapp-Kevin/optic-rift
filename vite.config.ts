import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    strictPort: true,
    port: 1420,
    // The Rust build tree is not frontend source. On Windows, watching it makes
    // Vite crash with EBUSY while cargo links the locked debug executable.
    watch: { ignored: ["**/src-tauri/**"] }
  }
});
