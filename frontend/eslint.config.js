import { config } from '@fohte/eslint-config'

<<<<<<< before updating
const config = [
  { ignores: ['dist', 'vitest.config.ts'] },
  ...mainConfig,
  ...typescriptConfig,
=======
export default config(
  { typescript: { typeChecked: true } },
>>>>>>> after updating
  {
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
  },
)
