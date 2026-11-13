import js from '@eslint/js';
import prettier from 'eslint-config-prettier';
import globals from 'globals';

export default [
  { ignores: ['node_modules/', 'results/'] },
  js.configs.recommended,
  {
    files: ['**/*.js'],
    languageOptions: {
      ecmaVersion: 2023,
      sourceType: 'module',
      // k6 is not Node: only its own globals exist at run time.
      globals: {
        ...globals.es2023,
        __ENV: 'readonly',
        __VU: 'readonly',
        __ITER: 'readonly',
        open: 'readonly',
      },
    },
    rules: { 'no-console': 'error', eqeqeq: 'error' },
  },
  prettier,
];
