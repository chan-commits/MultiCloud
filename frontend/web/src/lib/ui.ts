const statusTone: Record<string, string> = {
  active: 'border-[#1e5842] bg-[#0e281f] text-[#4ce3a5]',
  succeeded: 'border-[#1e5842] bg-[#0e281f] text-[#4ce3a5]',
  running: 'border-[#1e5842] bg-[#0e281f] text-[#4ce3a5]',
  approved: 'border-[#1e5842] bg-[#0e281f] text-[#4ce3a5]',
  in_sync: 'border-[#1e5842] bg-[#0e281f] text-[#4ce3a5]',
  failed: 'border-[#622133] bg-[#2b111a] text-[#ff7891]',
  error: 'border-[#622133] bg-[#2b111a] text-[#ff7891]',
  drifted: 'border-[#622133] bg-[#2b111a] text-[#ff7891]',
  queued: 'border-[#60451b] bg-[#291f0e] text-[#fbc468]',
  retrying: 'border-[#60451b] bg-[#291f0e] text-[#fbc468]',
  provisioning: 'border-[#60451b] bg-[#291f0e] text-[#fbc468]',
  pending: 'border-[#60451b] bg-[#291f0e] text-[#fbc468]',
  stopped: 'border-[#2b3c49] bg-[#17222c] text-[#8ba3b5]',
  cancelled: 'border-[#2b3c49] bg-[#17222c] text-[#8ba3b5]',
};

export function statusClass(status: string): string {
  return `inline-block rounded-full border px-[7px] py-1 text-[8px] font-black uppercase tracking-[0.09em] ${statusTone[status] ?? 'border-[#283743] bg-[#19232c] text-[#8092a0]'}`;
}

export function providerLogoClass(kind: string, large = false): string {
  const tone =
    {
      cloudflare: 'border-[#6a4324] bg-[#241b12] text-[#ffad52]',
      vultr: 'border-[#264c72] bg-[#101d2a] text-[#6baeff]',
      ovh: 'border-[#4a3f79] bg-[#19152a] text-[#a697ff]',
    }[kind] ?? 'border-[#28566a] bg-[#10232e] text-[#54dbe1]';
  return `grid place-items-center rounded-[3px] border font-black ${large ? 'h-[46px] w-[46px] text-[11px]' : 'h-[30px] w-[30px] text-[9px]'} ${tone}`;
}
