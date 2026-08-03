<script lang="ts">
  import { api } from '../bridge';
  import { ShieldAlert, RefreshCw, Archive, Cpu, Smartphone, Folder, Activity, Radio, Sliders, Search, Zap } from 'lucide-svelte';

  interface ProbeResult {
    ok: boolean;
    mtb_path?: string;
    mtb_exists?: boolean;
    mtb_executable?: boolean;
    mtbctl_version?: string;
    model?: string;
    android_sdk?: number | string;
    data_dir?: string;
    error?: string;
  }

  interface BackupItem {
    id: string;
    time: number;
    reason: string;
    entries: unknown[];
  }

  let { onNavigate }: { onNavigate: (screen: string) => void } = $props();

  let loading = $state(true);
  let probe = $state<ProbeResult | null>(null);
  let latestBackup = $state<BackupItem | null>(null);
  let error = $state<string | null>(null);
  let restartingModem = $state(false);
  let modemResult = $state<string | null>(null);

  async function loadDashboard() {
    loading = true;
    error = null;
    try {
      const res = await api('probe') as ProbeResult;
      probe = res;

      const bRes = await api('backup list') as { ok: boolean; backups?: BackupItem[] };
      if (bRes && bRes.backups && bRes.backups.length > 0) {
        latestBackup = bRes.backups[0];
      } else {
        latestBackup = null;
      }
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function handleModemRestart() {
    if (!confirm('Are you sure you want to restart the modem? Data connectivity will be lost temporarily.')) {
      return;
    }
    restartingModem = true;
    modemResult = null;
    try {
      const res = await api('modem restart') as { ok: boolean; exit?: number };
      modemResult = res.ok ? 'Modem restart initiated successfully.' : 'Failed to restart modem.';
    } catch (e: unknown) {
      modemResult = e instanceof Error ? e.message : String(e);
    } finally {
      restartingModem = false;
    }
  }

  $effect(() => {
    loadDashboard();
  });
</script>

<div class="dashboard-screen">
  <div class="screen-header">
    <div>
      <h1 class="screen-title">MTB Control</h1>
      <p class="screen-subtitle">POCO F6 Modem & NV Management System</p>
    </div>
    <button class="btn btn-secondary" onclick={loadDashboard} disabled={loading}>
      <RefreshCw size={16} class={loading ? 'spin' : ''} /> Refresh
    </button>
  </div>

  <div class="danger-zone banner">
    <ShieldAlert size={20} class="banner-icon" />
    <div>
      <strong style="color: var(--danger)">Danger Notice</strong>
      <p style="font-size: 13px; color: var(--text-secondary); margin-top: 2px;">
        Modem NV writes carry permanent risk. Always verify backup status before making changes.
      </p>
    </div>
  </div>

  {#if error}
    <div class="card status-err-card">
      <p style="color: var(--danger);">Failed to probe device: {error}</p>
    </div>
  {/if}

  {#if loading && !probe}
    <div class="card">
      <p style="color: var(--text-muted);">Probing MTB binary and environment...</p>
    </div>
  {:else if probe}
    <div class="section-label">MODEM & SYSTEM STATUS</div>
    <div class="card-grid">
      <div class="card info-card">
        <div class="info-header"><Cpu size={16} /> Device Model</div>
        <div class="info-val">{probe.model || 'Unknown'}</div>
      </div>
      <div class="card info-card">
        <div class="info-header"><Smartphone size={16} /> Android SDK</div>
        <div class="info-val">{probe.android_sdk || 'Unknown'}</div>
      </div>
      <div class="card info-card">
        <div class="info-header"><Activity size={16} /> MTB Binary</div>
        <div class="info-val">
          <span class={`chip ${probe.mtb_exists ? 'status-ok' : 'status-err'}`}>
            {probe.mtb_exists ? 'Present' : 'Missing'}
          </span>
          <span class="mono caption">{probe.mtb_path || '/vendor/bin/mtb'}</span>
        </div>
      </div>
      <div class="card info-card">
        <div class="info-header"><Folder size={16} /> Data Directory</div>
        <div class="info-val">
          <span class="mono caption">{probe.data_dir || '/data/adb/mtbtool'}</span>
        </div>
      </div>
    </div>
  {/if}

  <div class="section-label">QUICK NAVIGATION</div>
  <div class="nav-grid">
    <button class="card card-interactive nav-card" onclick={() => onNavigate('bandlock')}>
      <Radio size={20} class="accent-icon" />
      <div>
        <strong>Band Locking</strong>
        <p class="caption">Configure LTE & NR band masks</p>
      </div>
    </button>
    <button class="card card-interactive nav-card" onclick={() => onNavigate('features')}>
      <Sliders size={20} class="accent-icon" />
      <div>
        <strong>Feature Toggles</strong>
        <p class="caption">R17 2T2T, UL-MIMO, NR-CA & NR Mode</p>
      </div>
    </button>
    <button class="card card-interactive nav-card" onclick={() => onNavigate('nvimport')}>
      <Search size={20} class="accent-icon" />
      <div>
        <strong>NV Explorer & Import</strong>
        <p class="caption">Read single items & bulk import JSON</p>
      </div>
    </button>
    <button class="card card-interactive nav-card" onclick={() => onNavigate('cells')}>
      <Zap size={20} class="accent-icon" />
      <div>
        <strong>Diagnostic Cells</strong>
        <p class="caption">Real-time LTE/NR RSRP signal monitor</p>
      </div>
    </button>
  </div>

  <div class="section-label">QUICK ACTIONS</div>
  <div class="card action-card-bar">
    <button class="btn btn-primary" onclick={handleModemRestart} disabled={restartingModem}>
      <RefreshCw size={16} class={restartingModem ? 'spin' : ''} /> Restart Modem
    </button>
    <button class="btn btn-secondary" onclick={() => onNavigate('backups')}>
      <Archive size={16} /> Open Backups
    </button>
  </div>
  {#if modemResult}
    <div class="card" style="color: var(--primary);">{modemResult}</div>
  {/if}

  <div class="section-label">LATEST BACKUP SUMMARY</div>
  <div class="card">
    {#if latestBackup}
      <div class="backup-summary-box">
        <div>
          <strong style="color: var(--text-primary);">Reason:</strong> <span style="color: var(--text-secondary);">{latestBackup.reason}</span>
        </div>
        <div class="mono caption" style="margin-top: 4px;">
          ID: {latestBackup.id} | Date: {new Date(latestBackup.time * 1000).toLocaleString()} | Entries: {latestBackup.entries?.length || 0}
        </div>
      </div>
    {:else}
      <p class="caption">No backups recorded yet.</p>
    {/if}
  </div>
</div>

<style>
  .dashboard-screen {
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
  .danger-zone.banner {
    display: flex;
    gap: 12px;
    align-items: center;
    padding: 14px 16px;
  }
  .section-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-muted);
    letter-spacing: 0.4px;
    margin-top: 8px;
  }
  .card-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 12px;
  }
  .info-card {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .info-header {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    color: var(--text-muted);
  }
  .info-val {
    font-size: 15px;
    font-weight: 600;
    color: var(--text-primary);
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .nav-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(220px, 1fr));
    gap: 12px;
  }
  .nav-card {
    display: flex;
    align-items: center;
    gap: 14px;
    text-align: left;
    cursor: pointer;
    background-color: var(--surface-1);
    border: 1px solid var(--border);
  }
  .accent-icon {
    color: var(--primary);
  }
  .caption {
    font-size: 12px;
    color: var(--text-muted);
  }
  .action-card-bar {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
  }
  .status-err-card {
    border-color: var(--danger);
    background-color: rgba(255, 97, 97, 0.05);
  }
  .backup-summary-box {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  :global(.spin) {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
