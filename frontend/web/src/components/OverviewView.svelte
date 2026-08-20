<script lang="ts">
  import type { Operation, ProviderAccount, Resource } from '../lib/api';
  import { providerLogoClass, statusClass } from '../lib/ui';

  let {
    activeResources,
    providers,
    resources,
    operations,
    runningOperations,
    failedOperations,
    driftedResources,
    relativeDate,
    shortId,
    onOpenOperations,
    onOpenProviders,
  }: {
    activeResources: Resource[];
    providers: ProviderAccount[];
    resources: Resource[];
    operations: Operation[];
    runningOperations: Operation[];
    failedOperations: Operation[];
    driftedResources: Resource[];
    relativeDate: (value: string | null) => string;
    shortId: (value: string) => string;
    onOpenOperations: () => void;
    onOpenProviders: () => void;
  } = $props();
</script>

<section
  class="relative mb-[14px] flex min-h-[260px] items-center justify-between overflow-hidden border border-[#1b4b58] bg-[radial-gradient(circle_at_75%_50%,#0d3640,#0a151e_58%)] p-10 max-[760px]:min-h-[250px] max-[760px]:p-[30px]"
>
  <div>
    <p class="m-0 mb-[14px] text-[11px] font-extrabold tracking-[0.2em] text-brand-cyan">
      LIVE INFRASTRUCTURE POSTURE
    </p>
    <h2 class="m-0 text-[34px] tracking-[-0.045em] text-[#edf6fb]">
      Your cloud estate,<br /><span class="text-[#65e8ed]">resolved in real time.</span>
    </h2>
    <p class="max-w-[500px] text-[12px] leading-[1.7] text-[#78909f]">
      Unified visibility across every connected provider and canonical resource.
    </p>
  </div>
  <div
    class="relative mr-[8%] grid h-[128px] w-[128px] place-items-center rounded-full border border-[#2ebac5] bg-[#0b222a] shadow-[0_0_50px_#19d4df33] max-[760px]:hidden"
  >
    <div
      class="absolute inset-[10px] rounded-full border border-dashed border-[#2ebac5] opacity-60"
    ></div>
    <strong class="text-[31px] text-[#d9fbff]">{activeResources.length}</strong><small
      class="absolute bottom-[28px] text-[8px] tracking-[0.15em] text-[#69cdd3]">ACTIVE</small
    >
  </div>
</section>
<section
  class="mb-[14px] grid grid-cols-4 gap-[14px] max-[1100px]:grid-cols-2 max-[760px]:grid-cols-1"
>
  <article class="border border-[#1b2e3b] bg-gradient-to-br from-[#0d1721] to-[#090f16] p-[18px]">
    <div
      class="flex items-center justify-between text-[9px] font-extrabold tracking-[0.13em] text-[#6c8292]"
    >
      <span>CONNECTED PROVIDERS</span><i class="text-[18px] not-italic text-[#24dbe3]">⌁</i>
    </div>
    <strong class="mt-4 block text-[30px] text-[#dcecf4]"
      >{providers.filter((item) => item.status === 'active').length}<small>
        / {providers.length}</small
      ></strong
    >
    <p class="text-[10px] text-[#718798]">
      <span class="text-[#3fe4a2]">●</span> Capability registry online
    </p>
  </article>
  <article class="border border-[#1b2e3b] bg-gradient-to-br from-[#0d1721] to-[#090f16] p-[18px]">
    <div
      class="flex items-center justify-between text-[9px] font-extrabold tracking-[0.13em] text-[#6c8292]"
    >
      <span>MANAGED RESOURCES</span><i class="text-[18px] not-italic text-[#a48bff]">◇</i>
    </div>
    <strong class="mt-4 block text-[30px] text-[#dcecf4]">{resources.length}</strong>
    <p class="text-[10px] text-[#718798]">{activeResources.length} currently active</p>
  </article>
  <article class="border border-[#1b2e3b] bg-gradient-to-br from-[#0d1721] to-[#090f16] p-[18px]">
    <div
      class="flex items-center justify-between text-[9px] font-extrabold tracking-[0.13em] text-[#6c8292]"
    >
      <span>ACTIVE OPERATIONS</span><i class="text-[18px] not-italic text-[#ffc064]">↯</i>
    </div>
    <strong class="mt-4 block text-[30px] text-[#dcecf4]">{runningOperations.length}</strong>
    <p class="text-[10px] text-[#718798]">
      {failedOperations.length} failures in recent history
    </p>
  </article>
  <article class="border border-[#1b2e3b] bg-gradient-to-br from-[#0d1721] to-[#090f16] p-[18px]">
    <div
      class="flex items-center justify-between text-[9px] font-extrabold tracking-[0.13em] text-[#6c8292]"
    >
      <span>CONFIGURATION DRIFT</span><i class="text-[18px] not-italic text-[#ff718b]">∆</i>
    </div>
    <strong class="mt-4 block text-[30px] text-[#dcecf4]">{driftedResources.length}</strong>
    <p class="text-[10px] text-[#718798]">
      {driftedResources.length ? 'Review required' : 'Desired state aligned'}
    </p>
  </article>
