/**
 * Privacy & Data settings
 *
 * Shows data storage info and provides "Delete all data" action.
 */

import { useState, useEffect, useCallback } from 'react'
import { Trash2, HardDrive, AlertTriangle } from 'lucide-react'

interface DataSize {
  totalMB: number
  breakdown: Record<string, number>
}

const DIR_LABELS: Record<string, string> = {
  models: 'AI Models',
  data: 'Database',
  logs: 'Logs',
  ollama: 'Ollama Engine',
}

function formatSize(mb: number): string {
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`
  if (mb >= 1) return `${mb.toFixed(1)} MB`
  return `${(mb * 1024).toFixed(0)} KB`
}

export function PrivacySettings() {
  const [dataSize, setDataSize] = useState<DataSize | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    window.appData
      .getDataSize()
      .then(setDataSize)
      .catch(() => setDataSize(null))
      .finally(() => setLoading(false))
  }, [])

  const handleDeleteAll = useCallback(() => {
    window.appData.deleteAllData()
    // The main process shows a native dialog and handles the rest.
    // If confirmed, the app quits. If cancelled, nothing happens.
  }, [])

  return (
    <div className="preferences-panel" style={{ padding: '1.5rem' }}>
      {/* Data storage section */}
      <div style={{ marginBottom: '2rem' }}>
        <h3
          style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', marginBottom: '0.25rem' }}
        >
          <HardDrive size={18} />
          Local Storage
        </h3>
        <p style={{ fontSize: '0.85rem', color: 'var(--text-muted)', marginBottom: '1rem' }}>
          All your data is stored locally on this Mac. Nothing is sent to the cloud.
        </p>

        {loading ? (
          <p style={{ color: 'var(--text-muted)', fontSize: '0.85rem' }}>Calculating...</p>
        ) : dataSize ? (
          <div
            style={{
              background: 'var(--surface, #F5F0EA)',
              borderRadius: '0.75rem',
              padding: '1rem',
              border: '1px solid var(--border, #E8DCC4)',
            }}
          >
            <div
              style={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'baseline',
                marginBottom: '0.75rem',
              }}
            >
              <span style={{ fontWeight: 600 }}>Total</span>
              <span style={{ fontWeight: 600, fontSize: '1.1rem' }}>
                {formatSize(dataSize.totalMB)}
              </span>
            </div>
            {Object.entries(dataSize.breakdown)
              .filter(([, mb]) => mb > 0)
              .sort(([, a], [, b]) => b - a)
              .map(([dir, mb]) => (
                <div
                  key={dir}
                  style={{
                    display: 'flex',
                    justifyContent: 'space-between',
                    fontSize: '0.85rem',
                    padding: '0.25rem 0',
                    color: 'var(--text-muted)',
                  }}
                >
                  <span>{DIR_LABELS[dir] || dir}</span>
                  <span>{formatSize(mb)}</span>
                </div>
              ))}
          </div>
        ) : (
          <p style={{ color: 'var(--text-muted)', fontSize: '0.85rem' }}>
            Unable to calculate data size.
          </p>
        )}
      </div>

      {/* Danger zone */}
      <div
        style={{
          borderTop: '1px solid var(--border, #E8DCC4)',
          paddingTop: '1.5rem',
        }}
      >
        <h3
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '0.5rem',
            marginBottom: '0.25rem',
            color: '#C62828',
          }}
        >
          <AlertTriangle size={18} />
          Danger Zone
        </h3>
        <p style={{ fontSize: '0.85rem', color: 'var(--text-muted)', marginBottom: '1rem' }}>
          Permanently delete all BoBe data from this Mac. This removes your database, downloaded
          models, and configuration. BoBe will quit and you&apos;ll need to set up again on next
          launch.
        </p>

        <button
          onClick={handleDeleteAll}
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '0.5rem',
            padding: '0.6rem 1.25rem',
            borderRadius: '0.5rem',
            border: '1px solid #EF9A9A',
            background: '#FFEBEE',
            color: '#C62828',
            fontSize: '0.9rem',
            fontWeight: 500,
            cursor: 'pointer',
          }}
        >
          <Trash2 size={16} />
          Delete all data
        </button>
      </div>
    </div>
  )
}
