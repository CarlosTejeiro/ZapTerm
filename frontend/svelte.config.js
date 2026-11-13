import { vitePreprocess } from '@sveltejs/vite-plugin-svelte'

export default {
  preprocess: vitePreprocess(),
  compilerOptions: {
    // Enforce Svelte 5 runes mode across the entire project.
    // Legacy stores are forbidden in new code (style guide §3).
    runes: true,
  },
}
