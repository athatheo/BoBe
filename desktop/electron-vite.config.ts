/**
 * electron-vite configuration
 *
 * Default behavior (electron-vite 5+):
 * - Main & preload: dependencies are externalized (not bundled)
 * - Renderer: dependencies are bundled
 * - Node.js built-ins and electron are always external
 */

import { defineConfig } from 'electron-vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { resolve } from 'path'

export default defineConfig({
  // Main process configuration
  main: {
    build: {
      outDir: 'out/main',
      lib: {
        entry: resolve(__dirname, 'electron/main/index.ts'),
        formats: ['cjs'],
      },
      rollupOptions: {
        output: {
          entryFileNames: 'index.js',
        },
      },
    },
  },

  // Preload script configuration
  preload: {
    build: {
      outDir: 'out/preload',
      lib: {
        entry: resolve(__dirname, 'electron/preload/index.ts'),
        formats: ['cjs'],
      },
      rollupOptions: {
        output: {
          entryFileNames: 'index.js',
        },
      },
    },
  },

  // Renderer (React app) configuration
  renderer: {
    root: resolve(__dirname, 'src'),
    server: {
      port: 5175,
      strictPort: true,
    },
    build: {
      outDir: resolve(__dirname, 'out/renderer'),
      rollupOptions: {
        input: {
          main: resolve(__dirname, 'src/index.html'),
          settings: resolve(__dirname, 'src/settings.html'),
          setup: resolve(__dirname, 'src/setup.html'),
        },
      },
    },
    plugins: [react(), tailwindcss()],
    resolve: {
      alias: {
        '@': resolve(__dirname, 'src'),
      },
    },
  },
})
