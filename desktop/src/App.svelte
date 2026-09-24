<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import * as echarts from 'echarts/core';
  import { BarChart } from 'echarts/charts';
  import { GridComponent, TitleComponent, TooltipComponent } from 'echarts/components';
  import { CanvasRenderer } from 'echarts/renderers';

  echarts.use([BarChart, GridComponent, TitleComponent, TooltipComponent, CanvasRenderer]);

  let chartEl: HTMLDivElement;
  let status = $state('probing…');

  onMount(() => {
    const chart = echarts.init(chartEl);
    const onResize = () => chart.resize();
    window.addEventListener('resize', onResize);

    (async () => {
      const probe = await invoke<{ rows: number; sqliteVersion: string }>('sqlite_probe');
      const sample = await invoke<{ path: string; len: number }>('read_sample_file');
      status = `sqlite ${probe.sqliteVersion} · rows=${probe.rows} · ${sample.path}=${sample.len} B`;
      chart.setOption({
        title: { text: 'M0 probe: sqlite rows vs sample bytes' },
        tooltip: {},
        grid: { left: 48, right: 24, top: 64, bottom: 32 },
        xAxis: { type: 'category', data: ['sqlite rows', 'sample bytes'] },
        yAxis: { type: 'value' },
        series: [{ type: 'bar', data: [probe.rows, sample.len] }]
      });
    })().catch((e) => {
      status = `invoke failed: ${String(e)}`;
    });

    return () => {
      window.removeEventListener('resize', onResize);
      chart.dispose();
    };
  });
</script>

<main>
  <h1><img src="/brand/app-icon.svg" width="48" height="48" alt="" />llm-usage M0</h1>
  <p class="status">{status}</p>
  <div bind:this={chartEl} class="chart"></div>
</main>

<style>
  main {
    padding: 16px 24px;
    font-family: system-ui, sans-serif;
  }
  .status {
    color: #555;
    font-size: 13px;
  }
  h1 {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .chart {
    width: 100%;
    height: 560px;
  }
</style>
