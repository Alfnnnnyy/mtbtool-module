<script lang="ts">
  import { rpc, bridgeStatus } from '../bridge';
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
  let nrMode = $state<number>(0);
  let nrModeLoading = $state<boolean>(false);
  let nrModeMsg = $state<string | null>(null);

  const NR_MODE_PATH = '/nv/item_files/modem/mmode/nr5g_disable_mode';

  async function checkFeatures() {
    loading = true;
    errorMsg = null;
    try {
      const res = await rpc('features.check', { slot }) as { ok: boolean; error?: string; features?: FeatureItem[]; failed_paths?: { path: string; error: string }[] };
      if (res && res.features) {
        features = res.features;
      }
      if (res && res.ok === false) {
        const fails = (res.failed_paths || []).length;
        errorMsg = `Feature check failed on ${fails} NV read(s) (modem/QMI error) — statuses are marked. ${res.error || ''}`;
      }
    } catch (e: unknown) {
      errorMsg = e instanceof Error ? e.message : String(e);
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
        }
      }
    } catch (e: unknown) {
      nrModeMsg = `Read error: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      nrModeLoading = false;
    }
  }

  async function writeNrMode(newMode: number) {
    nrModeLoading = true;
    nrModeMsg = null;
    try {
      const hexVal = newMode.toString(16).padStart(2, '0');
      const res = await rpc('nv.write', { path: NR_MODE_PATH, hex: hexVal, slot, reason: 'NR mode set' }) as { ok: boolean; verified?: boolean };
      nrMode = newMode;
      if (res && res.verified === false) {
        nrModeMsg = 'NR 5G disable mode written but read-back verification failed — check Backups';
      } else {
        nrModeMsg = 'NR 5G disable mode written and verified successfully.';
      }
    } catch (e: unknown) {
      nrModeMsg = `Write error: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      nrModeLoading = false;
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
    <div class="segmented-control">
      <button
        class={`segmented-tab ${nrMode === 0 ? 'active' : ''}`}
        onclick={() => writeNrMode(0)}
        disabled={nrModeLoading || !$bridgeStatus.ready}
      >
        SA + NSA (Both)
      </button>
      <button
        class={`segmented-tab ${nrMode === 1 ? 'active' : ''}`}
        onclick={() => writeNrMode(1)}
        disabled={nrModeLoading || !$bridgeStatus.ready}
      >
        NSA Only
      </button>
      <button
        class={`segmented-tab ${nrMode === 2 ? 'active' : ''}`}
        onclick={() => writeNrMode(2)}
        disabled={nrModeLoading || !$bridgeStatus.ready}
      >
        SA Only
      </button>
    </div>
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
            disabled={loading || feat.status === 'absent' || !$bridgeStatus.ready}
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
