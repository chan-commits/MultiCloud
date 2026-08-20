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

<section
  class="mb-7 flex items-end justify-between max-[760px]:flex-col max-[760px]:items-start max-[760px]:gap-5"
>
  <div>
    <p class="eyebrow">RELIABLE EXECUTION</p>
    <h2>Operation Stream</h2>
    <p>Idempotent commands, retry state, and immutable execution history.</p>
  </div>
  <div
    class="flex items-center gap-2 border border-[#1b4b3b] bg-[#0b211a] px-[10px] py-[7px] text-[8px] font-extrabold tracking-[0.12em] text-[#50dca4]
    "
  >
    <i class="h-[7px] w-[7px] rounded-full bg-[#3ff1a7] shadow-[0_0_12px_#3ff1a7]"></i> LIVE QUEUE
  </div>
</section>
<section class="overflow-hidden border border-[#1b2a39] bg-[#0b121c] p-0">
  <div class="w-full overflow-x-auto">
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
              >{#if operation.error_code}<small class="text-[#fb6d86]">{operation.error_code}</small
                >{/if}</td
            >
            <td
              ><div class="progress labeled">
                <i style={`width:${operation.progress}%`}></i><span>{operation.progress}%</span>
              </div></td
            >
            <td>{relativeDate(operation.created_at)}</td><td
              >{#if operation.status === 'queued'}<button
                  class="border-0 bg-transparent text-[9px] text-[#ff718b]"
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
