/**
 * Settings UI types
 *
 * Types for Settings UI configuration (themes, categories).
 * Backend API types are now in api.ts.
 */

// =============================================================================
// SETTINGS UI TYPES
// =============================================================================

/**
 * Settings navigation category
 */
export type SettingsCategory =
  | 'souls'
  | 'goals'
  | 'memories'
  | 'user-profiles'
  | 'tools'
  | 'mcp-servers'
  | 'appearance'
  | 'ai-model'
  | 'behavior'
  | 'goal-worker'
  | 'privacy'
  | 'advanced'

/**
 * Category display configuration
 */
export interface CategoryConfig {
  id: SettingsCategory
  label: string
  icon: string
  description: string
  group: 'context' | 'integrations' | 'preferences' | 'advanced'
}

/**
 * All category configurations
 */
export const SETTINGS_CATEGORIES: CategoryConfig[] = [
  // Context - What BoBe knows
  {
    id: 'souls',
    label: 'Souls',
    icon: 'sparkles',
    description: 'AI personality and behavior definitions',
    group: 'context',
  },
  {
    id: 'goals',
    label: 'Goals',
    icon: 'target',
    description: 'User objectives and priorities',
    group: 'context',
  },
  {
    id: 'memories',
    label: 'Memories',
    icon: 'brain',
    description: 'What BoBe remembers about you',
    group: 'context',
  },
  {
    id: 'user-profiles',
    label: 'User Profiles',
    icon: 'user',
    description: 'Information about you (expertise, preferences)',
    group: 'context',
  },
  // Integrations - What BoBe connects to
  {
    id: 'tools',
    label: 'Tools',
    icon: 'wrench',
    description: 'Actions BoBe can take on your behalf',
    group: 'integrations',
  },
  {
    id: 'mcp-servers',
    label: 'MCP Servers',
    icon: 'server',
    description: 'Model Context Protocol integrations',
    group: 'integrations',
  },
  // Preferences - How BoBe behaves
  {
    id: 'appearance',
    label: 'Appearance',
    icon: 'palette',
    description: 'Theme, colors, and avatar customization',
    group: 'preferences',
  },
  {
    id: 'ai-model',
    label: 'AI Model',
    icon: 'cpu',
    description: 'Language model provider and model management',
    group: 'preferences',
  },
  {
    id: 'behavior',
    label: 'Behavior',
    icon: 'sliders',
    description: 'Screen capture, check-ins, and engagement settings',
    group: 'preferences',
  },
  {
    id: 'goal-worker',
    label: 'Goal Worker',
    icon: 'play-circle',
    description: 'Autonomous goal execution and planning',
    group: 'preferences',
  },
  {
    id: 'privacy',
    label: 'Privacy',
    icon: 'shield',
    description: 'Data handling and telemetry',
    group: 'preferences',
  },
  // Advanced - For power users
  {
    id: 'advanced',
    label: 'For Nerds',
    icon: 'terminal',
    description: 'Daemon configuration and developer settings',
    group: 'advanced',
  },
]

// =============================================================================
// THEME TYPES
// =============================================================================

/**
 * Available themes
 */
export type ThemeId =
  | 'bauhaus'
  | 'bauhaus-pastel'
  | 'cute'
  | 'cute-pastel'
  | 'bauhaus-dark'
  | 'cute-dark'

/**
 * Theme configuration
 */
export interface ThemeConfig {
  id: ThemeId
  name: string
  description: string
  isDark: boolean
  colors: {
    primary: string // Main accent (terracotta, pink, etc.)
    secondary: string // Secondary accent (olive, mint, etc.)
    tertiary: string // Tertiary (clay, lavender, etc.)
    background: string // Main background
    surface: string // Card/surface background
    border: string // Border color
    text: string // Primary text
    textMuted: string // Secondary text
    avatarFaceLight: string // Avatar gradient light color
    avatarFaceDark: string // Avatar gradient dark color
    avatarRing: string // Avatar outer ring
    avatarIris: string // Iris color (colored ring around pupil)
    avatarEyeOutline: string // Eye outline color (for contrast with face)
    avatarMouth: string // Mouth/lips color
  }
}

/**
 * All available themes
 */
