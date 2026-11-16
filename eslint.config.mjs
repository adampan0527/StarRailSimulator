import js from '@eslint/js';
import tseslint from 'typescript-eslint';

export default [
  {
    ignores: [
      'dist/**',
      'target/**',
      'src-tauri/gen/**',
      '.local-toolchain/**',
      '.m03-data/**',
      'html_report/**',
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
];
