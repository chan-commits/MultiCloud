<script lang="ts">
  import type { Resource } from '../lib/api';
  import { statusClass } from '../lib/ui';
  let {
    resources,
    onOpenResource,
    shortId,
  }: {
    resources: Resource[];
    onOpenResource: (resource: Resource) => Promise<void>;
    shortId: (value: string) => string;
  } = $props();
</script>

<section
  class="mb-7 flex items-end justify-between max-[760px]:flex-col max-[760px]:items-start max-[760px]:gap-5"
>
  <div>
    <p class="m-0 mb-[14px] text-[11px] font-extrabold tracking-[0.2em] text-brand-cyan">
      CANONICAL INVENTORY
    </p>
    <h2>Resource Matrix</h2>
    <p>Provider-neutral assets with normalized state and drift visibility.</p>
  </div>
  <div class="flex border border-[#20323f] bg-[#091018] p-[3px]">
    <button class="bg-[#103039] px-[11px] py-[7px] text-[9px] text-[#60e4eb]"
      >All {resources.length}</button
    ><button class="border-0 bg-transparent px-[11px] py-[7px] text-[9px] text-[#63798a]"
      >Compute {resources.filter((item) => item.resource_type === 'compute_instance')
        .length}</button
    ><button class="border-0 bg-transparent px-[11px] py-[7px] text-[9px] text-[#63798a]"
      >DNS {resources.filter((item) => item.resource_type.startsWith('dns_')).length}</button
    >
  </div>
</section>
<section class="overflow-hidden rounded-none border border-line bg-panel p-0">
  <div
    class="overflow-auto [&_table]:w-full [&_table]:min-w-[600px] [&_table]:border-collapse [&_th]:border-b [&_th]:border-[#1b2a36] [&_th]:p-[9px] [&_th]:text-left [&_th]:text-[8px] [&_th]:tracking-[0.14em] [&_th]:text-[#52697b] [&_td]:border-b [&_td]:border-[#14212b] [&_td]:p-[12px_9px] [&_td]:text-[10px] [&_td]:text-[#8fa3b3] [&_td_strong]:block [&_td_strong]:text-[11px] [&_td_strong]:text-[#bfced8] [&_td_small]:mt-[3px] [&_td_small]:block [&_td_small]:text-[8px] [&_td_small]:text-[#4e6677]"
  >
    <table>
      <thead
        ><tr
          ><th>Resource</th><th>Type</th><th>Region</th><th>Lifecycle</th><th>Observed</th><th
          ></th></tr
        ></thead
      ><tbody>
        {#each resources as resource}<tr>
            <td
              ><div class="flex items-center gap-[10px]">
                <span class="text-[17px] text-[#31d6de]"
                  >{resource.resource_type === 'compute_instance' ? '▣' : '◎'}</span
                >
                <div>
                  <strong class="block">{resource.name}</strong><small class="block text-[#587080]"
                    >{shortId(resource.id)}</small
                  >
                </div>
              </div></td
            >
            <td>{resource.resource_type.replaceAll('_', ' ')}</td><td
              >{resource.region ?? 'global'}</td
            >
            <td><span class={statusClass(resource.lifecycle)}>{resource.lifecycle}</span></td><td
              >v{resource.observed_state?.version ?? 0}</td
            >
            <td
              ><button
                class="border-0 bg-transparent text-[9px] text-[#43ced7]"
                onclick={() => onOpenResource(resource)}>Inspect →</button
              ></td
            >
          </tr>{:else}<tr
            ><td colspan="6"
              ><div class="flex flex-col items-center gap-[6px] p-[50px] text-[#536979]">
                <span class="text-[25px] text-[#2caeb7]">◇</span><strong class="text-[#8498a7]"
                  >No resources discovered</strong
                ><small>Run inventory sync from Provider Fabric.</small>
              </div></td
            ></tr
          >{/each}
      </tbody>
    </table>
  </div>
</section>
