import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { BLOCK_EXTERNAL_REQUESTS_ARGS } from '@fohte/storybook-addon/vitest-plugin'
import { storybookTest } from '@storybook/addon-vitest/vitest-plugin'
import { playwright } from '@vitest/browser-playwright'
import { defineConfig, mergeConfig } from 'vitest/config'

import viteConfig from './vite.config'
import { tapLinkOnTouch } from './vitest/browser-commands'

const dirname = path.dirname(fileURLToPath(import.meta.url))

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      projects: [
        {
          extends: true,
          test: {
            name: 'unit',
            include: ['src/**/*.test.{ts,tsx}'],
            exclude: ['src/**/*.browser.test.{ts,tsx}'],
            environment: 'jsdom',
            setupFiles: ['./src/test-setup.ts'],
          },
        },
        {
          extends: true,
          test: {
            name: 'browser',
            include: ['src/**/*.browser.test.{ts,tsx}'],
            exclude: ['src/**/*.touch.browser.test.{ts,tsx}'],
            browser: {
              enabled: true,
              provider: playwright({
                launchOptions: { args: [...BLOCK_EXTERNAL_REQUESTS_ARGS] },
              }),
              headless: true,
              instances: [{ browser: 'chromium' }],
            },
            setupFiles: ['./src/test-setup.ts'],
          },
        },
        {
          extends: true,
          test: {
            name: 'browser-touch',
            include: ['src/**/*.touch.browser.test.{ts,tsx}'],
            browser: {
              enabled: true,
              provider: playwright({
                launchOptions: { args: [...BLOCK_EXTERNAL_REQUESTS_ARGS] },
                contextOptions: { hasTouch: true },
              }),
              headless: true,
              instances: [{ browser: 'chromium' }],
              commands: { tapLinkOnTouch },
            },
            setupFiles: ['./src/test-setup.ts'],
          },
        },
        {
          extends: true,
          plugins: [
            storybookTest({ configDir: path.join(dirname, '.storybook') }),
          ],
          test: {
            name: 'storybook',
            browser: {
              enabled: true,
              provider: playwright({
                launchOptions: { args: [...BLOCK_EXTERNAL_REQUESTS_ARGS] },
              }),
              headless: true,
              instances: [{ browser: 'chromium' }],
            },
            setupFiles: ['./.storybook/vitest.setup.ts'],
          },
        },
      ],
    },
  }),
)
