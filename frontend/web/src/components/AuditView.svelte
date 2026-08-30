<script lang="ts">
  import type { AuditLog } from '../lib/api';
  import { statusClass } from '../lib/ui';
  import { t } from '$lib/i18n.svelte';
  let {
    auditLogs,
    auditAction,
    auditOutcome,
    auditLoadingMore,
    auditHasMore,
    actionBusy,
    onActionChange,
    onOutcomeChange,
    onApplyFilters,
    onLoadMore,
    onExport,
    relativeDate,
    shortId,
  }: {
    auditLogs: AuditLog[];
    auditAction: string;
    auditOutcome: string;
    auditLoadingMore: boolean;
    auditHasMore: boolean;
    actionBusy: string;
    onActionChange: (value: string) => void;
    onOutcomeChange: (value: string) => void;
    onApplyFilters: () => Promise<void>;
    onLoadMore: () => Promise<void>;
    onExport: () => Promise<void>;
    relativeDate: (value: string | null) => string;
    shortId: (value: string) => string;
  } = $props();
</script>

<section
  class="mb-7 flex items-end justify-between max-[760px]:flex-col max-[760px]:items-start max-[760px]:gap-5"
>
  <div>
    <p class="m-0 mb-[14px] text-[11px] font-extrabold tracking-[0.2em] text-brand-cyan">
      {t('APPEND-ONLY SECURITY LEDGER')}
    </p>
    <h2>{t('Audit Stream')}</h2>
    <p>
      {t('Sanitized tenant events with actor, outcome, target, and immutable source identity.')}
    </p>
  </div>
  <button
    class="rounded-[5px] border border-[#20dce6] bg-gradient-to-br from-[#18cbd5] to-[#0796a7] px-[17px] py-3 font-extrabold text-[#001114] shadow-[0_0_28px_#15d7e221]"
    onclick={onExport}
    disabled={actionBusy === 'audit-export'}
    >{actionBusy === 'audit-export' ? t('Generating…') : `↓ ${t('Export CSV')}`}</button
  >
</section>
<section class="mb-[14px] grid grid-cols-3 gap-3 max-[760px]:grid-cols-1">
  <article class="border border-[#1b2d39] bg-gradient-to-br from-[#0c1720] to-[#091018] p-[18px]">
    <small class="block text-[8px] tracking-[0.14em] text-[#587080]">{t('LOADED EVENTS')}</small
    ><strong class="mt-2 block text-[27px] text-[#dcecf4]">{auditLogs.length}</strong>
  </article>
  <article class="border border-[#1b2d39] bg-gradient-to-br from-[#0c1720] to-[#091018] p-[18px]">
    <small class="block text-[8px] tracking-[0.14em] text-[#587080]">{t('SECURITY WARNINGS')}</small
    ><strong class="mt-2 block text-[27px] text-[#dcecf4]"
      >{auditLogs.filter((item) => item.severity !== 'info').length}</strong
    >
  </article>
  <article>
    <small class="block text-[8px] tracking-[0.14em] text-[#587080]">{t('FAILED OUTCOMES')}</small
    ><strong class="mt-2 block text-[27px] text-[#dcecf4]"
      >{auditLogs.filter((item) => item.outcome === 'failed' || item.outcome === 'denied')
        .length}</strong
    >
  </article>
</section>
<form
  class="mb-[14px] flex items-end gap-[10px] border border-[#1b2d39] bg-[#091018] p-[13px] max-[760px]:flex-col max-[760px]:items-stretch"
  onsubmit={(event) => {
    event.preventDefault();
    onApplyFilters();
  }}
