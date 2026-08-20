<script lang="ts">
  import type { AuditLog } from '../lib/api';
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
    <p class="eyebrow">APPEND-ONLY SECURITY LEDGER</p>
    <h2>Audit Stream</h2>
    <p>Sanitized tenant events with actor, outcome, target, and immutable source identity.</p>
  </div>
  <button
    class="rounded-[5px] border border-[#20dce6] bg-gradient-to-br from-[#18cbd5] to-[#0796a7] px-[17px] py-3 font-extrabold text-[#001114] shadow-[0_0_28px_#15d7e221]"
    onclick={onExport}
    disabled={actionBusy === 'audit-export'}
    >{actionBusy === 'audit-export' ? 'Generating…' : '↓ Export CSV'}</button
  >
</section>
<section class="mb-[14px] grid grid-cols-3 gap-3 max-[760px]:grid-cols-1">
  <article class="border border-[#1b2d39] bg-gradient-to-br from-[#0c1720] to-[#091018] p-[18px]">
    <small class="block text-[8px] tracking-[0.14em] text-[#587080]">LOADED EVENTS</small><strong
      class="mt-2 block text-[27px] text-[#dcecf4]">{auditLogs.length}</strong
    >
  </article>
  <article class="border border-[#1b2d39] bg-gradient-to-br from-[#0c1720] to-[#091018] p-[18px]">
    <small class="block text-[8px] tracking-[0.14em] text-[#587080]">SECURITY WARNINGS</small
    ><strong class="mt-2 block text-[27px] text-[#dcecf4]"
      >{auditLogs.filter((item) => item.severity !== 'info').length}</strong
    >
  </article>
  <article>
    <small class="block text-[8px] tracking-[0.14em] text-[#587080]">FAILED OUTCOMES</small><strong
      class="mt-2 block text-[27px] text-[#dcecf4]"
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
    >Action<input
      class="rounded-[4px] border border-[#203140] bg-[#09111a] p-[11px] text-[#eaf6ff]"
      value={auditAction}
      oninput={(event) => onActionChange((event.currentTarget as HTMLInputElement).value)}
      placeholder="provider.credential.updated"
    /></label
  >
  <label class="flex flex-col gap-[6px] text-[9px] tracking-[0.08em] text-[#71899a]"
    >Outcome<select
      class="rounded-[4px] border border-[#203140] bg-[#09111a] p-[11px] text-[#eaf6ff]"
      value={auditOutcome}
      onchange={(event) => onOutcomeChange((event.currentTarget as HTMLSelectElement).value)}
      ><option value="">All outcomes</option><option value="attempted">Attempted</option><option
        value="succeeded">Succeeded</option
      ><option value="failed">Failed</option><option value="denied">Denied</option><option
        value="cancelled">Cancelled</option
      ></select
    ></label
  >
  <button
    class="rounded-[4px] border border-[#24505b] bg-[#0d2830] px-[14px] py-[10px] text-[10px] text-[#63dce3]"
    >Apply filters</button
  >
</form>
<section class="overflow-hidden border border-[#1b2a39] bg-[#0b121c] p-0">
  <div class="w-full overflow-x-auto">
    <table>
      <thead
        ><tr
          ><th>Time / Event</th><th>Actor</th><th>Target</th><th>Outcome</th><th>Severity</th><th
            >Trace</th
          ></tr
        ></thead
      ><tbody>
        {#each auditLogs as audit}<tr
            ><td
              ><strong>{audit.action}</strong><small
                >{relativeDate(audit.occurred_at)} · {shortId(audit.source_event_id)}</small
              ></td
            ><td
              ><strong>{audit.actor_type}</strong><small
                >{audit.actor_id ? shortId(audit.actor_id) : 'control plane'}</small
              ></td
            ><td><strong>{audit.target_type}</strong><small>{shortId(audit.target_id)}</small></td
            ><td><span class="status {audit.outcome}">{audit.outcome}</span></td><td
              ><span class="severity {audit.severity}">{audit.severity}</span></td
            ><td>{audit.trace_id ? shortId(audit.trace_id) : '—'}</td></tr
          >{:else}<tr
            ><td colspan="6"
              ><div class="empty-row">
                <span>≋</span><strong>No projected audit events</strong><small
                  >New domain events appear after the Worker projection runs.</small
                >
              </div></td
            ></tr
          >{/each}
      </tbody>
    </table>
  </div>
  {#if auditHasMore}<button
      class="mx-auto my-[15px] block rounded-[4px] border border-[#24505b] bg-[#0d2830] px-[14px] py-[10px] text-[10px] text-[#63dce3]"
      onclick={onLoadMore}
      disabled={auditLoadingMore}>{auditLoadingMore ? 'Loading…' : 'Load older events'}</button
    >{/if}
</section>
