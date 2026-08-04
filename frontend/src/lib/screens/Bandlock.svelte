<script lang="ts">
  import { rpc } from '../bridge';
  import { ALL_LTE_BANDS, ALL_NR_BANDS, toggleSetItem } from '../helpers';
  import { Radio, RefreshCw, AlertTriangle, ShieldCheck, CheckSquare, Square } from 'lucide-svelte';

  interface BandlockGetResult {
    ok: boolean;
    error?: string;
    errors?: Record<string, string>;
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

  interface RollbackEntry {
    path: string;
    action: string;
    exit: number;
    verified: boolean;
  }

  interface RollbackInfo {
    attempted: boolean;
    verified: boolean;
    entries?: RollbackEntry[];
  }

  interface BandlockSetResult {
    ok: boolean;
    error?: string;
    rollback?: RollbackInfo;
    verified?: Record<string, { bytes: string; match: boolean }>;
  }

  let slot = $state<number>(0);
  let activeTab = $state<'detected' | 'manual'>('detected');
  let loading = $state<boolean>(false);
  let statusMsg = $state<string | null>(null);
  let getResult = $state<BandlockGetResult | null>(null);

  // Band selections
  let selectedLte = $state<Set<number>>(new Set());
  let selectedNrNsa = $state<Set<number>>(new Set());
  let selectedNrSa = $state<Set<number>>(new Set());

  // Track explicit user zero-cleared state
  let clearedLte = $state<boolean>(false);
  let clearedNrNsa = $state<boolean>(false);
  let clearedNrSa = $state<boolean>(false);

  // Original NV state for diffing
  let originalLte = $state<Set<number>>(new Set());
  let originalNrNsa = $state<Set<number>>(new Set());
  let originalNrSa = $state<Set<number>>(new Set());

  // Bytes previews
  let currentBytes = $state<Record<string, string>>({});

  // Confirmation dialog state
  let showConfirm = $state<boolean>(false);
  let confirmStep = $state<number>(1);
  let showEmptyWarningModal = $state<boolean>(false);
  let clearedCategoriesWarning = $state<string[]>([]);
  let applying = $state<boolean>(false);
  let applyProgress = $state<string>('');
  let lastSetRollback = $state<RollbackInfo | null>(null);
  let hasChanges = $derived(
    selectedLte.size !== originalLte.size ||
    selectedNrNsa.size !== originalNrNsa.size ||
    selectedNrSa.size !== originalNrSa.size ||
    clearedLte || clearedNrNsa || clearedNrSa ||
    Array.from(selectedLte).some(b => !originalLte.has(b)) ||
    Array.from(selectedNrNsa).some(b => !originalNrNsa.has(b)) ||
    Array.from(selectedNrSa).some(b => !originalNrSa.has(b))
  );

  async function loadBands() {
    loading = true;
    statusMsg = null;
    lastSetRollback = null;
    clearedLte = false;
    clearedNrNsa = false;
    clearedNrSa = false;
    try {
      const getRes = await rpc('bandlock.get', { slot }) as BandlockGetResult;
      getResult = getRes;
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
      if (getRes && getRes.ok === false) {
        const errList = getRes.errors ? Object.entries(getRes.errors).map(([p, e]) => `${p}: ${e}`).join('; ') : (getRes.error || 'Failed to read modem NV');
        statusMsg = `Failed to read modem NV: ${errList}`;
      }
    } catch (e: unknown) {
      getResult = { ok: false, error: e instanceof Error ? e.message : String(e) };
      statusMsg = `Failed to read modem NV: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      loading = false;
    }
  }

  async function detectBands() {
    loading = true;
    statusMsg = null;
    try {
      const res = await rpc('bandlock.detect', { slot }) as { ok: boolean; bands?: { lte?: number[]; nrNsa?: number[]; nrSa?: number[] }; raw_byte_count?: number };
      if (res && res.ok) {
        selectedLte = new Set(res.bands?.lte || []);
        selectedNrNsa = new Set(res.bands?.nrNsa || []);
        selectedNrSa = new Set(res.bands?.nrSa || []);
        statusMsg = `DIAG Auto-detection complete (${res.raw_byte_count || 0} bytes scanned).`;
      }
    } catch (e: unknown) {
      statusMsg = `Detection error: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      loading = false;
    }
  }

  function selectAll(type: 'lte' | 'nrNsa' | 'nrSa') {
    if (type === 'lte') { selectedLte = new Set(ALL_LTE_BANDS); clearedLte = false; }
    else if (type === 'nrNsa') { selectedNrNsa = new Set(ALL_NR_BANDS); clearedNrNsa = false; }
    else if (type === 'nrSa') { selectedNrSa = new Set(ALL_NR_BANDS); clearedNrSa = false; }
  }

  function selectNone(type: 'lte' | 'nrNsa' | 'nrSa') {
    if (type === 'lte') { selectedLte = new Set(); clearedLte = true; }
    else if (type === 'nrNsa') { selectedNrNsa = new Set(); clearedNrNsa = true; }
    else if (type === 'nrSa') { selectedNrSa = new Set(); clearedNrSa = true; }
  }

  function toggleBand(type: 'lte' | 'nrNsa' | 'nrSa', band: number) {
    if (type === 'lte') {
      selectedLte = toggleSetItem(selectedLte, band);
      clearedLte = selectedLte.size === 0;
    } else if (type === 'nrNsa') {
      selectedNrNsa = toggleSetItem(selectedNrNsa, band);
      clearedNrNsa = selectedNrNsa.size === 0;
    } else if (type === 'nrSa') {
      selectedNrSa = toggleSetItem(selectedNrSa, band);
      clearedNrSa = selectedNrSa.size === 0;
    }
  }

  function checkEmptyCategoriesAndProceed() {
    const emptyCleared: string[] = [];
    if (selectedLte.size === 0 && clearedLte) emptyCleared.push('LTE');
    if (selectedNrNsa.size === 0 && clearedNrNsa) emptyCleared.push('NR NSA');
    if (selectedNrSa.size === 0 && clearedNrSa) emptyCleared.push('NR SA');

    if (emptyCleared.length > 0) {
      clearedCategoriesWarning = emptyCleared;
      showEmptyWarningModal = true;
    } else {
      openApplyDialog();
    }
  }

  function openApplyDialog() {
    showEmptyWarningModal = false;
    confirmStep = 1;
    showConfirm = true;
  }

  async function handleApply() {
    applying = true;
    applyProgress = 'Writing modem NV band masks and backing up EFS...';
    lastSetRollback = null;
    try {
      const params: Record<string, unknown> = { slot };
      let hasAllowEmpty = false;

      if (selectedLte.size > 0) {
        params.lte = Array.from(selectedLte).join(',');
      } else if (clearedLte) {
        params.lte = '';
        hasAllowEmpty = true;
      }

      if (selectedNrNsa.size > 0) {
        params.nrNsa = Array.from(selectedNrNsa).join(',');
      } else if (clearedNrNsa) {
        params.nrNsa = '';
        hasAllowEmpty = true;
      }

      if (selectedNrSa.size > 0) {
        params.nrSa = Array.from(selectedNrSa).join(',');
      } else if (clearedNrSa) {
        params.nrSa = '';
        hasAllowEmpty = true;
      }

      if (hasAllowEmpty) {
        params.allowEmpty = true;
      }

      const res = await rpc('bandlock.set', params) as BandlockSetResult;

      if (res && res.ok) {
        const verifiedMap = res.verified || {};
        const verifiedEntries = Object.values(verifiedMap);
        const allVerified = verifiedEntries.length > 0 && verifiedEntries.every(v => v.match === true);
        if (allVerified) {
          statusMsg = 'Bandlock NV write verified and applied successfully!';
        } else {
          statusMsg = 'Bandlock NV written but read-back verification FAILED — check Backups and restore if needed';
        }
        showConfirm = false;
        await loadBands();
      } else if (res && res.ok === false) {
        let msg = res.error || 'Failed to set bandlock NV';
        if (res.rollback) {
          msg += ' (rolled back)';
          lastSetRollback = res.rollback;
        }
        statusMsg = msg;
        showConfirm = false;
      }
    } catch (e: unknown) {
      statusMsg = `Apply error: ${e instanceof Error ? e.message : String(e)}`;
      showConfirm = false;
    } finally {
      applying = false;
    }
  }


  async function handleRestartModem() {
    if (!confirm('Restart modem hardware now? Cellular connection will reset.')) return;
    try {
      await rpc('modem.restart');
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

  {#if getResult && getResult.ok === false}
    <div class="card status-err-card">
      <strong style="color: var(--danger);">Bandlocking Disabled: Read Modem NV Failed</strong>
      {#if getResult.errors}
        <ul style="margin-top: 6px; font-size: 12px; color: var(--danger); padding-left: 18px;">
          {#each Object.entries(getResult.errors) as [p, err]}
            <li><span class="mono">{p}</span>: {err}</li>
          {/each}
        </ul>
      {:else if getResult.error}
        <p style="font-size: 12px; color: var(--danger); margin-top: 4px;">{getResult.error}</p>
      {/if}
    </div>
  {/if}

  {#if lastSetRollback && lastSetRollback.entries}
    <div class="card status-err-card">
      <strong style="color: var(--danger);">Rollback Verification Details</strong>
      <div style="display: flex; flex-direction: column; gap: 4px; margin-top: 6px;">
        {#each lastSetRollback.entries as entry}
          <div class="caption mono" style="display: flex; justify-content: space-between;">
            <span>[{entry.action.toUpperCase()}] {entry.path}</span>
            <span style={`color: ${entry.verified ? 'var(--success)' : 'var(--danger)'}`}>
              {entry.verified ? 'Verified OK' : 'Verified FALSE (Rollback Failed)'}
            </span>
          </div>
        {/each}
      </div>
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
    <button
      class="btn btn-primary"
      onclick={checkEmptyCategoriesAndProceed}
      disabled={loading || !hasChanges || (getResult !== null && getResult.ok === false)}
    >
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
  <!-- Dedicated Confirm Modal for Cleared Category (Zero Bands Selected) -->
  {#if showEmptyWarningModal}
    <div class="overlay">
      <div class="dialog">
        <div class="dialog-header">
          <h2 style="color: var(--danger);">Disable RAT Warning</h2>
          <p class="caption">Explicit Zero Band Mask Selection</p>
        </div>
        <div class="danger-zone banner">
          <AlertTriangle size={24} style="color: var(--danger);" />
          <div>
            <strong style="color: var(--danger);">ALL bands for {clearedCategoriesWarning.join(', ')} will be DISABLED!</strong>
            <p class="caption" style="margin-top: 4px;">
              You explicitly unselected all bands in {clearedCategoriesWarning.join(', ')}. Sending an empty band mask disables connection on these Radio Access Technologies entirely.
            </p>
          </div>
        </div>
        <div class="dialog-actions">
          <button class="btn btn-secondary" onclick={() => showEmptyWarningModal = false}>Cancel</button>
          <button class="btn btn-danger" onclick={openApplyDialog}>
            I Understand, Proceed to Apply
          </button>
        </div>
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
