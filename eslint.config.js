import { config } from '@fohte/eslint-config'
import storybook from 'eslint-plugin-storybook'

export default config(
  {
    typescript: { typeChecked: true },
    errorHandling: {},
    tailwind: { cssConfigPath: 'frontend/src/index.css' },
  },
  ...storybook.configs['flat/recommended'],
  {
    // vite.config.ts/vitest.config.ts are loaded through Vite's own
    // esbuild-based config loader, which doesn't resolve the package.json
    // "imports" field, unlike the Rollup pipeline that bundles the app
    // itself. .storybook/**/*.ts is loaded the same way, through
    // Storybook's own Node-based config loader.
    files: [
      'frontend/.storybook/**/*.ts',
      'frontend/vite.config.ts',
      'frontend/vitest.config.ts',
    ],
    rules: { 'no-restricted-imports': 'off' },
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
      // 既存コンポーネントが Tailwind の任意値記法 (text-[13px] 等) に
      // 広く依存しているため無効化
      'tailwindcss/no-arbitrary-value': 'off',
    },
  },
)
