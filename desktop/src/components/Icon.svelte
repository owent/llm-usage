<script lang="ts">
  let { name, size = 20 }: { name: string; size?: number } = $props();
  const paths: Record<string, string> = {
    overview: 'M3 3h7v7H3z M14 3h7v7h-7z M3 14h7v7H3z M14 14h7v7h-7z',
    trend: 'M3 3v18h18 M6 15l4-5 4 3 6-8',
    sources: 'M8 3h8v5H8z M3 16h6v5H3z M15 16h6v5h-6z M12 8v4 M6 16v-4h12v4',
    details: 'M8 3h10l3 3v15H8z M12 9h5 M12 13h5 M12 17h3 M3 7v14',
    settings: 'M4 6h16 M4 12h16 M4 18h16 M8 3v6 M16 9v6 M10 15v6',
    refresh: 'M20 7v5h-5 M4 17v-5h5 M6 6a8 8 0 0113 3 M18 18A8 8 0 015 15',
    shield: 'M12 3l8 3v6c0 5-8 9-8 9s-8-4-8-9V6z M8 12l3 3 5-6',
    arrow: 'M5 12h14 M13 6l6 6-6 6',
  };
</script>

<svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
  <path d={paths[name] ?? paths.overview} />
</svg>
