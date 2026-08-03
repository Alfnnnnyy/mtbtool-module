<script lang="ts">
  import { api } from '../bridge';
  import { Play, Square, Activity, Zap } from 'lucide-svelte';

  interface CellItem {
    label: string;
    earfcn?: number;
    pci?: number;
    rsrp?: number;
    rsrq?: number;
    rssi?: number;
    snr?: number;
  }

  interface CellsResponse {
    ok: boolean;
    ts?: number;
    lte?: CellItem[];
    nr?: CellItem[];
    tx_power?: number | null;
  }

  let slot = $state(0);
  let pollIntervalSec = $state(2);
  let pollingActive = $state(true);

  let lteCells = $state<CellItem[]>([]);
  let nrCells = $state<CellItem[]>([]);
  let txPower = $state<number | null>(null);
  let lastTimestamp = $state<number | null>(null);
  let errorMsg = $state<string | null>(null);

  // Staleness suppression map: key = label, value = { lastVal: string, count: number }
  const stalenessMap = new Map<string, { lastVal: string; count: number }>();

  function getCellSignature(c: CellItem): string {
    return `${c.earfcn || ''}_${c.pci || ''}_${c.rsrp || ''}_${c.rsrq || ''}`;
  }

  function isCellStale(c: CellItem): boolean {
    const key = c.label || getCellSignature(c);
    const sig = getCellSignature(c);
    const entry = stalenessMap.get(key);
    if (!entry) {
      stalenessMap.set(key, { lastVal: sig, count: 1 });
      return false;
    }
    if (entry.lastVal === sig) {
      entry.count += 1;
      return entry.count >= 3;
    } else {
      stalenessMap.set(key, { lastVal: sig, count: 1 });
      return false;
    }
  }

  function getRsrpColorClass(rsrp?: number): string {
    if (rsrp === undefined || rsrp === null) return 'text-muted';
    if (rsrp > -90) return 'text-ok';
    if (rsrp >= -105) return 'text-warn';
    return 'text-err';
  }

  async function fetchCells() {
    if (document.hidden || !pollingActive) return;
    try {
      const res = await api('cells get', { slot: String(slot) }) as CellsResponse;
      if (res && res.ok) {
        lteCells = res.lte || [];
        nrCells = res.nr || [];
        txPower = res.tx_power !== undefined ? res.tx_power : null;
        lastTimestamp = res.ts || Date.now();
        errorMsg = null;
      }
    } catch (e: unknown) {
      errorMsg = e instanceof Error ? e.message : String(e);
    }
  }

  $effect(() => {
    fetchCells();
    const interval = setInterval(() => {
      fetchCells();
    }, pollIntervalSec * 1000);

    const onVisibilityChange = () => {
      if (!document.hidden && pollingActive) {
        fetchCells();
      }
    };
    document.addEventListener('visibilitychange', onVisibilityChange);

    return () => {
      clearInterval(interval);
      document.removeEventListener('visibilitychange', onVisibilityChange);
    };
  });
</script>

