<script lang="ts">
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

<section class="empty-state onboarding">
  <div class="empty-orbit">◎</div>
  <h2>Create your organization</h2>
  <p>Your account is ready. Establish an isolated tenant workspace to continue.</p>
  <form
    onsubmit={(event) => {
      event.preventDefault();
      submit();
    }}
  >
    <label
      >Organization name<input
        bind:value={name}
        maxlength="160"
        placeholder="Acme Infrastructure"
        required
      /></label
    >
    <label
      >Organization slug<input
        bind:value={slug}
        minlength="3"
        maxlength="80"
        pattern="[a-z0-9](?:[a-z0-9-]*[a-z0-9])?"
        placeholder="acme-infra"
        required
      /></label
    >
    <button class="primary" disabled={creating}
      >{creating ? 'Creating…' : 'Create workspace'}</button
    >
  </form>
</section>