>
  <label class="flex flex-1 flex-col gap-[6px] text-[9px] tracking-[0.08em] text-[#71899a]"
    >{t('Action')}<input
      class="rounded-[4px] border border-[#203140] bg-[#09111a] p-[11px] text-[#eaf6ff]"
      value={auditAction}
      oninput={(event) => onActionChange((event.currentTarget as HTMLInputElement).value)}
      placeholder="provider.credential.updated"
    /></label
  >
  <label class="flex flex-col gap-[6px] text-[9px] tracking-[0.08em] text-[#71899a]"
    >{t('Outcome')}<select
      class="rounded-[4px] border border-[#203140] bg-[#09111a] p-[11px] text-[#eaf6ff]"
      value={auditOutcome}
      onchange={(event) => onOutcomeChange((event.currentTarget as HTMLSelectElement).value)}
      ><option value="">{t('All outcomes')}</option><option value="attempted"
        >{t('Attempted')}</option
      ><option value="succeeded">{t('Succeeded')}</option><option value="failed"
        >{t('Failed')}</option
      ><option value="denied">{t('Denied')}</option><option value="cancelled"
        >{t('Cancelled')}</option
      ></select
    ></label
  >
  <button
    class="rounded-[4px] border border-[#24505b] bg-[#0d2830] px-[14px] py-[10px] text-[10px] text-[#63dce3]"
    >{t('Apply filters')}</button
  >
</form>
<section class="overflow-hidden border border-line bg-panel p-0">
  <div
    class="overflow-auto [&_table]:w-full [&_table]:min-w-[600px] [&_table]:border-collapse [&_th]:border-b [&_th]:border-[#1b2a36] [&_th]:p-[9px] [&_th]:text-left [&_th]:text-[8px] [&_th]:tracking-[0.14em] [&_th]:text-[#52697b] [&_td]:border-b [&_td]:border-[#14212b] [&_td]:p-[12px_9px] [&_td]:text-[10px] [&_td]:text-[#8fa3b3] [&_td_strong]:block [&_td_strong]:text-[11px] [&_td_strong]:text-[#bfced8] [&_td_small]:mt-[3px] [&_td_small]:block [&_td_small]:text-[8px] [&_td_small]:text-[#4e6677]"
  >
    <table>
      <thead
        ><tr
          ><th>{t('Time / Event')}</th><th>{t('Actor')}</th><th>{t('Target')}</th><th
            >{t('Outcome')}</th
          ><th>{t('Severity')}</th><th>{t('Trace')}</th></tr
        ></thead
      ><tbody>
        {#each auditLogs as audit}<tr
            ><td
              ><strong>{audit.action}</strong><small
                >{relativeDate(audit.occurred_at)} · {shortId(audit.source_event_id)}</small
              ></td
            ><td
              ><strong>{t(audit.actor_type)}</strong><small
                >{audit.actor_id ? shortId(audit.actor_id) : 'control plane'}</small
              ></td
            ><td><strong>{audit.target_type}</strong><small>{shortId(audit.target_id)}</small></td
            ><td><span class={statusClass(audit.outcome)}>{t(audit.outcome)}</span></td><td
              ><span class="severity {audit.severity}">{t(audit.severity)}</span></td
            ><td>{audit.trace_id ? shortId(audit.trace_id) : '—'}</td></tr
          >{:else}<tr
            ><td colspan="6"
              ><div class="flex flex-col items-center gap-[6px] p-[50px] text-[#536979]">
                <span class="text-[25px] text-[#2caeb7]">≋</span><strong class="text-[#8498a7]"
                  >{t('No projected audit events')}</strong
                ><small>{t('New domain events appear after the Worker projection runs.')}</small>
              </div></td
            ></tr
          >{/each}
      </tbody>
    </table>
  </div>
  {#if auditHasMore}<button
      class="mx-auto my-[15px] block rounded-[4px] border border-[#24505b] bg-[#0d2830] px-[14px] py-[10px] text-[10px] text-[#63dce3]"
      onclick={onLoadMore}
      disabled={auditLoadingMore}>{t(auditLoadingMore ? 'Loading…' : 'Load older events')}</button
    >{/if}
</section>
