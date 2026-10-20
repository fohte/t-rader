import type { Meta, StoryObj } from '@storybook/react-vite'
import { useState } from 'react'

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

const SAMPLE_YAML = `phases:
  - key: plan
    label: 見出し案
    model: fictional-model-orbit
    prompt: |
      与えられたテーマを読みやすい見出しに分ける。
`

function Interactive({
  language,
  initial,
}: {
  language: 'python' | 'json' | 'yaml'
  initial: string
}) {
  const [value, setValue] = useState(initial)
  return (
    <CodeEditor
      language={language}
      value={value}
      onChange={setValue}
      ariaLabel="story editor"
    />
  )
}

export const Python: Story = {
  name: 'shows a Python text-processing example in the code editor.',
  args: { language: 'python', value: SAMPLE_PYTHON, onChange: () => {} },
  render: () => <Interactive language="python" initial={SAMPLE_PYTHON} />,
}

export const Json: Story = {
  name: 'shows JSON content in the code editor.',
  args: { language: 'json', value: SAMPLE_JSON, onChange: () => {} },
  render: () => <Interactive language="json" initial={SAMPLE_JSON} />,
}

export const Yaml: Story = {
  name: 'shows YAML content in the code editor.',
  args: { language: 'yaml', value: SAMPLE_YAML, onChange: () => {} },
  render: () => <Interactive language="yaml" initial={SAMPLE_YAML} />,
}
