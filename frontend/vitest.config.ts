import { defineConfig, mergeConfig } from 'vitest/config'

import viteConfig from './vite.config'

<<<<<<< before updating
export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: 'jsdom',
      setupFiles: ['./src/test-setup.ts'],
    },
  }),
)
||||||| last update
export default mergeConfig({
  resolve: {
    alias: {
      '@': '/src',
    },
  },
})
=======
export default mergeConfig({})
>>>>>>> after updating
