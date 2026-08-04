<script lang="ts">
  import { rpc, bridgeStatus } from '../bridge';
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
  let creatingSnapshot = $state(false);
  let verifyingId = $state<string | null>(null);
  let verifyResult = $state<string | null>(null);

  // Emergency Modal Confirm
  // resolvedId is FROZEN from the verify response: "latest" is resolved
  // exactly once; restore must never re-resolve it (TOCTOU).
  let restoreReview = $state<{ resolvedId: string; isEmergency: boolean; verify: { ok: boolean; integrity_ok: boolean; all_match: boolean; entries: Array<{ path: string; integrity: boolean; matches_current: boolean; read_error?: string }> } } | null>(null);
  let restoreConfirmText = $state('');
  let restoringId = $state<string | null>(null);

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

  // Read-only snapshot: captures current NV bytes WITHOUT writing anything.
  // peridot-safe preset: nr_nsa_band_pref returns an exit-0 QMI failure on
  // POCO F6/peridot, so it is EXCLUDED from the default snapshot (the
  // backend never silently skips a requested path).
  const SNAPSHOT_PATHS = [
    '/nv/item_files/modem/mmode/lte_bandpref',
    '/nv/item_files/modem/mmode/lte_bandpref_extn_65_256',
    '/nv/item_files/modem/mmode/nr_band_pref',
  ];

  async function handleCreateSnapshot() {
    creatingSnapshot = true;
    statusMsg = null;
    try {
      const res = await rpc('backup.create', { paths: SNAPSHOT_PATHS, slot: 0, reason: 'manual_snapshot' }) as { ok: boolean; error?: string; backup?: BackupItem };
      statusMsg = res?.ok && res.backup
        ? `Snapshot created (read-only): ${res.backup.id}`
        : `Snapshot failed: ${res?.error || 'unknown'}`;
      await loadBackups();
    } catch (e: unknown) {
      statusMsg = `Snapshot error: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      creatingSnapshot = false;
    }
  }

  async function handleVerify(id: string) {
    verifyingId = id;
    verifyResult = null;
    statusMsg = null;
    try {
      const res = await rpc('backup.verify', { id }) as {
        ok: boolean; integrity_ok?: boolean; all_match?: boolean;
        entries?: Array<{ path: string; integrity: boolean; matches_current: boolean; read_error?: string }>;
      };
      const entries = res?.entries || [];
      const integrityOk = res?.integrity_ok === true;
      const allMatch = res?.all_match === true;
      const differ = entries.filter((e) => e.integrity && !e.matches_current).length;
      if (res?.ok === false) {
        verifyResult = `Verify incomplete: live modem read failed for ${entries.filter((e) => e.read_error).length} entr(ies).`;
      } else if (!integrityOk) {
        verifyResult = `Integrity FAILED on ${entries.filter((e) => !e.integrity).length} entr(ies) — restore is disabled.`;
      } else if (allMatch) {
        verifyResult = `Verify OK: ${entries.length}/${entries.length} entries match current modem state.`;
      } else {
        verifyResult = `Integrity OK — differs from current modem (${differ}/${entries.length} differ, restore target).`;
      }
    } catch (e: unknown) {
      verifyResult = `Verify error: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      verifyingId = null;
    }
  }

  async function openRestoreReview(id: string, isEmergency = false) {
    statusMsg = null;
    try {
      const res = await rpc('backup.verify', { id }) as {
        ok: boolean; integrity_ok?: boolean; all_match?: boolean; id?: string;
        entries?: Array<{ path: string; integrity: boolean; matches_current: boolean; read_error?: string }>;
      };
      // A valid backup that differs from the modem (all_match:false) is the
      // REASON restore exists — the review dialog must open. It stays
      // disabled only for integrity problems or live pre-read failures.
      // The concrete resolved id is REQUIRED and frozen here. "latest" is
      // never acceptable as a frozen id — the backend resolved it already.
      const concreteId = res?.id;
      if (!concreteId || concreteId.trim() === '' || concreteId === 'latest') {
        statusMsg = 'Cannot resolve a concrete backup id for restore review (verify response missing id).';
        return;
      }
      const resolvedId = concreteId;
      restoreReview = {
        resolvedId,
        isEmergency,
        verify: {
          ok: !!res?.ok,
          integrity_ok: res?.integrity_ok !== false,
          all_match: res?.all_match === true,
          entries: res?.entries || [],
        },
      };
      restoreConfirmText = '';
    } catch (e: unknown) {
      statusMsg = `Cannot verify backup before restore: ${e instanceof Error ? e.message : String(e)}`;
    }
  }

  function closeRestoreReview() {
    restoreReview = null;
    restoreConfirmText = '';
  }

  async function handleRestore() {
    if (!restoreReview || restoreConfirmText !== 'RESTORE') {
      return;
    }
    // restore the FROZEN resolvedId — never "latest" after review resolved
    const targetId = restoreReview.resolvedId;
    closeRestoreReview();
    restoringId = targetId;
    statusMsg = null;
    try {
      const res = await rpc('backup.restore', { id: targetId }) as RestoreResult;
      const restoredList = res?.restored || [];
      const allOkAndVerified = res && res.ok && restoredList.length > 0 && restoredList.every(r => r.ok && r.verified === true);
      if (allOkAndVerified) {
        statusMsg = `Backup '${targetId}' successfully restored and verified on modem!`;
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

  <!-- Read-only snapshot & verify (safe without modem writes) -->
  <div class="card" style="display: flex; justify-content: space-between; align-items: center; gap: 10px;">
    <div>
      <div class="section-label">READ-ONLY SNAPSHOT</div>
      <p class="caption" style="margin-top: 2px;">
        Capture current NV bytes (3 bandlock paths; NR NSA excluded — unsupported on this firmware) without writing — then verify any backup against the live modem.
      </p>
    </div>
    <button class="btn btn-secondary" onclick={handleCreateSnapshot} disabled={creatingSnapshot || !$bridgeStatus.ready}>
      {creatingSnapshot ? 'Capturing...' : 'Create Snapshot'}
    </button>
  </div>
  {#if verifyResult}
    <div class="card status-info-card">{verifyResult}</div>
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
          <div style="display: flex; gap: 8px;">
            <button class="btn btn-secondary" onclick={() => handleVerify(item.id)} disabled={verifyingId === item.id}>
              {verifyingId === item.id ? 'Verifying...' : 'Verify'}
            </button>
            <button
              class="btn btn-secondary"
              onclick={() => openRestoreReview(item.id)}
              disabled={restoringId === item.id || !$bridgeStatus.ready}
            >
              <RotateCcw size={16} /> {restoringId === item.id ? 'Restoring...' : 'Restore'}
            </button>
          </div>
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
          Verifies then restores the newest backup manifest (resolved dynamically from the backups folder).
        </p>
      </div>
    </div>
    <button class="btn btn-danger" onclick={() => openRestoreReview('latest', true)} disabled={!$bridgeStatus.ready || backups.length === 0}>
      Review & Restore Latest Backup
    </button>
  </div>

  {#if restoreReview && restoreReview.verify}
    {@const review = restoreReview}
    <div class="overlay">
      <div class="dialog">
        <div class="dialog-header">
          <h2 style="color: var(--warning);">Review Restore</h2>
        </div>
        <div class="danger-zone card">
          <p class="caption" style="display: grid; gap: 4px;">
            <span>Backup ID: <span class="mono">{review.resolvedId}</span></span>
            <span>{review.verify.entries.length} entr(ies)</span>
            {#if !review.verify.all_match}
              <span style="color: var(--warning);">
                {review.verify.entries.filter((e) => e.integrity && !e.matches_current).length} entr(ies) differ from the current modem — that is the expected reason to restore.
              </span>
            {/if}
            {#if !review.verify.ok}
              <span style="color: var(--danger);">Live modem pre-read failed — restore is disabled until the modem can be read.</span>
            {/if}
          </p>
          <div class="mono caption" style="margin-top: 6px; display: grid; gap: 2px; max-height: 160px; overflow: auto;">
            {#each review.verify.entries as e}
              <span style="color: {e.integrity && e.matches_current ? 'var(--success)' : (e.read_error ? 'var(--danger)' : 'var(--warning)')};">
                {e.path.split('/').pop()} — {e.read_error ? 'READ ERROR' : (e.integrity ? 'intact' : 'BAD')} / {e.matches_current ? 'matches current' : (e.read_error ? 'unreadable' : 'differs (restore target)')}
              </span>
            {/each}
          </div>
          <p class="caption" style="margin-top: 6px;">
            Type <strong>RESTORE</strong> to arm. Every entry is verified before
            writing; failures roll back automatically.
          </p>
          <input type="text" class="input mono" bind:value={restoreConfirmText} placeholder="Type RESTORE" style="margin-top: 8px;" />
        </div>
        <div class="dialog-actions">
          <button class="btn btn-secondary" onclick={closeRestoreReview}>Cancel</button>
          <button
            class="btn btn-warning"
            onclick={handleRestore}
            disabled={restoreConfirmText !== 'RESTORE' || !review.verify.ok || !review.verify.integrity_ok}
          >
            Restore Backup
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
