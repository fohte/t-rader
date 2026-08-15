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
  // .storybook/ と vitest.config.ts は src 外にあり # subpath imports (src 配下の *.ts のみ解決) が届かない対象 (CSS、ルート直下の設定ファイル) を参照するため相対インポートを許可
  {
    files: ['frontend/.storybook/**/*.ts', 'frontend/vitest.config.ts'],
    rules: {
      'no-restricted-imports': 'off',
    },
  },
  ...storybook.configs['flat/recommended'],
)
