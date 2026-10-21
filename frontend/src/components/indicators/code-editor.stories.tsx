import type { Meta, StoryObj } from '@storybook/react-vite'

import { CodeEditor } from '#components/indicators/code-editor'

const meta = {
  title: 'Indicators/CodeEditor',
  component: CodeEditor,
} satisfies Meta<typeof CodeEditor>

export default meta
type Story = StoryObj<typeof meta>

const SAMPLE_PYTHON = `import json, sys

args = json.load(sys.stdin)["args"]
label = args.get("label", "")
print(json.dumps({"characters": len(label)}))
`

const SAMPLE_JSON = JSON.stringify(
  {
    type: 'object',
    properties: { label: { type: 'string' } },
    required: ['label'],
  },
  null,
  2,
)

const SAMPLE_YAML = `settings:
  label: example
  enabled: true
`

export const Python: Story = {
  name: 'shows a Python text-processing example in the code editor.',
  args: { language: 'python', value: SAMPLE_PYTHON, onChange: () => {} },
}

export const Json: Story = {
  name: 'shows JSON content in the code editor.',
  args: { language: 'json', value: SAMPLE_JSON, onChange: () => {} },
}

export const Yaml: Story = {
  name: 'shows YAML content in the code editor.',
  args: { language: 'yaml', value: SAMPLE_YAML, onChange: () => {} },
}
