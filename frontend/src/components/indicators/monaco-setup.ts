import { loader } from '@monaco-editor/react'
import * as monaco from 'monaco-editor'
import editorWorker from 'monaco-editor/editor/editor.worker?worker'
import jsonWorker from 'monaco-editor/language/json/json.worker?worker'

// @monaco-editor/react は既定では起動時に jsdelivr の CDN から Monaco 本体を
// script タグ注入で読み込む。ローカルにバンドルした monaco-editor を渡すことで
// CDN 依存を断ち、CI やオフライン環境でも動作するようにする
self.MonacoEnvironment = {
  getWorker(_workerId, label) {
    if (label === 'json') {
      return new jsonWorker()
    }
    return new editorWorker()
  },
}

loader.config({ monaco })
