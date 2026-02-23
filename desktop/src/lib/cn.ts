/**
 * Tailwind class name composition utility
 */

import { type ClassValue, clsx } from 'clsx'
import { twMerge } from 'tailwind-merge'

/**
 * Merges Tailwind CSS classes intelligently, handling conflicts properly.
 * Always use this for className composition instead of template literals.
 *
 * @example
 * cn('base-class', isActive && 'active-class', className)
 */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}
