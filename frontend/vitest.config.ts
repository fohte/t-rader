import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { BLOCK_EXTERNAL_REQUESTS_ARGS } from '@fohte/storybook-addon/vitest-plugin'
import { storybookTest } from '@storybook/addon-vitest/vitest-plugin'
import { playwright } from '@vitest/browser-playwright'
import { defineConfig, mergeConfig } from 'vitest/config'
import type { BrowserCommand } from 'vitest/node'

import viteConfig from './vite.config'

const dirname = path.dirname(fileURLToPath(import.meta.url))

const tapLinkOnTouch: BrowserCommand<[selector: string]> = async (
  context,
  selector,
) => {
  if (context.provider.name !== 'playwright') {
    return Promise.reject(new Error('Touch link tests require Playwright'))
  }

  const link = (await context.frame()).locator(selector)
  const position = await link.evaluate((element) => {
    const bounds = element.getBoundingClientRect()
    const firstRect = element.getClientRects().item(0)
    if (firstRect == null) return null

    return {
      x: firstRect.left - bounds.left + firstRect.width / 2,
      y: firstRect.top - bounds.top + firstRect.height / 2,
    }
  })
  if (position == null) {
    return Promise.reject(new Error('The touch target has no visible bounds'))
  }

  await link.tap({ position })
}

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
            exclude: [
              'src/components/note-detail/markdown-body-links.browser.test.tsx',
            ],
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
            include: [
              'src/components/note-detail/markdown-body-links.browser.test.tsx',
            ],
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
