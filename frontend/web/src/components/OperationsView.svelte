<script lang="ts">
  import type { Operation } from '../lib/api';
  let {
    operations,
    actionBusy,
    onCancel,
    relativeDate,
    shortId,
  }: {
    operations: Operation[];
    actionBusy: string;
    onCancel: (operation: Operation) => Promise<void>;
    relativeDate: (value: string | null) => string;
    shortId: (value: string) => string;
  } = $props();
</script>

<section class="page-intro">
  <div>
    <p class="eyebrow">RELIABLE EXECUTION</p>
    <h2>Operation Stream</h2>
    <p>Idempotent commands, retry state, and immutable execution history.</p>
  </div>
  <div class="live-pill"><i></i> LIVE QUEUE</div>
</section>
<section class="panel operation-panel">
  <div class="table-wrap">
    <table>
      <thead
        ><tr
          ><th>ID / Type</th><th>Target</th><th>Status</th><th>Progress</th><th>Created</th><th
          ></th></tr
        ></thead
      ><tbody>
        {#each operations as operation}<tr>
            <td
              ><strong>{operation.operation_type}</strong><small>{shortId(operation.id)}</small></td
            >
            <td
              ><strong>{operation.target_type}</strong><small
                >{operation.target_id ? shortId(operation.target_id) : '—'}</small
              ></td
            >
            <td
              ><span class="status {operation.status}">{operation.status}</span
              >{#if operation.error_code}<small class="error-code">{operation.error_code}</small
                >{/if}</td
            >
            <td
              ><div class="progress labeled">
                <i style={`width:${operation.progress}%`}></i><span>{operation.progress}%</span>
              </div></td
            >
            <td>{relativeDate(operation.created_at)}</td><td
              >{#if operation.status === 'queued'}<button
                  class="row-action danger"
                  onclick={() => onCancel(operation)}
                  disabled={actionBusy === operation.id}>Cancel</button
                >{/if}</td
            >
          </tr>{:else}<tr><td colspan="6" class="table-empty">Operation history is empty.</td></tr
          >{/each}
      </tbody>
    </table>
  </div>
</section>
