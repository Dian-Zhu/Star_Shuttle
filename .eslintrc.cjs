module.exports = {
  root: true,
  parser: '@typescript-eslint/parser',
  extends: [
    'eslint:recommended',
    '@typescript-eslint/recommended',
    'prettier',
    'plugin:svelte/recommended',
  ],
  plugins: ['@typescript-eslint'],
  ignorePatterns: ['*.js', 'node_modules/'],
  overrides: [
    {
      files: ['*.svelte'],
      parser: 'svelte-eslint-parser',
      parserOptions: {
        parser: '@typescript-eslint/parser',
        sourceType: 'module',
      },
    },
  ],
  parserOptions: {
    sourceType: 'module',
    ecmaVersion: 2020,
  },
  rules: {
    '@typescript-eslint/no-explicit-any': 'off',
  },
  env: {
    browser: true,
    es2017: true,
    node: true,
  },
}
