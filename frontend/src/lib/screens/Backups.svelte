<script lang="ts">
  import { rpc } from '../bridge';
  import { Archive, RefreshCw, AlertTriangle, RotateCcw } from 'lucide-svelte';

  interface BackupEntry {
    slot: number;
    path: string;
    bytes: string | null;
  }

  interface BackupItem {
    id: string;
    time: number;
    reason: string;
    entries: BackupEntry[];
    size?: number;
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

  interface RestoreResult {
    ok: boolean;
    error?: string;
    rollback?: RollbackInfo;
    restored?: Array<{
      slot: number;
      path: string;
      ok: boolean;
      exit: number;
      verified?: boolean;
    }>;
  }

  let loading = $state(false);
  let backups = $state<BackupItem[]>([]);
  let statusMsg = $state<string | null>(null);
  let restoringId = $state<string | null>(null);

  // Emergency Modal Confirm
  let showEmergencyModal = $state(false);
  let emergencyConfirming = $state(false);

  async function loadBackups() {
    loading = true;
    statusMsg = null;
    try {
      const res = await rpc('backup.list') as { ok: boolean; backups?: BackupItem[] };
      if (res && res.backups) {
        backups = res.backups;
      }
    } catch (e: unknown) {
      statusMsg = `Failed to load backups: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      loading = false;
    }
  }

  async function handleRestore(id: string) {
    if (!confirm(`Are you sure you want to restore backup '${id}'? Modem NV items will be rewritten.`)) {
      return;
    }
    restoringId = id;
    statusMsg = null;
    try {
      const res = await rpc('backup.restore', { id }) as RestoreResult;
      const restoredList = res?.restored || [];
      const allOkAndVerified = res && res.ok && restoredList.length > 0 && restoredList.every(r => r.ok && r.verified === true);
      if (allOkAndVerified) {
        statusMsg = `Backup '${id}' successfully restored and verified on modem!`;
      } else {
        let msg = res?.error || 'Restore failed (partial restore or verification error)';
        if (res?.rollback) msg += ' (rolled back)';
        statusMsg = msg;
      }
    } catch (e: unknown) {
      statusMsg = `Restore error: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      restoringId = null;
    }
  }

  async function handleEmergencyRestore() {
    emergencyConfirming = true;
    statusMsg = null;
    try {
      const res = await rpc('backup.restore', { id: 'latest' }) as RestoreResult;
      const restoredList = res?.restored || [];
      const allOkAndVerified = res && res.ok && restoredList.length > 0 && restoredList.every(r => r.ok && r.verified === true);
      if (allOkAndVerified) {
        statusMsg = 'EMERGENCY RESTORE COMPLETE: latest.json backup payload rewritten and verified on modem!';
      } else {
        let msg = res?.error || 'Emergency restore failed (partial restore or verification error)';
        if (res?.rollback) msg += ' (rolled back)';
        statusMsg = msg;
        showEmergencyModal = false;
        await loadBackups();
      }
    } catch (e: unknown) {
      statusMsg = `Emergency restore error: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      emergencyConfirming = false;
    }
  }

  $effect(() => {
    loadBackups();
  });
</script>

<div class="backups-screen">
  <!-- Screen Header -->
  <div class="screen-header">
    <div>
      <h1 class="screen-title">Backups & Recovery</h1>
      <p class="screen-subtitle">EFS Snapshot History & Emergency Modem Restore</p>
    </div>
    <button class="btn btn-secondary" onclick={loadBackups} disabled={loading}>
      <RefreshCw size={16} class={loading ? 'spin' : ''} /> Refresh List
    </button>
  </div>

  {#if statusMsg}
    <div class="card status-info-card">{statusMsg}</div>
  {/if}

  <!-- Backup List -->
  <div class="section-label">RECORDED EFS BACKUP SNAPSHOTS ({backups.length})</div>
  {#if backups.length === 0 && loading}
    <div class="card caption">Loading recorded backups...</div>
  {:else if backups.length === 0}
    <div class="card caption">No EFS backups saved in /data/adb/mtbtool/backups/ yet.</div>
  {:else}
    <div class="backups-list">
      {#each backups as item}
        <div class="card backup-card">
          <div class="backup-info">
            <div class="backup-title">
              <Archive size={16} style="color: var(--primary);" />
              <strong style="color: var(--text-primary);">{item.reason}</strong>
              <span class="chip status-info">{item.entries?.length || 0} entries</span>
            </div>
            <div class="mono caption" style="margin-top: 4px;">
              ID: {item.id} | Timestamp: {new Date(item.time * 1000).toLocaleString()}
            </div>
          </div>
          <button
            class="btn btn-secondary"
            onclick={() => handleRestore(item.id)}
            disabled={restoringId === item.id}
          >
            <RotateCcw size={16} /> {restoringId === item.id ? 'Restoring...' : 'Restore'}
          </button>
        </div>
      {/each}
    </div>
  {/if}

  <!-- Emergency Restore Danger Zone Card -->
  <div class="danger-zone card">
    <div class="danger-header">
      <AlertTriangle size={24} style="color: var(--danger);" />
      <div>
        <strong style="color: var(--danger); font-size: 16px;">EMERGENCY MODEM RESTORE</strong>
        <p class="caption" style="margin-top: 2px;">
          Instantly rewrites <code>latest.json</code> backup payload directly to modem EFS NV storage.
        </p>
      </div>
    </div>
    <button class="btn btn-danger" onclick={() => showEmergencyModal = true}>
      Restore Latest Backup Now
    </button>
  </div>

  <!-- Emergency Restore Modal -->
  {#if showEmergencyModal}
    <div class="overlay">
      <div class="dialog">
        <div class="dialog-header">
          <h2 style="color: var(--danger);">Confirm Emergency Restore</h2>
        </div>
        <div class="danger-zone card">
          <p class="caption">
            This will overwrite all affected modem NV items with the values stored in <code>latest.json</code>. Use this if a bad NV write caused modem instability or loss of network signal.
          </p>
        </div>
        <div class="dialog-actions">
          <button class="btn btn-secondary" onclick={() => showEmergencyModal = false} disabled={emergencyConfirming}>
            Cancel
          </button>
          <button class="btn btn-danger" onclick={handleEmergencyRestore} disabled={emergencyConfirming}>
            {emergencyConfirming ? 'Executing Restore...' : 'Confirm Emergency Restore'}
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .backups-screen {
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
  .section-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-muted);
    letter-spacing: 0.4px;
  }
  .backups-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .backup-card {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .backup-info {
    display: flex;
    flex-direction: column;
  }
  .backup-title {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 15px;
  }
  .danger-zone {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 16px 20px;
  }
  .danger-header {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .status-info-card {
    color: var(--info);
    font-size: 13px;
  }
  .caption {
    font-size: 12px;
    color: var(--text-muted);
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
    max-width: 440px;
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
  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }
</style>
