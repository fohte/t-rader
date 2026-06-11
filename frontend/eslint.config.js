import { config } from '@fohte/eslint-config'

export default config(
  { typescript: { typeChecked: true } },
  {
    files: ['**/*.ts{,x}'],
    languageOptions: {
      parserOptions: {
        projectService: {
          allowDefaultProject: ['.storybook/main.ts', '.storybook/preview.ts'],
        },
      },
    },
  },
  {
    rules: {
      // TanStack Router/Query の型定義が any を返すケースがあるため無効化
      '@typescript-eslint/no-unsafe-assignment': 'off',
      'no-restricted-imports': [
        'error',
        {
          patterns: [
            {
              group: ['./*', '../*'],
              message:
                'Please use absolute imports instead of relative imports.',
            },
          ],
        },
      ],
    },
  },
  // .storybook/ と vitest.config.ts は src 外にあり @ エイリアスが使えないため相対インポートを許可
  {
    files: ['.storybook/**/*.ts', 'vitest.config.ts'],
    rules: {
      'no-restricted-imports': 'off',
    },
  },
)
