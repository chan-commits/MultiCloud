<script lang="ts">
  const modules = ['Organizations', 'Cloud Assets', 'Tickets', 'Billing'];
</script>

<svelte:head>
  <title>MultiCloud Control Plane</title>
</svelte:head>

<main class="mx-auto flex min-h-screen max-w-6xl flex-col justify-center px-6 py-16">
  <p class="text-sm font-semibold uppercase tracking-[0.2em] text-sky-400">Control plane</p>
  <h1 class="mt-4 max-w-3xl text-5xl font-semibold tracking-tight text-white sm:text-7xl">
    MultiCloud engineering baseline is ready.
  </h1>
  <p class="mt-6 max-w-2xl text-lg leading-8 text-slate-300">
    Multi-tenant cloud operations with explicit domain boundaries, reliable workflows, and auditable access.
  </p>

  <ul class="mt-12 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
    {#each modules as module}
      <li class="rounded-2xl border border-white/10 bg-white/5 p-5 text-slate-200">{module}</li>
    {/each}
  </ul>
</main>
