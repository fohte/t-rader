import '@testing-library/jest-dom/vitest'

// jsdom の Request コンストラクタは相対 URL を受け付けない。
// 本番 (ブラウザ) では `/api/...` 相対パスをそのまま `new Request()` に渡しても
// document.baseURI 基準で解決されるが、テストでは事前に絶対 URL 化する必要がある。
const OriginalRequest = globalThis.Request
class PatchedRequest extends OriginalRequest {
  constructor(input: RequestInfo | URL, init?: RequestInit) {
    if (typeof input === 'string' && input.startsWith('/')) {
      super(new URL(input, window.location.origin).toString(), init)
    } else {
      super(input, init)
    }
  }
}
globalThis.Request = PatchedRequest
