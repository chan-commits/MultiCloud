<script lang="ts">
  import type { Ticket, TicketComment } from '../lib/api';
  import { statusClass } from '../lib/ui';
  import { t } from '$lib/i18n.svelte';

  let {
    tickets,
    selected,
    comments,
    actionBusy,
    onCreate,
    onSelect,
    onStatus,
    onComment,
    relativeDate,
  }: {
    tickets: Ticket[];
    selected: Ticket | null;
    comments: TicketComment[];
    actionBusy: string;
    onCreate: (subject: string, description: string, priority: string) => Promise<void>;
    onSelect: (ticket: Ticket) => Promise<void>;
    onStatus: (ticket: Ticket, status: string) => Promise<void>;
    onComment: (body: string) => Promise<void>;
    relativeDate: (value: string | null) => string;
  } = $props();
  let subject = $state(''),
    description = $state(''),
    priority = $state('normal'),
    comment = $state('');

  async function create() {
    if (!subject.trim() || !description.trim()) return;
    await onCreate(subject, description, priority);
    subject = '';
    description = '';
  }
  async function submitComment() {
    if (!comment.trim()) return;
    await onComment(comment);
    comment = '';
  }
</script>

<section
  class="mb-7 flex items-end justify-between max-[760px]:flex-col max-[760px]:items-start max-[760px]:gap-5"
>
  <div>
    <p class="m-0 mb-[14px] text-[11px] font-extrabold tracking-[0.2em] text-brand-cyan">
      {t('TENANT SUPPORT CONTROL')}
    </p>
    <h2>{t('Support Desk')}</h2>
    <p>{t('SLA-aware ticket lifecycle with auditable collaboration.')}</p>
  </div>
</section>

<div class="grid grid-cols-[minmax(0,1fr)_380px] gap-4 max-[980px]:grid-cols-1">
  <section class="border border-line bg-panel">
    <form
      class="grid gap-3 border-b border-line p-4"
      onsubmit={(event) => {
        event.preventDefault();
        void create();
      }}
    >
      <div class="grid grid-cols-[1fr_130px] gap-3 max-[600px]:grid-cols-1">
        <input
          class="rounded border border-line bg-[#09111a] p-3 text-sm text-white"
          bind:value={subject}
          placeholder={t('Ticket subject')}
          maxlength="200"
          required
        />
        <select
          class="rounded border border-line bg-[#09111a] p-3 text-sm text-white"
          bind:value={priority}
        >
          {#each ['low', 'normal', 'high', 'urgent'] as value}<option {value}>{t(value)}</option
            >{/each}
        </select>
      </div>
      <textarea
        class="min-h-24 rounded border border-line bg-[#09111a] p-3 text-sm text-white"
        bind:value={description}
        placeholder={t('Describe the issue')}
        maxlength="20000"
        required></textarea>
      <button
        class="justify-self-start rounded bg-brand-cyan px-4 py-2 text-xs font-bold text-black"
        disabled={actionBusy === 'ticket-create'}
        >{t(actionBusy === 'ticket-create' ? 'Creating…' : 'Create ticket')}</button
      >
    </form>
    <div class="divide-y divide-line">
      {#each tickets as ticket}
        <button
          class={`grid w-full grid-cols-[70px_1fr_auto] items-center gap-3 p-4 text-left hover:bg-white/[0.03] ${selected?.id === ticket.id ? 'bg-white/[0.04]' : ''}`}
          onclick={() => onSelect(ticket)}
        >
          <strong class="text-brand-cyan">#{ticket.number}</strong><span
            ><strong class="block text-sm text-white">{ticket.subject}</strong><small
              class="text-muted">{relativeDate(ticket.updated_at)}</small
            ></span
          >
          <span class={statusClass(ticket.status)}>{t(ticket.status)}</span>
        </button>
      {:else}<div class="p-12 text-center text-muted">{t('No tickets yet.')}</div>{/each}
    </div>
  </section>

  <aside class="border border-line bg-panel p-4">
    {#if selected}
      <div class="mb-4 flex items-start justify-between gap-3">
        <div>
          <small class="text-brand-cyan">#{selected.number}</small>
          <h3 class="mt-1 text-white">{selected.subject}</h3>
        </div>
        <span class={statusClass(selected.priority)}>{t(selected.priority)}</span>
      </div>
      <p class="whitespace-pre-wrap text-sm text-muted">{selected.description}</p>
      <label class="mt-4 grid gap-2 text-xs text-muted"
        >{t('Status')}<select
          class="rounded border border-line bg-[#09111a] p-2 text-white"
          value={selected.status}
          disabled={actionBusy === selected.id}
          onchange={(event) => onStatus(selected, event.currentTarget.value)}
        >
          {#each ['open', 'in_progress', 'waiting_on_customer', 'resolved', 'closed'] as value}<option
              {value}>{t(value)}</option
            >{/each}
        </select></label
      >
      <div class="my-5 border-t border-line pt-4">
        <h4 class="mb-3 text-xs uppercase tracking-wider text-muted">{t('Conversation')}</h4>
        <div class="max-h-64 space-y-2 overflow-auto">
          {#each comments as item}<article class="rounded border border-line bg-[#09111a] p-3">
              <p class="m-0 whitespace-pre-wrap text-xs text-white">{item.body}</p>
              <small class="mt-2 block text-muted">{relativeDate(item.created_at)}</small>
            </article>{:else}<p class="text-xs text-muted">{t('No comments yet.')}</p>{/each}
        </div>
        <form
          class="mt-3 grid gap-2"
          onsubmit={(event) => {
            event.preventDefault();
            void submitComment();
          }}
        >
          <textarea
            class="min-h-20 rounded border border-line bg-[#09111a] p-3 text-xs text-white"
            bind:value={comment}
            placeholder={t('Add a comment')}
            required></textarea><button
            class="justify-self-end rounded border border-[#24505b] bg-[#0d2830] px-3 py-2 text-xs text-brand-cyan"
            disabled={actionBusy === 'ticket-comment'}>{t('Send')}</button
          >
        </form>
      </div>
      <div class="grid grid-cols-2 gap-2 text-[10px] text-muted">
        <span
          >{t('Response due')}<strong class="block text-white"
            >{relativeDate(selected.response_due_at)}</strong
          ></span
        ><span
          >{t('Resolution due')}<strong class="block text-white"
            >{relativeDate(selected.resolution_due_at)}</strong
          ></span
        >
      </div>
    {:else}<div class="grid min-h-80 place-items-center text-center text-muted">
        {t('Select a ticket to inspect its lifecycle.')}
      </div>{/if}
  </aside>
</div>
