/** nodeTypes.graphScatterBackground。象限区切りの十字線を引くだけの装飾ノード */
export function GraphScatterBackgroundView() {
  return (
    <div className="border-border relative h-full w-full rounded-md border">
      <div className="bg-border absolute top-1/2 left-0 h-px w-full" />
      <div className="bg-border absolute top-0 left-1/2 h-full w-px" />
    </div>
  )
}
