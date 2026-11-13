import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import tailwindcss from '@tailwindcss/vite'

// TAURI_DEV_HOST is set by the Tauri CLI when running on a device or emulator.
const host = process.env['TAURI_DEV_HOST']

export default defineConfig({
  plugins: [
    svelte(),
    tailwindcss(),
  ],

  // Vite options tailored for Tauri development:
  clearScreen: false,

  server: {
    port: 1420,
    strictPort: true,
    host: host ?? false,
    hmr: host
      ? { protocol: 'ws', host, port: 1421 }
      : undefined,
    watch: {
      // Exclude Tauri build output from Vite's watcher to prevent infinite rebuilds.
      ignored: ['**/src-tauri/**', '**/target/**'],
    },
  },

  resolve: {
    alias: {
      $lib: '/src/lib',
    },
  },
})
