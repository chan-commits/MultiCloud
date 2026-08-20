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
  class="modal-backdrop"
  role="presentation"
  onclick={(event) => event.target === event.currentTarget && onClose()}
>
  <form
    class="modal"
    onsubmit={(event) => {
      event.preventDefault();
      submit();
    }}
  >
    <div class="modal-head">
      <div>
        <p class="kicker">NEW CONNECTION</p>
        <h2>Connect provider</h2>
      </div>
      <button type="button" onclick={onClose}>×</button>
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
    {#if providerKind === 'cloudflare'}<div class="credential-choice">
        <button type="button" class:active={!useGlobalKey} onclick={() => (useGlobalKey = false)}
          ><strong>API Token</strong><small>✓ Recommended · Restricted scope</small></button
        ><button type="button" class:risk={useGlobalKey} onclick={() => (useGlobalKey = true)}
          ><strong>Global API Key</strong><small>⚠ Legacy · Full account access</small></button
        >
      </div>
      {#if useGlobalKey}<div class="risk-banner">
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
    {:else}<div class="info-banner">
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
    <div class="modal-actions">
      <button type="button" onclick={onClose}>Cancel</button><button
        class="primary"
        disabled={saving}>{saving ? 'Encrypting…' : 'Encrypt & connect'}</button
      >
    </div>
  </form>
</div>
