<script lang="ts">
  import type { Operation } from '../lib/api';
  import { statusClass } from '../lib/ui';
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
    <p class="m-0 mb-[14px] text-[11px] font-extrabold tracking-[0.2em] text-brand-cyan">
      RELIABLE EXECUTION
    </p>
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
<section class="overflow-hidden border border-line bg-panel p-0">
  <div
    class="overflow-auto [&_table]:w-full [&_table]:min-w-[600px] [&_table]:border-collapse [&_th]:border-b [&_th]:border-[#1b2a36] [&_th]:p-[9px] [&_th]:text-left [&_th]:text-[8px] [&_th]:tracking-[0.14em] [&_th]:text-[#52697b] [&_td]:border-b [&_td]:border-[#14212b] [&_td]:p-[12px_9px] [&_td]:text-[10px] [&_td]:text-[#8fa3b3] [&_td_strong]:block [&_td_strong]:text-[11px] [&_td_strong]:text-[#bfced8] [&_td_small]:mt-[3px] [&_td_small]:block [&_td_small]:text-[8px] [&_td_small]:text-[#4e6677]"
  >
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
              ><span class={statusClass(operation.status)}>{operation.status}</span
              >{#if operation.error_code}<small class="text-[#fb6d86]">{operation.error_code}</small
                >{/if}</td
            >
            <td
              ><div
                class="relative mt-1 h-[3px] w-[120px] bg-[#1b2934]
                "
              >
                <i
                  class="block h-full bg-[#1bd4de] shadow-[0_0_7px_#1bd4de]"
                  style={`width:${operation.progress}%`}
                ></i><span class="absolute right-[-28px] top-[-5px] text-[8px] text-[#577080]"
                  >{operation.progress}%</span
                >
              </div></td
            >
            <td>{relativeDate(operation.created_at)}</td><td
              >{#if operation.status === 'queued'}<button
                  class="border-0 bg-transparent text-[9px] text-[#ff718b]"
                  onclick={() => onCancel(operation)}
                  disabled={actionBusy === operation.id}>Cancel</button
                >{/if}</td
            >
          </tr>{:else}<tr
            ><td colspan="6" class="p-[30px] text-center text-[11px] text-[#526a7a]"
              >Operation history is empty.</td
            ></tr
          >{/each}
      </tbody>
    </table>
  </div>
</section>
