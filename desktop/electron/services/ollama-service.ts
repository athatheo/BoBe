/**
 * OllamaService - Manages Ollama binary presence for the bundled app
 *
 * Simplified responsibility: only ensures the Ollama binary exists.
 * The Python service manages the actual Ollama lifecycle via ollama_manager.py
 * (BOBE_OLLAMA_AUTO_START=true).
 *
 * Responsibilities:
 * - Check if Ollama binary exists at ~/Library/Application Support/BoBe/ollama/bin/ollama
 * - Download + verify if missing (first-run setup)
 * - Report download progress via events
 */

import { app } from 'electron'
import { createWriteStream, existsSync, mkdirSync, chmodSync, rmSync } from 'node:fs'
import { pipeline } from 'node:stream/promises'
import { execFile } from 'node:child_process'
import { promisify } from 'node:util'
import path from 'node:path'
import { createHash, timingSafeEqual } from 'node:crypto'
import { readFile } from 'node:fs/promises'
import { EventEmitter } from 'node:events'

const execFileAsync = promisify(execFile)

// Pin Ollama version + checksums for supply chain security
const OLLAMA_VERSION = '0.6.2'
const OLLAMA_SHA256: Record<string, string> = {
  'ollama-darwin.tgz': '5877492693ee3673245427014e2d78cc3db1dccc80e6c145d74cf12f342cd904',
  'ollama-linux-amd64.tgz': 'd59967750335233d2c116acae63d7927f0b91409aba47eb8636d8f990ad46bd1',
  'ollama-linux-arm64.tgz': 'e956f6bf58224729e487e71acc0059150105869ab117a412461f8ad59d9d9963',
}

function getOllamaFilename(): string {
  switch (process.platform) {
    case 'darwin':
      // macOS ships a universal binary (no arch suffix)
      return 'ollama-darwin.tgz'
    case 'win32':
      return `ollama-windows-${process.arch === 'x64' ? 'amd64' : 'arm64'}.zip`
    case 'linux':
      return `ollama-linux-${process.arch === 'x64' ? 'amd64' : 'arm64'}.tgz`
    default:
      throw new Error(`Unsupported platform: ${process.platform}`)
  }
}

function getOllamaDownloadUrl(): string {
  const base = `https://github.com/ollama/ollama/releases/download/v${OLLAMA_VERSION}`
  return `${base}/${getOllamaFilename()}`
}

export interface OllamaServiceEvents {
  'download-start': []
  'download-progress': [percent: number, downloadedMB: number, totalMB: number]
  'download-complete': []
  'download-error': [error: Error]
  ready: []
}

export class OllamaService extends EventEmitter<OllamaServiceEvents> {
  private ollamaDir: string
  private ollamaBinPath: string

  constructor() {
    super()
    this.ollamaDir = path.join(app.getPath('userData'), 'ollama')
    this.ollamaBinPath = path.join(this.ollamaDir, 'bin', 'ollama')
  }

  /** Check if Ollama binary is installed */
  isInstalled(): boolean {
    return existsSync(this.ollamaBinPath)
  }

  /** Get the path to the Ollama binary */
  getBinPath(): string {
    return this.ollamaBinPath
  }

  /** Ensure Ollama is installed, downloading if necessary */
  async ensureInstalled(): Promise<void> {
    if (this.isInstalled()) {
      console.log('[OllamaService] Ollama binary found')
      this.emit('ready')
      return
    }

    console.log('[OllamaService] Ollama not found, downloading...')
    await this.download()
    this.emit('ready')
  }

  /** Download and install the Ollama binary */
  private async download(): Promise<void> {
    const tgzPath = path.join(this.ollamaDir, 'ollama.tgz')
    const binDir = path.join(this.ollamaDir, 'bin')

    mkdirSync(binDir, { recursive: true })
    this.emit('download-start')

    try {
      // Download
      const downloadUrl = getOllamaDownloadUrl()
      console.log(`[OllamaService] Downloading from ${downloadUrl}`)
      const response = await fetch(downloadUrl)
      if (!response.ok || !response.body) {
        throw new Error(`Download failed: ${response.status} ${response.statusText}`)
      }

      const totalBytes = parseInt(response.headers.get('content-length') || '0', 10)
      let downloadedBytes = 0
      let lastEmittedPercent = -10

      // Create a transform stream to track and report progress
      const { Readable } = await import('node:stream')
      const reader = response.body.getReader()
      const emitter = this // Capture OllamaService for progress emission
      const progressStream = new Readable({
        async read() {
          const { done, value } = await reader.read()
          if (done) {
            this.push(null)
            return
          }
          downloadedBytes += value.length
          this.push(Buffer.from(value))

          // Emit progress every ~5% to avoid flooding
          if (totalBytes > 0) {
            const percent = Math.round((downloadedBytes / totalBytes) * 100)
            if (percent >= lastEmittedPercent + 5) {
              lastEmittedPercent = percent
              emitter.emit('download-progress', percent, downloadedBytes / 1e6, totalBytes / 1e6)
            }
          }
        },
      })

      const fileStream = createWriteStream(tgzPath)
      await pipeline(progressStream, fileStream)

      this.emit('download-progress', 100, totalBytes / 1e6, totalBytes / 1e6)

      // Verify checksum
      const expectedHash = OLLAMA_SHA256[getOllamaFilename()]
      if (expectedHash) {
        console.log('[OllamaService] Verifying SHA256...')
        const fileBuffer = await readFile(tgzPath)
        const hashBuffer = createHash('sha256').update(fileBuffer).digest()
        const expectedBuffer = Buffer.from(expectedHash, 'hex')
        if (
          hashBuffer.length !== expectedBuffer.length ||
          !timingSafeEqual(hashBuffer, expectedBuffer)
        ) {
          throw new Error('Checksum mismatch: downloaded file integrity check failed')
        }
        console.log('[OllamaService] Checksum verified')
      } else {
        console.warn('[OllamaService] No pinned checksum for this platform, skipping verification')
      }

      // Extract using execFile (no shell injection risk)
      console.log('[OllamaService] Extracting...')
      await execFileAsync('tar', ['-xzf', tgzPath, '-C', binDir])

      // Make executable
      chmodSync(this.ollamaBinPath, 0o755)

      // Clean up tarball
      rmSync(tgzPath, { force: true })

      console.log('[OllamaService] Ollama installed successfully')
      this.emit('download-complete')
    } catch (error) {
      rmSync(tgzPath, { force: true })
      const err = error instanceof Error ? error : new Error(String(error))
      console.error('[OllamaService] Download failed:', err)
      this.emit('download-error', err)
      throw err
    }
  }
}

export const ollamaService = new OllamaService()