</section>
<section class="grid grid-cols-2 gap-[14px] max-[1100px]:grid-cols-2 max-[760px]:grid-cols-1">
  <article class="col-span-2 border border-[#1b2e3b] bg-panel p-5 max-[760px]:col-span-1">
    <div class="mb-[18px] flex items-start justify-between border-b border-[#192a36] pb-3">
      <div>
        <p class="m-0 mb-[5px] text-[8px] font-extrabold tracking-[0.2em] text-brand-cyan">
          OPERATION TELEMETRY
        </p>
        <h3 class="m-0 text-[14px] text-[#c5d6df]">Execution stream</h3>
      </div>
      <button
        class="border-0 bg-transparent p-1 text-[11px] text-[#6f8799] hover:text-brand-cyan"
        onclick={() => onOpenOperations()}>View all →</button
      >
    </div>
    <div class="relative h-[195px] overflow-hidden">
      <div
        class="absolute inset-[10px_0_18px] bg-[linear-gradient(#29404d35_1px,transparent_1px),linear-gradient(90deg,#29404d25_1px,transparent_1px)] bg-[length:100%_33%,12.5%_100%]"
      ></div>
      <svg
        viewBox="0 0 800 180"
        preserveAspectRatio="none"
        class="absolute inset-[10px_0_18px] h-[calc(100%-28px)] w-full overflow-visible"
        ><path
          class="fill-none stroke-[#26d5df] [filter:drop-shadow(0_0_6px_#26d5df77)] [stroke-width:2] [vector-effect:non-scaling-stroke]"
          d="M0 145 C70 130 90 135 150 105 S250 135 315 80 S420 115 485 55 S590 90 650 38 S750 55 800 20"
        /></svg
      ><span class="absolute bottom-0 right-1 text-[8px] text-[#425968]"
        >Time-series telemetry connects in Observability phase</span
      >
    </div>
  </article>
  <article class="border border-[#1b2e3b] bg-panel p-5">
    <div class="mb-[18px] flex items-start justify-between border-b border-[#192a36] pb-3">
      <div>
        <p class="m-0 mb-[5px] text-[8px] font-extrabold tracking-[0.2em] text-brand-cyan">
          PROVIDER FABRIC
        </p>
        <h3 class="m-0 text-[14px] text-[#c5d6df]">Connection status</h3>
      </div>
    </div>
    <div class="flex flex-col gap-2">
      {#each providers.slice(0, 4) as provider}<button
          class="flex items-center gap-[10px] border border-[#172633] bg-[#0a1119] p-[9px] text-left text-[#b8c9d5]"
          onclick={() => onOpenProviders()}
          ><span class={providerLogoClass(provider.provider_kind)}
            >{provider.provider_kind.slice(0, 2).toUpperCase()}</span
          ><span class="flex-1"
            ><strong class="block text-[11px]">{provider.name}</strong><small
              class="block text-[9px] uppercase text-[#536c7e]">{provider.provider_kind}</small
            ></span
          ><i
            class={`h-[6px] w-[6px] rounded-full ${provider.status === 'active' ? 'bg-[#3de6a1] shadow-[0_0_9px_#3de6a1]' : 'bg-[#77404a]'}`}
          ></i></button
        >{:else}<div class="p-[30px] text-center text-[11px] text-[#526a7a]">
          Connect a provider to activate the fabric.
        </div>{/each}
    </div>
  </article>
  <article class="col-span-2 border border-[#1b2e3b] bg-panel p-5 max-[760px]:col-span-1">
    <div class="mb-[18px] flex items-start justify-between border-b border-[#192a36] pb-3">
      <div>
        <p class="m-0 mb-[5px] text-[8px] font-extrabold tracking-[0.2em] text-brand-cyan">
          RECENT ACTIVITY
        </p>
        <h3 class="m-0 text-[14px] text-[#c5d6df]">Operation ledger</h3>
      </div>
    </div>
    <div
      class="overflow-auto [&_table]:w-full [&_table]:min-w-[600px] [&_table]:border-collapse [&_th]:border-b [&_th]:border-[#1b2a36] [&_th]:p-[9px] [&_th]:text-left [&_th]:text-[8px] [&_th]:tracking-[0.14em] [&_th]:text-[#52697b] [&_td]:border-b [&_td]:border-[#14212b] [&_td]:p-[12px_9px] [&_td]:text-[10px] [&_td]:text-[#8fa3b3] [&_td_strong]:block [&_td_strong]:text-[11px] [&_td_strong]:text-[#bfced8] [&_td_small]:mt-[3px] [&_td_small]:block [&_td_small]:text-[8px] [&_td_small]:text-[#4e6677]"
    >
      <table>
        <thead
          ><tr><th>Operation</th><th>Target</th><th>Status</th><th>Progress</th><th>Created</th></tr
          ></thead
        ><tbody
          >{#each operations.slice(0, 5) as operation}<tr
              ><td
                ><strong>{operation.operation_type}</strong><small>{shortId(operation.id)}</small
                ></td
              ><td>{operation.target_type}</td><td
                ><span class={statusClass(operation.status)}>{operation.status}</span></td
              ><td
                ><div class="relative h-[3px] w-[100px] bg-[#1b2934]">
                  <i
                    class="block h-full bg-[#1bd4de] shadow-[0_0_7px_#1bd4de]"
                    style={`width:${operation.progress}%`}
                  ></i>
                </div></td
              ><td>{relativeDate(operation.created_at)}</td></tr
            >{:else}<tr
              ><td colspan="5" class="p-[30px] text-center text-[11px] text-[#526a7a]"
                >No operations recorded yet.</td
              ></tr
            >{/each}</tbody
        >
      </table>
    </div>
  </article>
  <article class="border border-[#1b2e3b] bg-panel p-5">
    <div class="mb-[18px] flex items-start justify-between border-b border-[#192a36] pb-3">
      <div>
        <p class="m-0 mb-[5px] text-[8px] font-extrabold tracking-[0.2em] text-brand-cyan">
          COST SIGNAL
        </p>
        <h3 class="m-0 text-[14px] text-[#c5d6df]">Cloud spend</h3>
      </div>
      <span
        class="rounded-[3px] border border-[#2b4050] px-[6px] py-[3px] text-[8px] tracking-[0.12em] text-[#688397]"
        >PHASE 9</span
      >
    </div>
    <div>
      <span class="text-[38px] text-[#37505e]">—</span>
      <p class="text-[11px] leading-[1.5] text-[#5c7181]">
        Billing telemetry is reserved for the Billing bounded context.
      </p>
      <div class="flex h-[75px] items-end gap-[7px] opacity-30">
        <i class="h-[30%] flex-1 bg-gradient-to-b from-[#1dbec7] to-[#143541]"></i><i
          class="h-[55%] flex-1 bg-gradient-to-b from-[#1dbec7] to-[#143541]"
        ></i><i class="h-[40%] flex-1 bg-gradient-to-b from-[#1dbec7] to-[#143541]"></i><i
          class="h-[75%] flex-1 bg-gradient-to-b from-[#1dbec7] to-[#143541]"
        ></i><i class="h-[62%] flex-1 bg-gradient-to-b from-[#1dbec7] to-[#143541]"></i><i
          class="h-[90%] flex-1 bg-gradient-to-b from-[#1dbec7] to-[#143541]"
        ></i>
      </div>
    </div>
  </article>
</section>
