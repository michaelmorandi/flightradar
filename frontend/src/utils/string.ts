/**
 * Truncates a string to a specified length and adds ellipsis if needed
 */
export function truncate(str: string | undefined | null, maxLength: number): string {
  if (!str) return '';
  if (str.length <= maxLength) return str;
  return str.slice(0, maxLength - 3) + '...';
}