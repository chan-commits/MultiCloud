<script lang="ts">
  import type { Resource } from '../lib/api';
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
    <p class="eyebrow">CANONICAL INVENTORY</p>
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
<section class="overflow-hidden rounded-none border border-[#1b2a39] bg-[#0b121c] p-0">
  <div class="w-full overflow-x-auto">
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
            <td><span class="status {resource.lifecycle}">{resource.lifecycle}</span></td><td
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
              ><div class="empty-row">
                <span>◇</span><strong>No resources discovered</strong><small
                  >Run inventory sync from Provider Fabric.</small
                >
              </div></td
            ></tr
          >{/each}
      </tbody>
    </table>
  </div>
</section>
