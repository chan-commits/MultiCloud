<script lang="ts">
  let {
    error,
    notice,
    onDismissError,
    onDismissNotice,
  }: {
    error: string;
    notice: string;
    onDismissError: () => void;
    onDismissNotice: () => void;
  } = $props();
</script>

{#if error}
  <div
    class="mx-8 mt-[18px] flex items-center gap-[10px] rounded-[5px] border border-[#60202e] bg-[#2b1118] px-[14px] py-[10px] text-[12px] text-[#ff788e] max-[760px]:mx-[14px] max-[760px]:mt-3"
  >
    <span>!</span>
    <p class="m-0 flex-1">{error}</p>
    <button class="border-0 bg-transparent" onclick={onDismissError}>×</button>
  </div>
{/if}
{#if notice}
  <div
    class="mx-8 mt-[18px] flex items-center gap-[10px] rounded-[5px] border border-[#205846] bg-[#0d211a] px-[14px] py-[10px] text-[12px] text-[#54d7a1] max-[760px]:mx-[14px] max-[760px]:mt-3"
  >
    <span>✓</span>
    <p class="m-0 flex-1">{notice}</p>
    <button class="border-0 bg-transparent" onclick={onDismissNotice}>×</button>
  </div>
{/if}
