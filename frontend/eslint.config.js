import { config } from '@fohte/eslint-config'

export default config(
  {
    typescript: { typeChecked: true },
    errorHandling: {},
  },
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
  {
    rules: {
      // TanStack Router/Query の型定義が any を返すケースがあるため無効化
      '@typescript-eslint/no-unsafe-assignment': 'off',
    },
  },
  // .storybook/ と vitest.config.ts は src 外にあり # subpath imports (src 配下の *.ts のみ解決) が届かない対象 (CSS、ルート直下の設定ファイル) を参照するため相対インポートを許可
  {
    files: ['.storybook/**/*.ts', 'vitest.config.ts'],
    rules: {
      'no-restricted-imports': 'off',
    },
  },
)
