<script lang="ts">
  import { t } from '$lib/i18n.svelte';
  let {
    creating,
    onCreate,
  }: { creating: boolean; onCreate: (name: string, slug: string) => Promise<void> } = $props();
  let name = $state('');
  let slug = $state('');

  async function submit() {
    await onCreate(name, slug);
  }
</script>

<section
  class="flex min-h-[400px] flex-col items-center justify-center border border-dashed border-[#233746] bg-[#091019] text-center text-[#617687]"
>
  <div
    class="mb-[10px] grid h-[70px] w-[70px] place-items-center rounded-full border border-[#1f6771] text-[25px] text-[#37d7df] shadow-[0_0_30px_#19d4df15]"
  >
    ◎
  </div>
  <h2 class="mb-1 text-[#c7d5df]">{t('Create your organization')}</h2>
  <p class="max-w-[430px] text-[12px]">
    {t('Your account is ready. Establish an isolated tenant workspace to continue.')}
  </p>
  <form
    class="mt-[15px] grid w-[min(420px,90%)] gap-[13px] text-left"
    onsubmit={(event) => {
      event.preventDefault();
      submit();
    }}
  >
    <label class="flex flex-col gap-[6px] text-[9px] tracking-[0.08em] text-[#71899a]"
      >{t('Organization name')}<input
        class="rounded-[4px] border border-[#203140] bg-[#09111a] p-[11px] text-[#eaf6ff]"
        bind:value={name}
        maxlength="160"
        placeholder="Acme Infrastructure"
        required
      /></label
    >
    <label class="flex flex-col gap-[6px] text-[9px] tracking-[0.08em] text-[#71899a]"
      >{t('Organization slug')}<input
        class="rounded-[4px] border border-[#203140] bg-[#09111a] p-[11px] text-[#eaf6ff]"
        bind:value={slug}
        minlength="3"
        maxlength="80"
        pattern="[a-z0-9](?:[a-z0-9-]*[a-z0-9])?"
        placeholder="acme-infra"
        required
      /></label
    >
    <button
      class="justify-self-start rounded-[5px] border border-[#20dce6] bg-gradient-to-br from-[#18cbd5] to-[#0796a7] px-[17px] py-3 font-extrabold text-[#001114] shadow-[0_0_28px_#15d7e221]"
      disabled={creating}>{t(creating ? 'Creating…' : 'Create workspace')}</button
    >
  </form>
</section>