export const THEMES: ThemeConfig[] = [
  // Light themes
  {
    id: 'bauhaus',
    name: 'Terracotta Dreams',
    description: 'Warm earthy tones',
    isDark: false,
    colors: {
      primary: '#C67B5C', // Terracotta
      secondary: '#8B9A7D', // Olive
      tertiary: '#A69080', // Clay
      background: '#FAF7F2', // Warm white
      surface: '#FAF7F2',
      border: '#E8DCC4', // Sand
      text: '#3A3A3A', // Charcoal
      textMuted: '#6B6B6B',
      avatarFaceLight: '#E8DCC4', // Sand (gradient top)
      avatarFaceDark: '#B8A99A', // Warm taupe (gradient bottom)
      avatarRing: '#FAF7F2',
      avatarIris: '#A69080', // Clay iris
      avatarEyeOutline: 'white', // White outline for contrast
      avatarMouth: '#C67B5C', // Terracotta mouth
    },
  },
  {
    id: 'bauhaus-pastel',
    name: 'Soft Clay',
    description: 'Gentle muted warmth',
    isDark: false,
    colors: {
      primary: '#D4A59A', // Soft terracotta
      secondary: '#A8B89F', // Soft olive
      tertiary: '#C4B5A9', // Soft clay
      background: '#FDFBF8', // Lighter warm white
      surface: '#FDFBF8',
      border: '#EDE5D8', // Lighter sand
      text: '#4A4A4A',
      textMuted: '#7A7A7A',
      avatarFaceLight: '#EDE5D8', // Light sand
      avatarFaceDark: '#C9BAA9', // Lighter taupe
      avatarRing: '#FDFBF8',
      avatarIris: '#B8A99A', // Soft taupe iris
      avatarEyeOutline: 'white', // White outline for contrast
      avatarMouth: '#D4A59A', // Soft terracotta mouth
    },
  },
  {
    id: 'cute',
    name: 'Bubblegum',
    description: 'Playful pink vibes',
    isDark: false,
    colors: {
      primary: '#E8879C', // Rose pink
      secondary: '#7DBDA8', // Mint green
      tertiary: '#B8A4D4', // Lavender
      background: '#FFF8FA', // Pink-tinted white
      surface: '#FFF8FA',
      border: '#F5E0E5', // Light pink
      text: '#3D3D3D',
      textMuted: '#6D6D6D',
      avatarFaceLight: '#FFD4DC', // Pink face
      avatarFaceDark: '#FFBAC8', // Darker pink
      avatarRing: '#FFF8FA',
      avatarIris: '#E8879C', // Pink iris
      avatarEyeOutline: 'white', // White outline
      avatarMouth: '#E8879C', // Pink mouth
    },
  },
  {
    id: 'cute-pastel',
    name: 'Cotton Candy',
    description: 'Dreamy soft pastels',
    isDark: false,
    colors: {
      primary: '#F2A6B4', // Soft pink
      secondary: '#A8D5C2', // Soft mint
      tertiary: '#D4C4E8', // Soft lavender
      background: '#FFFCFD', // Very light pink
      surface: '#FFFCFD',
      border: '#F8E8EC', // Very light pink border
      text: '#4D4D4D',
      textMuted: '#7D7D7D',
      avatarFaceLight: '#D8C4E8', // Medium pastel purple
      avatarFaceDark: '#C4B0D8', // Darker pastel purple
      avatarRing: '#FFFCFD',
      avatarIris: '#B8A4D4', // Lavender iris
      avatarEyeOutline: 'white', // White outline
      avatarMouth: '#9A7AAA', // Darker purple mouth
    },
  },
  // Dark themes
  {
    id: 'bauhaus-dark',
    name: 'Midnight Clay',
    description: 'Warm tones in the dark',
    isDark: true,
    colors: {
      primary: '#D4926F', // Lighter terracotta for contrast
      secondary: '#9AAD8E', // Lighter olive
      tertiary: '#B8A090', // Lighter clay
      background: '#1E1E1E', // Dark charcoal
      surface: '#2A2A2A', // Slightly lighter
      border: '#3D3D3D', // Dark border
      text: '#E8E4DF', // Light warm text
      textMuted: '#9A9590',
      avatarFaceLight: '#A89070', // Medium brown
      avatarFaceDark: '#8A7560', // Darker brown
      avatarRing: '#2A2A2A',
      avatarIris: '#3A3A3A', // Dark iris
      avatarEyeOutline: 'white', // White outline
      avatarMouth: '#3A3A3A', // Dark mouth
    },
  },
  {
    id: 'cute-dark',
    name: 'Twilight Rose',
    description: 'Soft pink in the dark',
    isDark: true,
    colors: {
      primary: '#F2A0B0', // Lighter pink for contrast
      secondary: '#8DCAB5', // Lighter mint
      tertiary: '#C4B4E0', // Lighter lavender
      background: '#1A1A1E', // Dark with slight purple
      surface: '#252528', // Slightly lighter
      border: '#3A3A40', // Dark border
      text: '#F5F0F2', // Light pink-tinted text
      textMuted: '#9A9598',
      avatarFaceLight: '#C89098', // Darkish pastel rose
      avatarFaceDark: '#A87080', // Darker rose
      avatarRing: '#252528',
      avatarIris: '#3A3A3A', // Dark iris
      avatarEyeOutline: 'white', // White outline
      avatarMouth: '#8A4050', // Very dark pink mouth
    },
  },
]

/**
 * Get theme by ID
 */
export function getThemeById(id: ThemeId): ThemeConfig {
  return THEMES.find((t) => t.id === id) ?? THEMES[0]!
}
