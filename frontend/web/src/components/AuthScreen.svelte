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

<main class="grid min-h-screen grid-cols-[1.25fr_0.75fr] overflow-hidden max-[760px]:grid-cols-1">
  <section
    class="relative flex flex-col justify-center overflow-hidden bg-[radial-gradient(circle_at_52%_45%,#0c3340_0,transparent_23%),linear-gradient(135deg,#06131c,#05090f_70%)] p-[8vw] max-[760px]:hidden"
    aria-hidden="true"
  >
    <div
      class="absolute right-[-120px] top-[calc(50%-300px)] h-[600px] w-[600px] rounded-full border border-[#1ddde526]"
    ></div>
    <div
      class="absolute right-[-20px] top-[calc(50%-200px)] h-[400px] w-[400px] rounded-full border border-dashed border-[#1ddde526]"
    ></div>
    <div
      class="relative z-[1] mb-12 grid h-[58px] w-[58px] place-items-center border border-[#1fcad7] font-extrabold text-brand-cyan shadow-[0_0_45px_#16d9e326]"
    >
      MC
    </div>
    <p
      class="relative z-[1] m-0 mb-[14px] text-[11px] font-extrabold tracking-[0.2em] text-brand-cyan"
    >
      MULTI-TENANT CONTROL PLANE
    </p>
    <h1
      class="relative z-[1] m-0 max-w-[850px] text-[clamp(42px,5vw,78px)] leading-[1.03] tracking-[-0.055em] text-[#f1f8ff]"
    >
      Operate every cloud<br />from one command layer.
    </h1>
    <p class="relative z-[1] max-w-[570px] text-[17px] leading-[1.7] text-[#8499aa]">
      Provider-neutral infrastructure, deterministic operations, and tenant-safe control.
    </p>
    <div
      class="relative z-[1] mt-[60px] flex items-center gap-3 font-mono text-[10px] font-bold tracking-[0.18em] text-[#5d7989]"
    >
      <span class="h-[7px] w-[7px] rounded-full bg-[#3ff1a7] shadow-[0_0_12px_#3ff1a7]"></span> CONTROL
      FABRIC ONLINE
    </div>
  </section>
  <section
    class="grid min-h-screen place-items-center border-l border-[#14212d] bg-[#080d14] p-10 max-[760px]:p-[25px]"
  >
    <form
      class="flex w-[min(420px,100%)] flex-col gap-[23px]"
      onsubmit={(event) => {
        event.preventDefault();
        submit();
      }}
    >
      <div
        class="mb-[54px] flex items-center gap-[11px] text-[18px] font-[750] text-[#eff8ff]
        "
      >
        <span
          class="grid h-[30px] w-[30px] rotate-45 place-items-center border border-brand-cyan text-[14px] text-brand-cyan"
          >M</span
        ><span>MultiCloud</span>
      </div>
      <div>
        <p class="m-0 mb-[14px] text-[11px] font-extrabold tracking-[0.2em] text-brand-cyan">
          SECURE ACCESS
        </p>
        <h2 class="m-0 text-[30px] tracking-[-0.035em] text-[#f2f8ff]">
          {mode === 'login' ? 'Welcome back' : 'Create account'}
        </h2>
        <p class="m-[8px_0_0] text-muted">
          {mode === 'login'
            ? 'Authenticate to enter your organization workspace.'
            : 'Join the control plane and create your tenant workspace.'}
        </p>
      </div>
      {#if mode === 'register'}<label
          class="flex flex-col gap-[9px] text-[12px] font-semibold text-[#9fb0c0]"
          >Display name<input
            class="rounded-[6px] border border-[#203140] bg-[#09111a] p-[13px_14px] text-[#eaf6ff] outline-none transition focus:border-[#1ab9c6] focus:shadow-[0_0_0_3px_#16d9e311]"
            bind:value={displayName}
            autocomplete="name"
            maxlength="120"
            required
          /></label
        >{/if}
      <label class="flex flex-col gap-[9px] text-[12px] font-semibold text-[#9fb0c0]"
        >Email address<input
          class="rounded-[6px] border border-[#203140] bg-[#09111a] p-[13px_14px] text-[#eaf6ff] outline-none transition focus:border-[#1ab9c6] focus:shadow-[0_0_0_3px_#16d9e311]"
          bind:value={email}
          type="email"
          autocomplete="email"
          placeholder="operator@company.com"
          required
        /></label
      >
      <label class="flex flex-col gap-[9px] text-[12px] font-semibold text-[#9fb0c0]"
        >Password<input
          class="rounded-[6px] border border-[#203140] bg-[#09111a] p-[13px_14px] text-[#eaf6ff] outline-none transition focus:border-[#1ab9c6] focus:shadow-[0_0_0_3px_#16d9e311]"
          bind:value={password}
          type="password"
          autocomplete={mode === 'login' ? 'current-password' : 'new-password'}
          minlength="12"
          placeholder="••••••••••••"
          required
        /></label
      >
      {#if error}<p
          class="m-0 rounded-[5px] border border-[#60202e] bg-[#2b1118] px-3 py-[10px] text-[#ff788e]"
        >
          {error}
        </p>{/if}
      <button
        class="flex w-full justify-between rounded-[5px] border border-[#20dce6] bg-gradient-to-br from-[#18cbd5] to-[#0796a7] px-[18px] py-[14px] font-extrabold text-[#001114] shadow-[0_0_28px_#15d7e221]"
        disabled={authenticating}
        >{authenticating
          ? 'Working…'
          : mode === 'login'
            ? 'Enter Command Center'
            : 'Create account'}<span>→</span></button
      >
      {#if registrationEnabled}<button
          class="border-0 bg-transparent text-[11px] text-[#53cbd2]"
          type="button"
          onclick={() => (mode = mode === 'login' ? 'register' : 'login')}
          >{mode === 'login'
            ? 'New user? Create an account'
            : 'Already registered? Sign in'}</button
        >{:else}<p class="m-[-10px_0_0] text-center text-[10px] text-[#a1747d]">
          Public registration is currently closed.
        </p>{/if}
      <p class="m-0 text-center text-[11px] text-[#526879]">
        <span class="text-[#20bfc9]">◆</span>
        {platformInitialized
          ? 'Platform access policy is enforced by the control plane.'
          : 'The first platform administrator must be initialized locally by CLI.'}
      </p>
    </form>
  </section>
</main>
