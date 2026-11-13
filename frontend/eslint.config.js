import globals from 'globals'
import tseslint from 'typescript-eslint'
import svelte from 'eslint-plugin-svelte'

export default [
  ...tseslint.configs.recommended,
  ...svelte.configs['flat/recommended'],

  {
    languageOptions: {
      globals: { ...globals.browser, ...globals.es2022 },
    },
  },

  {
    files: ['**/*.svelte'],
    languageOptions: {
      parserOptions: {
        parser: tseslint.parser,
      },
    },
  },

  {
    rules: {
      // Enforce the style guide: unknown over any
      '@typescript-eslint/no-explicit-any': 'error',
      // A leading underscore marks an intentionally-unused binding — the
      // convention already used across the Rust and TS code (`_e`, `_id`,
      // `_bastions`). Don't flag those.
      '@typescript-eslint/no-unused-vars': [
        'error',
        {
          argsIgnorePattern: '^_',
          varsIgnorePattern: '^_',
          caughtErrorsIgnorePattern: '^_',
        },
      ],
    },
  },

  {
    ignores: ['dist/', 'node_modules/', '.svelte-kit/'],
  },
]
