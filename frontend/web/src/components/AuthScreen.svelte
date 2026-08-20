<script lang="ts">
  let {
    registrationEnabled,
    platformInitialized,
    authenticating,
    error,
    onLogin,
    onRegister,
  }: {
    registrationEnabled: boolean;
    platformInitialized: boolean;
    authenticating: boolean;
    error: string;
    onLogin: (email: string, password: string) => Promise<void>;
    onRegister: (email: string, password: string, displayName: string) => Promise<void>;
  } = $props();

  let mode = $state<'login' | 'register'>('login');
  let email = $state('');
  let password = $state('');
  let displayName = $state('');

  async function submit() {
    if (mode === 'register' && registrationEnabled) {
      await onRegister(email, password, displayName);
    } else {
      await onLogin(email, password);
    }
  }

  $effect(() => {
    if (!registrationEnabled) mode = 'login';
  });
</script>

<main class="auth-shell">
  <section class="auth-ambient" aria-hidden="true">
    <div class="orbit one"></div>
    <div class="orbit two"></div>
    <div class="auth-mark">MC</div>
    <p class="eyebrow">MULTI-TENANT CONTROL PLANE</p>
    <h1>Operate every cloud<br />from one command layer.</h1>
    <p>Provider-neutral infrastructure, deterministic operations, and tenant-safe control.</p>
    <div class="signal-row"><span></span> CONTROL FABRIC ONLINE</div>
  </section>
  <section class="auth-panel">
    <form
      class="auth-form"
      onsubmit={(event) => {
        event.preventDefault();
        submit();
      }}
    >
      <div class="brand"><span class="brand-glyph">M</span><span>MultiCloud</span></div>
      <div>
        <p class="kicker">SECURE ACCESS</p>
        <h2>{mode === 'login' ? 'Welcome back' : 'Create account'}</h2>
        <p>
          {mode === 'login'
            ? 'Authenticate to enter your organization workspace.'
            : 'Join the control plane and create your tenant workspace.'}
        </p>
      </div>
      {#if mode === 'register'}<label
          >Display name<input
            bind:value={displayName}
            autocomplete="name"
            maxlength="120"
            required
          /></label
        >{/if}
      <label
        >Email address<input
          bind:value={email}
          type="email"
          autocomplete="email"
          placeholder="operator@company.com"
          required
        /></label
      >
      <label
        >Password<input
          bind:value={password}
          type="password"
          autocomplete={mode === 'login' ? 'current-password' : 'new-password'}
          minlength="12"
          placeholder="••••••••••••"
          required
        /></label
      >
      {#if error}<p class="form-error">{error}</p>{/if}
      <button class="primary wide" disabled={authenticating}
        >{authenticating
          ? 'Working…'
          : mode === 'login'
            ? 'Enter Command Center'
            : 'Create account'}<span>→</span></button
      >
      {#if registrationEnabled}<button
          class="auth-switch"
          type="button"
          onclick={() => (mode = mode === 'login' ? 'register' : 'login')}
          >{mode === 'login'
            ? 'New user? Create an account'
            : 'Already registered? Sign in'}</button
        >{:else}<p class="registration-closed">Public registration is currently closed.</p>{/if}
      <p class="security-note">
        <span>◆</span>
        {platformInitialized
          ? 'Platform access policy is enforced by the control plane.'
          : 'The first platform administrator must be initialized locally by CLI.'}
      </p>
    </form>
  </section>
</main>
