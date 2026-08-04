<script lang="ts">
  import { rpc, bridgeStatus, ApiError } from '../bridge';
  import { RefreshCw, Sliders, CheckCircle2, XCircle, HelpCircle, AlertOctagon } from 'lucide-svelte';

  interface FeatureWriteResult {
    path: string;
    verified: boolean;
  }

  interface FeatureItem {
    id: string;
    label: string;
    status: 'enabled' | 'disabled' | 'absent' | 'error';
    writes?: FeatureWriteResult[];
    paths: Array<{
      path: string;
      absent: boolean;
      bytes: string;
    }>;
  }

  interface RollbackInfo {
    attempted: boolean;
    verified: boolean;
    entries?: Array<{ path: string; action: string; exit: number; verified: boolean }>;
  }

  interface FeatureRpcResult {
    ok: boolean;
    error?: string;
    rollback?: RollbackInfo;
    writes?: FeatureWriteResult[];
  }
  let slot = $state<number>(0);
  let loading = $state<boolean>(false);
  let features = $state<FeatureItem[]>([]);
  let errorMsg = $state<string | null>(null);

  // NR mode selector: nr5g_disable_mode (0 = SA+NSA, 1 = NSA only, 2 = SA only)
  // SAFETY: tapping an option only changes a PENDING selection — the modem
  // is never written from the option onclick handler.
  let nrMode = $state<number>(0);       // current (read from modem)
  let nrPending = $state<number | null>(null); // local pending selection
  let nrModeLoading = $state<boolean>(false);
  let nrModeMsg = $state<string | null>(null);
  const NR_LABELS = ['SA + NSA (Both)', 'NSA Only', 'SA Only'];

  const NR_MODE_PATH = '/nv/item_files/modem/mmode/nr5g_disable_mode';

  async function checkFeatures() {
    loading = true;
    errorMsg = null;
    try {
      let res: { ok: boolean; error?: string; features?: FeatureItem[]; failed_paths?: { path: string; error: string }[] };
      try {
        res = await rpc('features.check', { slot }) as typeof res;
      } catch (e: unknown) {
        // ok:false responses arrive as ApiError with the parsed payload —
        // render the partial results instead of discarding them.
        const payload = e instanceof ApiError ? e.payload as typeof res | undefined : undefined;
        if (payload?.features) {
          features = payload.features;
          const fails = (payload.failed_paths || []).length;
          errorMsg = `Feature check failed on ${fails} NV read(s) (modem/QMI error) — partial results shown. ${payload.error || ''}`;
          return;
        }
        errorMsg = e instanceof Error ? e.message : String(e);
        return;
      }
      if (res && res.features) {
        features = res.features;
      }
      if (res && res.ok === false) {
        const fails = (res.failed_paths || []).length;
        errorMsg = `Feature check failed on ${fails} NV read(s) (modem/QMI error) — statuses are marked. ${res.error || ''}`;
      }
    } finally {
      loading = false;
    }
  }

  async function toggleFeature(feat: FeatureItem) {
    loading = true;
    errorMsg = null;
    try {
      const method = feat.status === 'enabled' ? 'features.disable' : 'features.restore';
      const res = await rpc(method, { id: feat.id, slot }) as FeatureRpcResult;
      if (res && res.ok) {
        await checkFeatures();
      } else if (res && res.ok === false) {
        let msg = res.error || `Failed to ${feat.status === 'enabled' ? 'disable' : 'restore'} feature ${feat.label}`;
        if (res.rollback) msg += ' (rolled back)';
        errorMsg = msg;
      }
    } catch (e: unknown) {
      errorMsg = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function readNrMode() {
    nrModeLoading = true;
    nrModeMsg = null;
    try {
      const res = await rpc('nv.read', { path: NR_MODE_PATH, slot }) as { ok: boolean; bytes?: string; absent?: boolean };
      if (res && res.bytes) {
        const val = parseInt(res.bytes, 16);
        if (!isNaN(val)) {
          nrMode = val;
          nrPending = null;
        }
      }
    } catch (e: unknown) {
      nrModeMsg = `Read error: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      nrModeLoading = false;
    }
  }

  function selectNrMode(idx: number) {
    nrPending = idx; // local only — no modem write
  }

  async function applyNrMode() {
    if (nrPending === null) return;
    const target = nrPending;
    nrModeLoading = true;
    nrModeMsg = null;
    try {
      const hexVal = target.toString(16).padStart(2, '0');
      const res = await rpc('nv.write', { path: NR_MODE_PATH, hex: hexVal, slot, reason: 'NR mode set' }) as {
        ok: boolean; verified?: boolean; backup?: { id?: string }; rollback?: { attempted?: boolean; verified?: boolean };
      };
      nrPending = null;
      if (res && res.ok && res.verified) {
        nrModeMsg = 'NR mode applied and verified — re-reading to confirm...';
      } else if (res && res.verified === false) {
        nrModeMsg = `NR mode written but read-back verification FAILED (backup ${res.backup?.id || '?'}) — check Backups`;
      }
    } catch (e: unknown) {
      nrPending = null;
      const err = e instanceof Error ? e : null;
      const payload = err instanceof ApiError ? err.payload as {
        ok?: boolean; error?: string; backup?: { id?: string }; verified?: boolean;
        rollback?: { attempted?: boolean; verified?: boolean };
      } | undefined : undefined;
      if (payload) {
        const rb = payload.rollback;
        if (payload.backup || payload.verified === false || rb) {
          // the write WAS attempted — verification/rollback reporting
          nrModeMsg = `Apply attempted: ${payload.error || 'verification failed'}` +
            (rb ? ` — rollback attempted ${rb.attempted === true ? 'yes' : 'no'}, verified ${rb.verified === true ? 'yes' : 'NO'}` : '') +
            (payload.backup ? ` (backup ${payload.backup.id})` : '');
        } else {
          // write never started (read-before-write / backup creation failure)
          nrModeMsg = `Apply failed (nothing written): ${payload.error || (err ? err.message : 'unknown error')}`;
        }
      } else {
        nrModeMsg = `Apply failed (nothing written): ${err ? err.message : String(e)}`;
      }
    } finally {
      // never assume the modem state: re-read live and reconcile nrMode
      nrModeLoading = false;
      await readNrMode();
    }
  }

  $effect(() => {
    checkFeatures();
    readNrMode();
  });
</script>

<div class="features-screen">
  <!-- Screen Header -->
  <div class="screen-header">
    <div>
      <h1 class="screen-title">Modem Features</h1>
      <p class="screen-subtitle">Hardware Capabilities & NR Mode Controls</p>
    </div>
    <div class="header-right">
      <select class="select slot-select" bind:value={slot} onchange={() => { checkFeatures(); readNrMode(); }}>
        <option value={0}>SIM 0</option>
        <option value={1}>SIM 1</option>
      </select>
      <button class="btn btn-secondary" onclick={checkFeatures} disabled={loading}>
        <RefreshCw size={16} class={loading ? 'spin' : ''} /> Check
      </button>
    </div>
  </div>

  {#if errorMsg}
    <div class="card status-err-card">{errorMsg}</div>
  {/if}

  <!-- NR Mode Selector Card -->
  <div class="card mode-card">
    <div class="mode-header">
      <Sliders size={20} class="accent-icon" />
      <div>
        <strong style="color: var(--text-primary);">NR 5G Mode Selector</strong>
        <p class="caption">Modifies <code>nr5g_disable_mode</code> byte at <span class="mono">{NR_MODE_PATH}</span></p>
      </div>
    </div>
    <!-- Selecting here ONLY sets a pending value — nothing is written. -->
    <div class="caption" style="margin-bottom: 6px;">
      Current: <strong>{NR_LABELS[nrMode]}</strong>
      {#if nrPending !== null && nrPending !== nrMode}
        &nbsp;→&nbsp;Pending: <strong style="color: var(--warning);">{NR_LABELS[nrPending]}</strong>
      {/if}
    </div>
    <div class="segmented-control">
      {#each NR_LABELS as label, i}
        <button
          class={`segmented-tab ${(nrPending ?? nrMode) === i ? 'active' : ''}`}
          onclick={() => selectNrMode(i)}
          disabled={nrModeLoading}
        >
          {label}
        </button>
      {/each}
    </div>
    {#if nrPending !== null && nrPending !== nrMode}
      <div class="card" style="margin-top: 10px;">
        <div class="caption" style="color: var(--text-secondary);">
          Target byte: <span class="mono">{nrPending.toString(16).padStart(2, '0')}</span>
          (path <span class="mono">{NR_MODE_PATH}</span>, slot {slot}) — a backup is created before writing.
        </div>
        <button
          class="btn btn-primary"
          style="margin-top: 8px;"
          onclick={applyNrMode}
          disabled={nrModeLoading || !$bridgeStatus.ready}
        >
          {nrModeLoading ? 'Applying...' : 'Preview & Apply 5G Mode Change'}
        </button>
      </div>
    {/if}
    {#if nrModeMsg}
      <div class="caption" style="color: var(--primary);">{nrModeMsg}</div>
    {/if}
  </div>

  <!-- Feature List -->
  <div class="section-label">12 EMBEDDED FEATURE CONTROLS</div>
  
  {#if features.length === 0 && loading}
    <div class="card caption">Checking modem feature NV items...</div>
  {:else}
    <div class="features-list">
      {#each features as feat}
        <div class="card feat-card">
          <div class="feat-info">
            <div class="feat-title">
              <strong>{feat.label}</strong>
              {#if feat.status === 'enabled'}
                <span class="chip status-ok"><CheckCircle2 size={12} /> Enabled</span>
              {:else if feat.status === 'disabled'}
                <span class="chip status-warn"><XCircle size={12} /> Disabled</span>
              {:else if feat.status === 'absent'}
                <span class="chip status-info"><HelpCircle size={12} /> Absent</span>
              {:else}
                <span class="chip status-err"><AlertOctagon size={12} /> Error</span>
              {/if}
            </div>
            <div class="mono caption">ID: {feat.id}</div>
          </div>
          <button
            class={`btn ${feat.status === 'disabled' ? 'btn-primary' : 'btn-secondary'}`}
            onclick={() => toggleFeature(feat)}
            disabled={loading || feat.status === 'absent' || feat.status === 'error' || !$bridgeStatus.ready}
          >
            {feat.status === 'disabled' ? 'Restore' : 'Disable'}
          </button>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .features-screen {
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
  .header-right {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .slot-select {
    width: 100px;
  }
  .mode-card {
    display: flex;
    flex-direction: column;
    gap: 14px;
    border-color: var(--primary);
  }
  .mode-header {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .accent-icon {
    color: var(--primary);
  }
  .section-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-muted);
    letter-spacing: 0.4px;
  }
  .features-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .feat-card {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .feat-info {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .feat-title {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 15px;
    color: var(--text-primary);
  }
  .status-err-card {
    border-color: var(--danger);
    background-color: rgba(255, 97, 97, 0.05);
  }
  .caption {
    font-size: 12px;
    color: var(--text-muted);
  }
</style>
