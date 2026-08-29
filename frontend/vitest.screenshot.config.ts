import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { createStorybookProject } from '@fohte/storybook-addon/vitest-plugin'
import { defineConfig, mergeConfig } from 'vitest/config'

import { SCREENSHOT_VIEWPORT } from './.storybook/screenshot-viewport'
import viteConfig from './vite.config'

const dirname = path.dirname(fileURLToPath(import.meta.url))

// `storybook` project (通常の check 実行用、vitest.config.ts) と同じ configDir を
// 同一プロセス内で同時に触ると Storybook 側のキャッシュ/開発サーバーが競合するため、
// 撮影用の project は独立した config ファイルに分離している
export default mergeConfig(
  viteConfig,
  defineConfig(
    createStorybookProject({
      name: 'storybook-screenshot-frontend',
      rootDir: dirname,
      viewport: SCREENSHOT_VIEWPORT,
      screenshotsSubdir: 'desktop',
      setupFiles: ['./.storybook/vitest.setup.screenshot.ts'],
    }),
  ),
)
