import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/** The `owner/name` a GitHub remote URL names, or null if it doesn't look like one. */
export function repositorySlug(remoteUrl: string | null): string | null {
  if (!remoteUrl) return null
  const match = remoteUrl.match(/^.*[:/]([^/]+\/[^/]+?)(\.git)?$/)
  return match ? match[1] : null
}
