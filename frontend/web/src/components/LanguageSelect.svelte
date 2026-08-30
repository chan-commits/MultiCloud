<script lang="ts">
  import { i18n, setLocale, t, type Locale } from '$lib/i18n.svelte';

  let { compact = false }: { compact?: boolean } = $props();
</script>

<label class="flex items-center gap-2 text-[9px] text-muted">
  <span class={compact ? 'sr-only' : ''}>{t('Language')}</span>
  <select
    class="rounded-[5px] border border-[#203140] bg-[#09111a] px-[9px] py-2 text-[10px] text-[#dcecf4] outline-none"
    aria-label={t('Language')}
    title={t('Uses your browser language by default; unsupported languages use English.')}
    value={i18n.locale}
    onchange={(event) => setLocale(event.currentTarget.value as Locale)}
  >
    <option value="en">English</option>
    <option value="zh-CN">简体中文</option>
    <option value="zh-TW">繁體中文</option>
  </select>
</label>
