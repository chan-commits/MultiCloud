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

<section class="page-intro">
  <div>
    <p class="eyebrow">APPEND-ONLY SECURITY LEDGER</p>
    <h2>Audit Stream</h2>
    <p>Sanitized tenant events with actor, outcome, target, and immutable source identity.</p>
  </div>
  <button class="primary" onclick={onExport} disabled={actionBusy === 'audit-export'}
    >{actionBusy === 'audit-export' ? 'Generating…' : '↓ Export CSV'}</button
  >
</section>
<section class="audit-summary">
  <article><small>LOADED EVENTS</small><strong>{auditLogs.length}</strong></article>
  <article>
    <small>SECURITY WARNINGS</small><strong
      >{auditLogs.filter((item) => item.severity !== 'info').length}</strong
    >
  </article>
  <article>
    <small>FAILED OUTCOMES</small><strong
      >{auditLogs.filter((item) => item.outcome === 'failed' || item.outcome === 'denied')
        .length}</strong
    >
  </article>
</section>
<form
  class="audit-filters"
  onsubmit={(event) => {
    event.preventDefault();
    onApplyFilters();
  }}
>
  <label
    >Action<input
      value={auditAction}
      oninput={(event) => onActionChange((event.currentTarget as HTMLInputElement).value)}
      placeholder="provider.credential.updated"
    /></label
  >
  <label
    >Outcome<select
      value={auditOutcome}
      onchange={(event) => onOutcomeChange((event.currentTarget as HTMLSelectElement).value)}
      ><option value="">All outcomes</option><option value="attempted">Attempted</option><option
        value="succeeded">Succeeded</option
      ><option value="failed">Failed</option><option value="denied">Denied</option><option
        value="cancelled">Cancelled</option
      ></select
    ></label
  >
  <button>Apply filters</button>
</form>
<section class="panel operation-panel">
  <div class="table-wrap">
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
  {#if auditHasMore}<button class="load-more" onclick={onLoadMore} disabled={auditLoadingMore}
      >{auditLoadingMore ? 'Loading…' : 'Load older events'}</button
    >{/if}
</section>
