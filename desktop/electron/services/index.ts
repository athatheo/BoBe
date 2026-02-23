/**
 * Service exports
 */

export { daemonClient } from './daemon-client'
export { backendService } from './rust-service'
export { ollamaService } from './ollama-service'
export {
  runSetupWizard,
  checkOnboardingStatus,
  ensureEncryptionKey,
  getEncryptionKey,
  hasEncryptionKey,
} from './setup-service'
export {
  createOverlayWindow,
  getOverlayWindow,
  toggleOverlayVisibility,
  isOverlayVisible,
  resizeForBubble,
  resizeWindow,
} from './window-manager'
export { createTray, updateTrayMenu, getTray } from './tray-manager'
export { openSettingsWindow, closeSettingsWindow } from './settings-window'
