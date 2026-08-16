import { config } from '@fohte/eslint-config'
import storybook from 'eslint-plugin-storybook'

export default config(
  {
    typescript: { typeChecked: true },
    errorHandling: {},
  },
  // TanStack Router の自動生成ファイル
  { ignores: ['frontend/src/routeTree.gen.ts'] },
  // projectService はプロセス内で一度だけ生成されるシングルトンのため、files を
  // frontend 配下に絞ると最初にパースされる ts ファイル (glob 順で frontend より前) が
  // 生成条件を決めてしまい allowDefaultProject が無視される。全 ts ファイルに適用する
  {
    files: ['**/*.ts{,x}'],
    languageOptions: {
      parserOptions: {
        projectService: {
          allowDefaultProject: [
            'frontend/.storybook/main.ts',
            'frontend/.storybook/preview.ts',
            'frontend/.storybook/vitest.setup.ts',
          ],
        },
      },
    },
  },
  {
    files: ['frontend/**/*.ts{,x}'],
    rules: {
      // TanStack Router/Query の型定義が any を返すケースがあるため無効化
      '@typescript-eslint/no-unsafe-assignment': 'off',
    },
  },
  // .storybook/ と vitest.config.ts は src 外にあり、相対インポートが必要な参照
  // (vitest.config.ts -> ../vite.config、preview.ts -> ../src/index.css) を含むため
  // no-restricted-imports を無効化する
  {
    files: ['frontend/.storybook/**/*.ts', 'frontend/vitest.config.ts'],
    rules: {
      'no-restricted-imports': 'off',
    },
  },
  ...storybook.configs['flat/recommended'],
)
