/**
 * Monaco Editor setup for Electron.
 *
 * CRITICAL: Monaco must load from local node_modules, NOT from CDN.
 * Electron's CSP blocks external script loading (jsdelivr.net).
 * Call initMonacoLoader() once at app startup before any Editor mounts.
 */

import type { Monaco } from '@monaco-editor/react'
import { loader } from '@monaco-editor/react'
import * as monaco from 'monaco-editor'

let configured = false
let loaderInitialized = false

// Auto-initialize loader when this module is imported.
// This MUST run before any <Editor> component mounts, otherwise
// @monaco-editor/react tries to load from CDN, which CSP blocks.
// Since all Monaco-using components import from this module,
// calling it here guarantees correct timing.
initMonacoLoader()

/**
 * Initialize Monaco to use local copy instead of CDN.
 * Must be called once before any Editor component mounts.
 */
export function initMonacoLoader(): void {
  if (loaderInitialized) return
  loaderInitialized = true

  // Suppress Monaco worker errors in Electron's sandboxed renderer.
  // Monaco's internal createWebWorker bypasses our getWorker override,
  // so we intercept errors at the window level to prevent console spam.
  // Monaco worker errors only come from Monaco's bundled chunks.
  // We check both the error message AND source to avoid suppressing
  // unrelated errors (CWE-20: validate message origin).
  const originalOnError = window.onerror
  window.onerror = (message, source, lineno, colno, error) => {
    const msg = String(message)
    const src = String(source || '')
    const isMonacoWorkerError =
      (src.includes('chunk-') || src.includes('monaco')) &&
      (msg.includes('postMessage') || msg.includes('FAILED to post message to worker'))
    if (isMonacoWorkerError) return true
    return originalOnError
      ? originalOnError.call(window, message, source, lineno, colno, error)
      : false
  }

  // Catch unhandled promise rejections from Monaco worker disposal.
  // These originate from Monaco's internal dispose() chain — safe to suppress.
  window.addEventListener('unhandledrejection', (event) => {
    const reason = event.reason
    if (reason instanceof TypeError) {
      const msg = reason.message
      if (msg.includes('onmessage') || msg.includes('postMessage')) {
        event.preventDefault()
      }
    }
  })

  loader.config({ monaco })

  // Disable Monaco web workers — Monaco falls back to sync mode on main thread
  ;(self as unknown as Record<string, unknown>).MonacoEnvironment = {
    getWorker() {
      return null
    },
  }
}

export function configureMonaco(monaco: Monaco): void {
  // Ensure loader is initialized (deferred from app startup)
  initMonacoLoader()
  if (configured) return
  configured = true

  // ---------------------------------------------------------------------------
  // Markdown completions
  // ---------------------------------------------------------------------------
  monaco.languages.registerCompletionItemProvider('markdown', {
    triggerCharacters: ['#', '*', '-', '[', '`', '!', '>'],
    provideCompletionItems(
      model: Parameters<monaco.languages.CompletionItemProvider['provideCompletionItems']>[0],
      position: Parameters<monaco.languages.CompletionItemProvider['provideCompletionItems']>[1],
    ) {
      const word = model.getWordUntilPosition(position)
      const range = {
        startLineNumber: position.lineNumber,
        endLineNumber: position.lineNumber,
        startColumn: word.startColumn,
        endColumn: word.endColumn,
      }

      const suggestions: monaco.languages.CompletionItem[] = [
        // Headers
        {
          label: '# Heading 1',
          kind: monaco.languages.CompletionItemKind.Snippet,
          insertText: '# ${1:Heading}\n$0',
          insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
          detail: 'H1 heading',
          range,
        },
        {
          label: '## Heading 2',
          kind: monaco.languages.CompletionItemKind.Snippet,
          insertText: '## ${1:Heading}\n$0',
          insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
          detail: 'H2 heading',
          range,
        },
        {
          label: '### Heading 3',
          kind: monaco.languages.CompletionItemKind.Snippet,
          insertText: '### ${1:Heading}\n$0',
          insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
          detail: 'H3 heading',
          range,
        },

        // Formatting
        {
          label: 'Bold',
          kind: monaco.languages.CompletionItemKind.Snippet,
          insertText: '**${1:text}**$0',
          insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
          detail: 'Bold text',
          range,
        },
        {
          label: 'Italic',
          kind: monaco.languages.CompletionItemKind.Snippet,
          insertText: '*${1:text}*$0',
          insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
          detail: 'Italic text',
          range,
        },
        {
          label: 'Strikethrough',
          kind: monaco.languages.CompletionItemKind.Snippet,
          insertText: '~~${1:text}~~$0',
          insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
          detail: 'Strikethrough',
          range,
        },
        {
          label: 'Inline code',
          kind: monaco.languages.CompletionItemKind.Snippet,
          insertText: '`${1:code}`$0',
          insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
          detail: 'Inline code',
          range,
        },

        // Blocks
        {
          label: 'Code block',
          kind: monaco.languages.CompletionItemKind.Snippet,
          insertText: '```${1:language}\n${2:code}\n```\n$0',
          insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
          detail: 'Fenced code block',
          range,
        },
        {
          label: 'Blockquote',
          kind: monaco.languages.CompletionItemKind.Snippet,
          insertText: '> ${1:quote}\n$0',
          insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
          detail: 'Blockquote',
          range,
        },

        // Lists
        {
          label: 'Bullet list',
          kind: monaco.languages.CompletionItemKind.Snippet,
          insertText: '- ${1:item}\n- ${2:item}\n- ${3:item}\n$0',
          insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
          detail: 'Unordered list',
          range,
        },
        {
          label: 'Numbered list',
          kind: monaco.languages.CompletionItemKind.Snippet,
          insertText: '1. ${1:item}\n2. ${2:item}\n3. ${3:item}\n$0',
          insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
          detail: 'Ordered list',
          range,
        },
        {
          label: 'Task list',
          kind: monaco.languages.CompletionItemKind.Snippet,
          insertText: '- [ ] ${1:task}\n- [ ] ${2:task}\n$0',
          insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
          detail: 'Task/checkbox list',
          range,
        },

        // Links & media
        {
          label: 'Link',
          kind: monaco.languages.CompletionItemKind.Snippet,
          insertText: '[${1:text}](${2:url})$0',
          insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
          detail: 'Hyperlink',
          range,
        },
        {
          label: 'Image',
          kind: monaco.languages.CompletionItemKind.Snippet,
          insertText: '![${1:alt}](${2:url})$0',
          insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
          detail: 'Image',
          range,
        },

        // Structure
        {
          label: 'Horizontal rule',
          kind: monaco.languages.CompletionItemKind.Snippet,
          insertText: '\n---\n$0',
          insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
          detail: 'Horizontal rule',
          range,
        },
        {
          label: 'Table',
          kind: monaco.languages.CompletionItemKind.Snippet,
          insertText: '| ${1:Header} | ${2:Header} |\n| --- | --- |\n| ${3:Cell} | ${4:Cell} |\n$0',
          insertTextRules: monaco.languages.CompletionItemInsertTextRule.InsertAsSnippet,
          detail: 'Table',
          range,
        },
      ]

      return { suggestions }
    },
  })

  // ---------------------------------------------------------------------------
  // JSON diagnostics — already built-in, just ensure validation is on
  // ---------------------------------------------------------------------------
  monaco.languages.json.jsonDefaults.setDiagnosticsOptions({
    validate: true,
    allowComments: false,
    trailingCommas: 'error',
    schemaValidation: 'error',
  })
}
