<script lang="ts">
  import { api } from '../bridge';
  import { ALL_LTE_BANDS, ALL_NR_BANDS, toggleSetItem } from '../helpers';
  import { Radio, RefreshCw, AlertTriangle, ShieldCheck, CheckSquare, Square } from 'lucide-svelte';

  interface BandlockGetResult {
    ok: boolean;
    paths?: {
      ltePrimary: string;
      lteExtension: string;
      nrNsa: string;
      nr: string;
    };
    bytes?: {
      ltePrimary?: string;
      lteExtension?: string;
      nrNsa?: string;
      nr?: string;
    };
    bands?: {
      lte: number[];
      nrNsa: number[];
      nrSa: number[];
    };
  }

  let slot = $state<number>(0);
  let activeTab = $state<'detected' | 'manual'>('detected');
  let loading = $state<boolean>(false);
  let statusMsg = $state<string | null>(null);

  // Band selections
  let selectedLte = $state<Set<number>>(new Set());
  let selectedNrNsa = $state<Set<number>>(new Set());
  let selectedNrSa = $state<Set<number>>(new Set());

  // Original NV state for diffing
  let originalLte = $state<Set<number>>(new Set());
  let originalNrNsa = $state<Set<number>>(new Set());
  let originalNrSa = $state<Set<number>>(new Set());

  // Bytes previews
  let currentBytes = $state<Record<string, string>>({});

  // Confirmation dialog state
  let showConfirm = $state<boolean>(false);
  let confirmStep = $state<number>(1);
  let applying = $state<boolean>(false);
  let applyProgress = $state<string>('');

  let hasChanges = $derived(
    selectedLte.size !== originalLte.size ||
    selectedNrNsa.size !== originalNrNsa.size ||
    selectedNrSa.size !== originalNrSa.size ||
    Array.from(selectedLte).some(b => !originalLte.has(b)) ||
    Array.from(selectedNrNsa).some(b => !originalNrNsa.has(b)) ||
    Array.from(selectedNrSa).some(b => !originalNrSa.has(b))
  );

  async function loadBands() {
    loading = true;
    statusMsg = null;
    try {
      const getRes = await api('bandlock get', { slot: String(slot) }) as BandlockGetResult;
      if (getRes && getRes.bands) {
        selectedLte = new Set(getRes.bands.lte || []);
        selectedNrNsa = new Set(getRes.bands.nrNsa || []);
        selectedNrSa = new Set(getRes.bands.nrSa || []);

        originalLte = new Set(getRes.bands.lte || []);
        originalNrNsa = new Set(getRes.bands.nrNsa || []);
        originalNrSa = new Set(getRes.bands.nrSa || []);
      }
      if (getRes && getRes.bytes) {
        currentBytes = {
          '/nv/item_files/modem/mmode/lte_bandpref': getRes.bytes.ltePrimary || '',
          '/nv/item_files/modem/mmode/lte_bandpref_extn_65_256': getRes.bytes.lteExtension || '',
          '/nv/item_files/modem/mmode/nr_nsa_band_pref': getRes.bytes.nrNsa || '',
          '/nv/item_files/modem/mmode/nr_band_pref': getRes.bytes.nr || ''
        };
      }
    } catch (e: unknown) {
      statusMsg = `Failed to read modem NV: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      loading = false;
    }
  }

  async function detectBands() {
    loading = true;
    statusMsg = null;
    try {
      const res = await api('bandlock detect') as { ok: boolean; lte?: number[]; nrNsa?: number[]; nrSa?: number[]; raw_byte_count?: number };
      if (res && res.ok) {
        selectedLte = new Set(res.lte || []);
        selectedNrNsa = new Set(res.nrNsa || []);
        selectedNrSa = new Set(res.nrSa || []);
        statusMsg = `DIAG Auto-detection complete (${res.raw_byte_count || 0} bytes scanned).`;
      }
    } catch (e: unknown) {
      statusMsg = `Detection error: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      loading = false;
    }
  }

  function selectAll(type: 'lte' | 'nrNsa' | 'nrSa') {
    if (type === 'lte') selectedLte = new Set(ALL_LTE_BANDS);
    else if (type === 'nrNsa') selectedNrNsa = new Set(ALL_NR_BANDS);
    else if (type === 'nrSa') selectedNrSa = new Set(ALL_NR_BANDS);
  }

  function selectNone(type: 'lte' | 'nrNsa' | 'nrSa') {
    if (type === 'lte') selectedLte = new Set();
    else if (type === 'nrNsa') selectedNrNsa = new Set();
    else if (type === 'nrSa') selectedNrSa = new Set();
  }

  function toggleBand(type: 'lte' | 'nrNsa' | 'nrSa', band: number) {
    if (type === 'lte') selectedLte = toggleSetItem(selectedLte, band);
    else if (type === 'nrNsa') selectedNrNsa = toggleSetItem(selectedNrNsa, band);
    else if (type === 'nrSa') selectedNrSa = toggleSetItem(selectedNrSa, band);
  }

  function openApplyDialog() {
    confirmStep = 1;
    showConfirm = true;
  }

  async function handleApply() {
    applying = true;
    applyProgress = 'Generating 4 modem NV bitmasks and backing up EFS...';
    try {
      const lteStr = Array.from(selectedLte).join(',');
      const nrNsaStr = Array.from(selectedNrNsa).join(',');
      const nrSaStr = Array.from(selectedNrSa).join(',');

      const res = await api('bandlock set', {
        slot: String(slot),
        lte: lteStr,
        nrNsa: nrNsaStr,
        nrSa: nrSaStr
      }) as { ok: boolean };

      if (res && res.ok) {
        statusMsg = 'Bandlock NV write verified and applied successfully!';
        showConfirm = false;
        await loadBands();
      }
    } catch (e: unknown) {
      statusMsg = `Apply error: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      applying = false;
    }
  }

  async function handleRestartModem() {
    if (!confirm('Restart modem hardware now? Cellular connection will reset.')) return;
    try {
      await api('modem restart');
      statusMsg = 'Modem restart signal issued successfully.';
    } catch (e: unknown) {
      statusMsg = `Restart error: ${e instanceof Error ? e.message : String(e)}`;
    }
  }

  $effect(() => {
    loadBands();
  });
</script>

<div class="bandlock-screen">
  <!-- Screen Header -->
  <div class="screen-header">
    <div>
      <h1 class="screen-title">Band Locking</h1>
      <p class="screen-subtitle">LTE & NR Band Mask Configuration</p>
    </div>
    <div class="header-right">
      <select class="select slot-select" bind:value={slot} onchange={loadBands}>
        <option value={0}>SIM 0 (Primary)</option>
        <option value={1}>SIM 1 (Secondary)</option>
      </select>
    </div>
  </div>

  <!-- Segmented Control Mode Switcher -->
  <div class="segmented-control">
    <button
      class={`segmented-tab ${activeTab === 'detected' ? 'active' : ''}`}
      onclick={() => { activeTab = 'detected'; detectBands(); }}
    >
      <Radio size={14} /> DIAG Auto-Detected
    </button>
    <button
      class={`segmented-tab ${activeTab === 'manual' ? 'active' : ''}`}
      onclick={() => activeTab = 'manual'}
    >
      Manual Selection
    </button>
  </div>

  <!-- Top Action Toolbar -->
  <div class="card toolbar-card">
    <button class="btn btn-secondary" onclick={detectBands} disabled={loading}>
      <Radio size={16} /> Re-Detect Hardware Bands
    </button>
    <button class="btn btn-secondary" onclick={loadBands} disabled={loading}>
      <RefreshCw size={16} class={loading ? 'spin' : ''} /> Reload On-Modem NV
    </button>
    <button class="btn btn-danger" onclick={handleRestartModem}>
      Restart Modem
    </button>
  </div>

  {#if statusMsg}
    <div class="card status-info-card">
      <span class="chip status-info">STATUS</span>
      <span>{statusMsg}</span>
    </div>
  {/if}

  <!-- LTE Band Grid -->
  <div class="card grid-card">
    <div class="grid-header">
      <div class="section-label">LTE BANDS ({selectedLte.size}/{ALL_LTE_BANDS.length})</div>
      <div class="grid-actions">
        <button class="btn-link" onclick={() => selectAll('lte')}>Select All</button>
        <span class="divider">|</span>
        <button class="btn-link" onclick={() => selectNone('lte')}>Select None</button>
      </div>
    </div>
    <div class="band-grid">
      {#each ALL_LTE_BANDS as band}
        <button
          class={`band-tile ${selectedLte.has(band) ? 'selected' : ''}`}
          onclick={() => toggleBand('lte', band)}
        >
          B{band}
        </button>
      {/each}
    </div>
  </div>

  <!-- NR NSA Band Grid -->
  <div class="card grid-card">
    <div class="grid-header">
      <div class="section-label">NR NSA BANDS ({selectedNrNsa.size}/{ALL_NR_BANDS.length})</div>
      <div class="grid-actions">
        <button class="btn-link" onclick={() => selectAll('nrNsa')}>Select All</button>
        <span class="divider">|</span>
        <button class="btn-link" onclick={() => selectNone('nrNsa')}>Select None</button>
      </div>
    </div>
    <div class="band-grid">
      {#each ALL_NR_BANDS as band}
        <button
          class={`band-tile ${selectedNrNsa.has(band) ? 'selected' : ''}`}
          onclick={() => toggleBand('nrNsa', band)}
        >
          n{band}
        </button>
      {/each}
    </div>
  </div>

  <!-- NR SA Band Grid -->
  <div class="card grid-card">
    <div class="grid-header">
      <div class="section-label">NR SA BANDS ({selectedNrSa.size}/{ALL_NR_BANDS.length})</div>
      <div class="grid-actions">
        <button class="btn-link" onclick={() => selectAll('nrSa')}>Select All</button>
        <span class="divider">|</span>
        <button class="btn-link" onclick={() => selectNone('nrSa')}>Select None</button>
      </div>
    </div>
    <div class="band-grid">
      {#each ALL_NR_BANDS as band}
        <button
          class={`band-tile ${selectedNrSa.has(band) ? 'selected' : ''}`}
          onclick={() => toggleBand('nrSa', band)}
        >
          n{band}
        </button>
      {/each}
    </div>
  </div>

  <!-- Sticky Bottom Action Bar -->
  <div class="sticky-bar">
    <div class="bar-summary">
      <strong>Pending Band Selection</strong>
      <span class="caption">LTE: {selectedLte.size} | NSA: {selectedNrNsa.size} | SA: {selectedNrSa.size}</span>
    </div>
    <button class="btn btn-primary" onclick={openApplyDialog} disabled={loading || !hasChanges}>
      <ShieldCheck size={16} /> Preview & Apply
    </button>
  </div>

  <!-- Two-step Preview & Confirm Modal -->
  {#if showConfirm}
    <div class="overlay">
      <div class="dialog">
        {#if confirmStep === 1}
          <div class="dialog-header">
            <h2>Step 1: Review NV File Path Diffs</h2>
            <p class="caption">Modem NV paths to write for SIM Slot {slot}:</p>
          </div>
          <div class="hex-diff-container">
            {#each Object.entries(currentBytes) as [path, val]}
              <div class="diff-item card">
                <span class="mono caption path-title">{path}</span>
                <div class="hex-compare">
                  <div><span class="caption">Current On-Modem:</span> <span class="mono text-warn">{val || 'None'}</span></div>
                  <div><span class="caption">Proposed New:</span> <span class="mono text-ok">[Calculated 3GPP Bitmask]</span></div>
                </div>
              </div>
            {/each}
          </div>
          <div class="dialog-actions">
            <button class="btn btn-secondary" onclick={() => showConfirm = false}>Cancel</button>
            <button class="btn btn-primary" onclick={() => confirmStep = 2}>Proceed to Step 2 →</button>
          </div>
        {:else}
          <div class="dialog-header">
            <h2>Step 2: Confirm Modem Hardware Write</h2>
          </div>
          <div class="danger-zone banner">
            <AlertTriangle size={20} style="color: var(--danger);" />
            <div>
              <strong style="color: var(--danger);">Modem Lock Invariant</strong>
              <p class="caption" style="margin-top: 2px;">
                EFS items will be backed up automatically before writing. Data connection will temporarily reset.
              </p>
            </div>
          </div>
          {#if applying}
            <div class="card mono caption" style="color: var(--info);">{applyProgress}</div>
          {/if}
          <div class="dialog-actions">
            <button class="btn btn-secondary" onclick={() => confirmStep = 1} disabled={applying}>Back</button>
            <button class="btn btn-danger" onclick={handleApply} disabled={applying}>
              {applying ? 'Writing NV...' : 'Confirm & Apply Write'}
            </button>
          </div>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .bandlock-screen {
    display: flex;
    flex-direction: column;
    gap: 16px;
    padding-bottom: 70px;
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
  .slot-select {
    width: 170px;
  }
  .toolbar-card {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .status-info-card {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
  }
  .grid-card {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .grid-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .section-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-muted);
    letter-spacing: 0.4px;
  }
  .grid-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .btn-link {
    background: transparent;
    border: none;
    color: var(--primary);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
  }
  .divider {
    color: var(--border-strong);
    font-size: 12px;
  }
  .band-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(54px, 1fr));
    gap: 6px;
  }
  .bar-summary {
    display: flex;
    flex-direction: column;
  }
  .bar-summary strong {
    font-size: 14px;
    color: var(--text-primary);
  }
  .overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: var(--overlay);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
    padding: 16px;
  }
  .dialog {
    max-width: 480px;
    width: 100%;
    background-color: var(--surface-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-dialog);
    box-shadow: var(--shadow-dialog);
    padding: 20px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
  .dialog-header h2 {
    font-size: 18px;
    font-weight: 600;
    color: var(--text-primary);
  }
  .hex-diff-container {
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-height: 240px;
    overflow-y: auto;
  }
  .diff-item {
    background-color: var(--surface-1);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .path-title {
    color: var(--text-muted);
    word-break: break-all;
  }
  .hex-compare {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 12px;
  }
  .text-warn { color: var(--warning); }
  .text-ok { color: var(--success); }
  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }
</style>