<div class="cells-screen">
  <!-- Screen Header -->
  <div class="screen-header">
    <div>
      <h1 class="screen-title">Cell Monitor</h1>
      <p class="screen-subtitle">Real-Time Signal Quality & Serving Cell Diagnostics</p>
    </div>
    <div class="controls">
      <select class="select slot-select" bind:value={slot}>
        <option value={0}>SIM 0</option>
        <option value={1}>SIM 1</option>
      </select>
      <select class="select rate-select" bind:value={pollIntervalSec}>
        <option value={1}>1s Poll</option>
        <option value={2}>2s Poll</option>
        <option value={5}>5s Poll</option>
        <option value={10}>10s Poll</option>
        <option value={30}>30s Poll</option>
      </select>
      <button
        class={`btn ${pollingActive ? 'btn-danger' : 'btn-primary'}`}
        onclick={() => pollingActive = !pollingActive}
      >
        {#if pollingActive}
          <Square size={16} /> Pause
        {:else}
          <Play size={16} /> Poll
        {/if}
      </button>
    </div>
  </div>

  {#if errorMsg}
    <div class="card status-err-card">{errorMsg}</div>
  {/if}

  <!-- TX Power Header Card -->
  <div class="card tx-card">
    <div class="tx-info">
      <Zap size={20} class="accent-icon" />
      <div>
        <span class="caption">MODEM TX POWER OUTPUT</span>
        <div class="tx-val mono">{txPower !== null ? `${txPower} dBm` : 'N/A / Idle'}</div>
      </div>
    </div>
    <div class="ts-info">
      {#if pollingActive}
        <span class="chip status-info"><span class="pulse-dot"></span> Live Polling ({pollIntervalSec}s)</span>
      {:else}
        <span class="chip status-warn">Polling Paused</span>
      {/if}
    </div>
  </div>

  <!-- LTE Serving & Neighbor Cells -->
  <div class="section-label">LTE SERVING & NEIGHBOR CELLS ({lteCells.length})</div>
  {#if lteCells.length === 0}
    <div class="card caption">No LTE cell signal detected on SIM {slot}.</div>
  {:else}
    <div class="cell-grid">
      {#each lteCells as cell}
        {@const stale = isCellStale(cell)}
        <div class={`card cell-card ${stale ? 'stale' : ''}`}>
          <div class="cell-header">
            <strong style="color: var(--text-primary);">{cell.label}</strong>
            {#if stale}
              <span class="chip status-warn">Stale Data</span>
            {/if}
          </div>
          <div class="metrics-grid">
            <div class="metric"><span class="caption">EARFCN:</span> <span class="mono">{cell.earfcn ?? 'N/A'}</span></div>
            <div class="metric"><span class="caption">PCI:</span> <span class="mono">{cell.pci ?? 'N/A'}</span></div>
            <div class="metric">
              <span class="caption">RSRP:</span>
              <span class={`mono ${getRsrpColorClass(cell.rsrp)}`}>{cell.rsrp ?? 'N/A'} dBm</span>
            </div>
            <div class="metric"><span class="caption">RSRQ:</span> <span class="mono">{cell.rsrq ?? 'N/A'} dB</span></div>
            <div class="metric"><span class="caption">RSSI:</span> <span class="mono">{cell.rssi ?? 'N/A'} dBm</span></div>
            <div class="metric"><span class="caption">SNR:</span> <span class="mono">{cell.snr ?? 'N/A'} dB</span></div>
          </div>
        </div>
      {/each}
    </div>
  {/if}

  <!-- NR Serving & Neighbor Cells -->
  <div class="section-label">5G NR SERVING & NEIGHBOR CELLS ({nrCells.length})</div>
  {#if nrCells.length === 0}
    <div class="card caption">No 5G NR cell signal detected on SIM {slot}.</div>
  {:else}
    <div class="cell-grid">
      {#each nrCells as cell}
        {@const stale = isCellStale(cell)}
        <div class={`card cell-card ${stale ? 'stale' : ''}`}>
          <div class="cell-header">
            <strong style="color: var(--text-primary);">{cell.label}</strong>
            {#if stale}
              <span class="chip status-warn">Stale Data</span>
            {/if}
          </div>
          <div class="metrics-grid">
            <div class="metric">
              <span class="caption">RSRP:</span>
              <span class={`mono ${getRsrpColorClass(cell.rsrp)}`}>{cell.rsrp ?? 'N/A'} dBm</span>
            </div>
            <div class="metric"><span class="caption">RSRQ:</span> <span class="mono">{cell.rsrq ?? 'N/A'} dB</span></div>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .cells-screen {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .screen-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
  }
  .screen-title {
    font-size: 24px;
    font-weight: 600;
    letter-spacing: -0.5px;
    color: var(--text-primary);
  }
  .screen-subtitle {
    font-size: 13px;
    color: var(--text-muted);
  }
  .controls {
    display: flex;
    gap: 8px;
  }
  .slot-select, .rate-select {
    width: 100px;
  }
  .tx-card {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .tx-info {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .accent-icon {
    color: var(--primary);
  }
  .tx-val {
    font-size: 20px;
    font-weight: 600;
    color: var(--primary);
  }
  .section-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-muted);
    letter-spacing: 0.4px;
  }
  .cell-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: 12px;
  }
  .cell-card {
    display: flex;
    flex-direction: column;
    gap: 10px;
    transition: opacity 0.3s ease;
  }
  .cell-card.stale {
    opacity: 0.45;
  }
  .cell-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .metrics-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
    font-size: 13px;
  }
  .status-err-card {
    border-color: var(--danger);
    background-color: rgba(255, 97, 97, 0.05);
  }
  .caption {
    font-size: 12px;
    color: var(--text-muted);
  }
  .text-ok { color: var(--success); }
  .text-warn { color: var(--warning); }
  .text-err { color: var(--danger); }
  .text-muted { color: var(--text-muted); }
  .pulse-dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background-color: var(--info);
    margin-right: 4px;
    animation: pulse 1.5s infinite;
  }
  @keyframes pulse {
    0% { opacity: 1; }
    50% { opacity: 0.3; }
    100% { opacity: 1; }
  }
</style>
