import { i18n, t } from '$lib/i18n.svelte';

export function messageOf(cause: unknown): string {
  return cause instanceof Error ? t(cause.message) : t('An unexpected error occurred');
}

export function relativeDate(value: string | null): string {
  if (!value) return t('Never');
  const seconds = Math.floor((Date.now() - new Date(value).getTime()) / 1000);
  if (seconds < 60) return t('Just now');
  if (seconds < 3600) return t('{count}m ago', { count: Math.floor(seconds / 60) });
  if (seconds < 86400) return t('{count}h ago', { count: Math.floor(seconds / 3600) });
  return new Intl.DateTimeFormat(i18n.locale, { month: 'short', day: 'numeric' }).format(
    new Date(value),
  );
}

export function shortId(value: string): string {
  return value.length > 16 ? `${value.slice(0, 8)}…${value.slice(-4)}` : value;
}
