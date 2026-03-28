import { defineConfig, mergeConfig } from 'vitest/config'

// eslint-disable-next-line no-restricted-imports -- vitest は vite.config の設定をマージする必要がある
import viteConfig from './vite.config'

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: 'jsdom',
      setupFiles: ['./src/test-setup.ts'],
    },
  }),
)
