import { config } from '@fohte/eslint-config'

export default config(
<<<<<<< before updating
  { typescript: { typeChecked: true } },
  // TanStack Router の自動生成ファイル
  { ignores: ['src/routeTree.gen.ts'] },
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
||||||| last update
  { typescript: { typeChecked: true } },
=======
>>>>>>> after updating
  {
<<<<<<< before updating
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
||||||| last update
    rules: {
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
=======
    typescript: { typeChecked: true },
    errorHandling: {},
>>>>>>> after updating
  },
  // .storybook/ と vitest.config.ts は src 外にあり @ エイリアスが使えないため相対インポートを許可
  {
    files: ['.storybook/**/*.ts', 'vitest.config.ts'],
    rules: {
      'no-restricted-imports': 'off',
    },
  },
)
