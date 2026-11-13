import { defineConfig } from 'vitest/config'

// Vitest runs with its own config rather than reusing vite.config.ts.
//
// vite.config.ts loads @sveltejs/vite-plugin-svelte, whose `configureServer`
// hook crashes under the Vite that Vitest bundles internally (a different
// major than the one vite-plugin-svelte targets), aborting the whole run
// before a single test executes. The unit tests are plain TypeScript and do
// not compile any `.svelte` component, so we deliberately omit the Svelte and
// Tailwind plugins here. If a future test needs to mount a component, add the
// svelte plugin back and align the Vite/Vitest versions.
export default defineConfig({
  test: {
    environment: 'node',
    include: ['src/**/*.{test,spec}.ts', 'tests/unit/**/*.{test,spec}.ts'],
  },
  resolve: {
    alias: {
      $lib: '/src/lib',
    },
  },
})
