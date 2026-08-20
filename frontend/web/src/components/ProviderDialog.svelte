<script lang="ts">
  type ProviderKind = 'cloudflare' | 'vultr' | 'ovh';
  let {
    saving,
    onClose,
    onCreate,
  }: {
    saving: boolean;
    onClose: () => void;
    onCreate: (payload: Record<string, unknown>) => Promise<void>;
  } = $props();
  let providerKind = $state<ProviderKind>('cloudflare');
  let providerName = $state(''),
    apiToken = $state(''),
    emailIdentity = $state(''),
    globalApiKey = $state('');
  let applicationKey = $state(''),
    applicationSecret = $state(''),
    consumerKey = $state(''),
    useGlobalKey = $state(false);

  async function submit() {
    let credential: Record<string, unknown>;
    if (providerKind === 'cloudflare' && useGlobalKey)
      credential = {
        credential_type: 'global_api_key',
        email: emailIdentity,
        global_api_key: globalApiKey,
      };
    else if (providerKind === 'ovh')
      credential = {
        credential_type: 'ovh_application',
        application_key: applicationKey,
        application_secret: applicationSecret,
        consumer_key: consumerKey,
      };
    else credential = { credential_type: 'api_token', api_token: apiToken };
    await onCreate({
      provider_kind: providerKind,
      name: providerName,
      configuration: {},
      ...credential,
    });
  }
</script>

<div
  class="fixed inset-0 z-50 grid place-items-center bg-[#020508cc] p-5 backdrop-blur-[8px]"
  role="presentation"
  onclick={(event) => event.target === event.currentTarget && onClose()}
>
  <form
    class="flex max-h-[90vh] w-[min(540px,100%)] flex-col gap-[18px] overflow-auto border border-[#23404e] bg-[#0a1119] p-[25px] shadow-[0_25px_100px_#000]"
    onsubmit={(event) => {
      event.preventDefault();
      submit();
    }}
  >
    <div class="flex items-start justify-between border-b border-[#192a36] pb-[17px]">
      <div>
        <p class="m-0 mb-[14px] text-[11px] font-extrabold tracking-[0.2em] text-brand-cyan">
          NEW CONNECTION
        </p>
        <h2 class="m-0 text-[30px] tracking-[-0.035em] text-[#f2f8ff]">Connect provider</h2>
      </div>
      <button
        class="border-0 bg-transparent text-[22px] text-[#6c8292]"
        type="button"
        onclick={onClose}>×</button
      >
    </div>
    <label
      >Provider<select bind:value={providerKind}
        ><option value="cloudflare">Cloudflare · DNS</option><option value="vultr"
          >Vultr · Compute</option
        ><option value="ovh">OVHcloud · VPS</option></select
      ></label
    >
    <label
      >Connection name<input
        bind:value={providerName}
        placeholder="Production account"
        required
      /></label
    >
    {#if providerKind === 'cloudflare'}<div class="grid grid-cols-2 gap-2 max-[760px]:grid-cols-1">
        <button
          class={`border p-3 text-left ${!useGlobalKey ? 'border-[#1ea9b3] bg-[#0c2930]' : 'border-[#273a47] bg-[#0b151e]'} text-[#91a5b4]`}
          type="button"
          onclick={() => (useGlobalKey = false)}
          ><strong>API Token</strong><small>✓ Recommended · Restricted scope</small></button
        ><button
          class={`border p-3 text-left ${useGlobalKey ? 'border-[#8f5428] bg-[#2a190f]' : 'border-[#273a47] bg-[#0b151e]'} text-[#91a5b4]`}
          type="button"
          onclick={() => (useGlobalKey = true)}
          ><strong>Global API Key</strong><small>⚠ Legacy · Full account access</small></button
        >
      </div>
      {#if useGlobalKey}<div
          class="flex flex-col gap-[3px] border border-[#764421] bg-[#26170f] p-[11px] text-[10px] text-[#e7a166]"
        >
          <strong>High-risk credential</strong><span
            >Use a scoped API Token whenever possible. This action is security-audited.</span
          >
        </div>
        <label>Cloudflare email<input bind:value={emailIdentity} type="email" required /></label
        ><label>Global API Key<input bind:value={globalApiKey} type="password" required /></label
        >{:else}<label
          >API Token<input
            bind:value={apiToken}
            type="password"
            autocomplete="off"
            required
          /><small>Token should include Zone Read and DNS Edit scopes.</small></label
        >{/if}
    {:else if providerKind === 'vultr'}<label
        >API Token<input bind:value={apiToken} type="password" autocomplete="off" required /><small
          >Use a dedicated token with minimum Compute permissions.</small
        ></label
      >
    {:else}<div class="border border-[#1c5862] bg-[#0d242a] p-[11px] text-[10px] text-[#68ccd3]">
        OVHcloud signs each request with three encrypted credential components.
      </div>
      <label>Application Key<input bind:value={applicationKey} autocomplete="off" required /></label
      ><label
        >Application Secret<input
          bind:value={applicationSecret}
          type="password"
          autocomplete="off"
          required
        /></label
      ><label
        >Consumer Key<input
          bind:value={consumerKey}
          type="password"
          autocomplete="off"
          required
        /></label
      >{/if}
    <div class="flex justify-end gap-[9px] pt-2">
      <button
        class="flex-1 border border-[#293c49] bg-[#0b141d] p-[9px] text-[10px] font-bold text-[#8ea4b3]"
        type="button"
        onclick={onClose}>Cancel</button
      ><button
        class="flex-1 rounded-[5px] border border-[#20dce6] bg-gradient-to-br from-[#18cbd5] to-[#0796a7] px-[17px] py-3 font-extrabold text-[#001114] shadow-[0_0_28px_#15d7e221]"
        disabled={saving}>{saving ? 'Encrypting…' : 'Encrypt & connect'}</button
      >
    </div>
  </form>
</div>
