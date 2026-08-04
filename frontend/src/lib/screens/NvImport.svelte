<script lang="ts">
  import { rpc } from '../bridge';
  import { bridgeStatus } from '../bridge';
  import { FileCode, Upload, Search, CheckCircle2, AlertCircle } from 'lucide-svelte';

  interface ReadNvResult {
    ok: boolean;
    bytes?: string;
    absent?: boolean;
    exit?: number;
    error?: string;
  }

  interface ImportCommand {
    slot: number;
    op: 'w' | 'd';
    path: string;
    bytes?: string;
  }

  interface ImportPreviewResult {
    ok: boolean;
    commands?: ImportCommand[];
    errors?: string[];
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

  interface ImportApplyResult {
    ok: boolean;
    error?: string;
    rollback?: RollbackInfo;
    results?: Array<{
      slot: number;
      op: string;
      path: string;
      ok: boolean;
      exit: number;
      backup_id?: string;
      verified?: boolean;
    }>;
    ok_count?: number;
    fail_count?: number;
    verified_count?: number;
    unverified_count?: number;
  }

  const BASE_PATHS = [
    '/nv/item_files/modem/lte/rrc/efs/',
    '/nv/item_files/modem/nr5g/RRC/',
    '/nv/item_files/modem/mmode/'
  ];

  let modeTab = $state<'single' | 'batch'>('single');
  let selectedBase = $state(BASE_PATHS[0]);
  let subPath = $state('cap_control_nrca_2x_f_plus_t_band_combos');
  let slot = $state(0);

  let readLoading = $state(false);
  let readResult = $state<ReadNvResult | null>(null);
  let readError = $state<string | null>(null);
  // The exact path+slot that produced readResult — Review Delete is only
  // armed when this still matches the CURRENT inputs (no stale-target deletes).
  let readTarget = $state<{ path: string; slot: number } | null>(null);

  // Single NV write / delete state
  let writeHex = $state('');
  let singleActionLoading = $state(false);
  let deleteReview = $state(false);      // review dialog open
  let deleteConfirmText = $state('');    // must type DELETE
  let deleteCurrentBytes = $state<string | null>(null);
  let singleActionMsg = $state<string | null>(null);
  let singleActionRollback = $state<RollbackInfo | null>(null);

  // Import section
  let jsonString = $state('');
  let importPreview = $state<ImportPreviewResult | null>(null);
  let importApplying = $state(false);
  let importResults = $state<ImportApplyResult | null>(null);
  let importError = $state<string | null>(null);

  let textareaElem = $state<HTMLTextAreaElement | undefined>(undefined);

  function fullPath(): string {
    if (subPath.startsWith('/')) return subPath;
    return selectedBase + subPath;
  }

  $effect(() => {
    // any change to the target inputs invalidates the read result
    void selectedBase; void subPath; void slot;
    readResult = null;
    readTarget = null;
    readError = null;
  });

  async function handleReadNv() {
    readLoading = true;
    readResult = null;
    readError = null;
    try {
      const path = fullPath();
      const res = await rpc('nv.read', { path, slot }) as ReadNvResult;
      readResult = res;
      readTarget = { path, slot };
    } catch (e: unknown) {
      readError = e instanceof Error ? e.message : String(e);
    } finally {
      readLoading = false;
    }
  }
  async function handleWriteNv() {
    if (!writeHex.trim()) return;
    singleActionLoading = true;
    singleActionMsg = null;
    singleActionRollback = null;
    try {
      const path = fullPath();
      const res = await rpc('nv.write', { path, hex: writeHex, slot, reason: 'Single NV write' }) as { ok: boolean; verified?: boolean; error?: string; rollback?: RollbackInfo };
      if (res && res.ok && res.verified) {
        singleActionMsg = `NV write verified and applied successfully to ${path}`;
        await handleReadNv();
      } else if (res && res.ok === false) {
        let msg = res.error || 'NV write failed';
        if (res.rollback) {
          msg += ' (rolled back)';
          singleActionRollback = res.rollback;
        }
        singleActionMsg = msg;
      } else {
        singleActionMsg = `NV written but read-back verification FAILED for ${path}`;
      }
    } catch (e: unknown) {
      singleActionMsg = `Write error: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      singleActionLoading = false;
    }
  }

  function reviewDeleteNv() {
    // The reviewed target is FROZEN here; handleDeleteNv must use it, never
    // recompute fullPath().
    if (!readTarget) return;
    const current = readResult && !readResult.absent && readResult.bytes ? readResult.bytes : null;
    deleteCurrentBytes = current;
    deleteConfirmText = '';
    deleteReview = true;
  }

  function closeDeleteReview() {
    deleteReview = false;
    deleteConfirmText = '';
  }

  async function handleDeleteNv() {
    // two-step guard: dialog must be open and DELETE typed; target frozen
    if (!deleteReview || deleteConfirmText !== 'DELETE' || !readTarget) return;
    const target = { ...readTarget };
    deleteReview = false;
    deleteConfirmText = '';
    singleActionLoading = true;
    singleActionMsg = null;
    singleActionRollback = null;
    try {
      // re-read the FROZEN target right before deletion; abort if the
      // current bytes differ from what the user reviewed.
      let fresh: ReadNvResult | null = null;
      try {
        fresh = await rpc('nv.read', { path: target.path, slot: target.slot }) as ReadNvResult;
      } catch { /* keep null */ }
      const freshBytes = fresh && !fresh.absent && fresh.bytes ? fresh.bytes : null;
      const reviewedBytes = deleteCurrentBytes || null;
      if (freshBytes !== reviewedBytes) {
        singleActionMsg = `ABORTED: ${target.path} changed since review (reviewed ${reviewedBytes ?? 'absent'}, now ${freshBytes ?? 'absent'}). Delete cancelled — review again.`;
        return;
      }
      const path = target.path;
      const slotT = target.slot;
      const res = await rpc('nv.delete', { path, slot: slotT, reason: 'Single NV delete' }) as { ok: boolean; verified?: boolean; error?: string; rollback?: RollbackInfo };
      if (res && res.ok && res.verified) {
        singleActionMsg = `NV item deleted and verified successfully at ${path}`;
        await handleReadNv();
      } else if (res && res.ok === false) {
        let msg = res.error || 'NV delete failed';
        if (res.rollback) {
          msg += ' (rolled back)';
          singleActionRollback = res.rollback;
        }
        singleActionMsg = msg;
      } else {
        singleActionMsg = `NV item deleted but verification FAILED at ${path}`;
      }
    } catch (e: unknown) {
      singleActionMsg = `Delete error: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      singleActionLoading = false;
    }
  }

  function handleFileUpload(e: Event) {
    const target = e.target as HTMLInputElement;
    if (!target.files || target.files.length === 0) return;
    const file = target.files[0];
    const reader = new FileReader();
    reader.onload = () => {
      jsonString = String(reader.result || '');
      if (textareaElem) {
        textareaElem.value = jsonString;
        textareaElem.dispatchEvent(new Event('input', { bubbles: true }));
      }
      handlePreviewImport();
    };
    reader.readAsText(file);
  }

  async function handlePreviewImport() {
    if (!jsonString.trim()) return;
    importError = null;
    importResults = null;
    try {
      const res = await rpc('import.preview', { json: jsonString }) as ImportPreviewResult;
      importPreview = res;
    } catch (e: unknown) {
      importError = e instanceof Error ? e.message : String(e);
    }
  }

  async function handleApplyImport() {
    if (!importPreview || !importPreview.commands) return;
    importApplying = true;
    importError = null;
    try {
      const res = await rpc('import.apply', { json: jsonString }) as ImportApplyResult;
      importResults = res;
      importPreview = null;
      if (res && res.ok === false) {
        let msg = res.error || 'Import apply failed';
        if (res.rollback) msg += ' (rolled back)';
        importError = msg;
      }
    } catch (e: unknown) {
      importError = e instanceof Error ? e.message : String(e);
    } finally {
      importApplying = false;
    }
  }

  function formatHexColorBytes(hexStr: string): Array<{ byte: string; colorClass: string }> {
    const clean = hexStr.replace(/\s+/g, '');
    const res: Array<{ byte: string; colorClass: string }> = [];
    for (let i = 0; i < clean.length; i += 2) {
      const b = clean.substring(i, i + 2);
      const val = parseInt(b, 16);
      let colorClass = 'byte-zero';
      if (val === 1) colorClass = 'byte-one';
      else if (val > 1) colorClass = 'byte-val';
      res.push({ byte: b, colorClass });
    }
    return res;
  }
</script>

<div class="nvimport-screen">
  <!-- Screen Header -->
  <div class="screen-header">
    <div>
      <h1 class="screen-title">NV Explorer & Import</h1>
      <p class="screen-subtitle">Inspect EFS File Paths & Execute Batch Operations</p>
    </div>
  </div>

  <!-- Segmented Control Mode Switcher -->
  <div class="segmented-control">
    <button
      class={`segmented-tab ${modeTab === 'single' ? 'active' : ''}`}
      onclick={() => modeTab = 'single'}
    >
      <Search size={14} /> Single NV Read / Write
    </button>
    <button
      class={`segmented-tab ${modeTab === 'batch' ? 'active' : ''}`}
      onclick={() => modeTab = 'batch'}
    >
      <FileCode size={14} /> Batch Import JSON
    </button>
  </div>

  {#if modeTab === 'single'}
    <!-- Single Path NV Reader Card -->
    <div class="card reader-card">
      <div class="section-label">READ NV EFS ITEM</div>
      <div class="path-builder">
        <div class="field-group">
          <label for="base-path">Base Path Prefix:</label>
          <select id="base-path" class="select" bind:value={selectedBase}>
            {#each BASE_PATHS as bp}
              <option value={bp}>{bp}</option>
            {/each}
            <option value="">Custom Absolute Path</option>
          </select>
        </div>

        <div class="field-group">
          <label for="item-path">Item Subpath / Absolute Target:</label>
          <input id="item-path" type="text" class="input mono" bind:value={subPath} placeholder="e.g. cap_control..." />
        </div>

        <div class="row-flex">
          <div class="field-group slot-group">
            <label for="nv-slot">SIM Slot:</label>
            <select id="nv-slot" class="select" bind:value={slot}>
              <option value={0}>Slot 0</option>
              <option value={1}>Slot 1</option>
            </select>
          </div>
          <button class="btn btn-primary" onclick={handleReadNv} disabled={readLoading}>
            {readLoading ? 'Reading...' : 'Read NV Path'}
          </button>
        </div>
        </div>

      <div class="section-label" style="margin-top: 16px;">SINGLE NV WRITE / DELETE</div>
      <div class="field-group">
        <label for="write-hex">Payload Hex (for Write):</label>
        <input id="write-hex" type="text" class="input mono" bind:value={writeHex} placeholder="e.g. 01000000..." />
      </div>
      <div class="row-flex" style="margin-top: 8px;">
        <button class="btn btn-secondary" onclick={handleWriteNv} disabled={singleActionLoading || !writeHex.trim() || !$bridgeStatus.ready}>
          {singleActionLoading ? 'Writing...' : 'Write NV Item'}
        </button>
        <button
          class="btn btn-danger"
          onclick={reviewDeleteNv}
          disabled={singleActionLoading || !$bridgeStatus.ready || !readTarget || readTarget.path !== fullPath() || readTarget.slot !== slot}
          title={readTarget ? 'Review current NV before deleting' : 'Read the item first (Review Delete locks to the exact path+slot read)'}
        >
          {singleActionLoading ? 'Deleting...' : 'Review Delete'}
        </button>
      </div>

      {#if singleActionMsg}
        <div class="card status-info-card" style="margin-top: 10px;">
          <span>{singleActionMsg}</span>
        </div>
      {/if}

      {#if singleActionRollback && singleActionRollback.entries}
        <div class="card status-err-card" style="margin-top: 10px;">
          <strong style="color: var(--danger);">Rollback Verification Details</strong>
          <div style="display: flex; flex-direction: column; gap: 4px; margin-top: 6px;">
            {#each singleActionRollback.entries as entry}
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

      {#if readError}
        <div class="card status-err-card">{readError}</div>
      {/if}

      {#if readResult}
        <div class="result-box">
          {#if readResult.absent}
            <div class="chip status-info">NV Item Absent / Empty (Modem Default)</div>
          {:else if readResult.bytes}
            <div class="hex-dump card">
              <span class="caption">Hex Dump Payload ({readResult.bytes.length / 2} bytes):</span>
              <div class="hex-bytes mono">
                {#each formatHexColorBytes(readResult.bytes) as bItem}
                  <span class={`hex-byte ${bItem.colorClass}`}>{bItem.byte}</span>
                {/each}
              </div>
            </div>
          {/if}
        </div>
      {/if}
    </div>
  {:else}
    <!-- JSON File Import Card -->
    <div class="card import-card">
      <div class="section-label">BATCH NV IMPORT PAYLOAD</div>
      
      <div class="file-picker">
        <label class="btn btn-secondary">
          <Upload size={16} /> Load JSON File
          <input type="file" accept=".json" onchange={handleFileUpload} hidden />
        </label>
        <span class="caption">Select bulk-import JSON configuration</span>
      </div>

      <textarea
        bind:this={textareaElem}
        class="textarea mono"
        rows="8"
        placeholder="Paste JSON import payload..."
        bind:value={jsonString}
        oninput={handlePreviewImport}
      ></textarea>

      {#if importError}
        <div class="card status-err-card">{importError}</div>
      {/if}

      {#if importPreview && importPreview.commands}
        <div class="preview-table-box">
          <div class="caption">Import Commands Preview ({importPreview.commands.length} operations):</div>
          <div class="table-scroll">
            <table class="preview-table">
              <thead>
                <tr>
                  <th>Slot</th>
                  <th>Op</th>
                  <th>Path</th>
                  <th>Payload (Hex)</th>
                </tr>
              </thead>
              <tbody>
                {#each importPreview.commands as cmd}
                  <tr>
                    <td>{cmd.slot}</td>
                    <td><span class={`op-chip ${cmd.op}`}>{cmd.op === 'w' ? 'WRITE' : 'DELETE'}</span></td>
                    <td class="mono path-cell">{cmd.path}</td>
                    <td class="mono hex-cell">{cmd.bytes || '-'}</td>
                  </tr>
                {/each}
              </tbody>
            </table>
          </div>

          <button class="btn btn-danger apply-import-btn" onclick={handleApplyImport} disabled={importApplying || !$bridgeStatus.ready}>
            {importApplying ? 'Applying Import...' : 'Confirm & Apply Bulk Import'}
          </button>
        </div>
      {/if}

      {#if importResults}
        {@const resList = importResults.results || []}
        {@const okAndVerifiedCount = resList.filter(r => r.ok && r.verified === true).length}
        {@const failOrUnverifiedCount = resList.length - okAndVerifiedCount}
        <div class="card import-results">
          <div class="results-header">
            <CheckCircle2 class={importResults.ok ? 'icon-success' : 'icon-fail'} />
            <strong style="color: var(--text-primary);">
              Import Summary: {okAndVerifiedCount} OK & Verified, {failOrUnverifiedCount} Failed / Unverified
            </strong>
          </div>
          <ul class="results-list">
            {#each resList as r}
              <li class={r.ok && r.verified === true ? 'res-ok' : 'res-fail'}>
                <span class="mono">[{r.op.toUpperCase()}] Slot {r.slot}: {r.path}</span>
                <span class="caption">{r.ok && r.verified === true ? 'OK & Verified' : `Failed (Exit ${r.exit}, Verified: ${r.verified ?? false})`}</span>
              </li>
            {/each}
          </ul>
        </div>
      {/if}
    </div>
  {/if}
  {#if deleteReview}
    <div class="overlay">
      <div class="dialog">
        <div class="dialog-header">
          <h2 style="color: var(--danger);">Review NV Delete</h2>
        </div>
        <div class="danger-zone card">
          <p class="caption" style="display: grid; gap: 4px;">
            <span>Path: <span class="mono">{readTarget?.path || '—'}</span></span>
            <span>Slot: {readTarget?.slot ?? '—'}</span>
            <span>Current bytes: <span class="mono">{deleteCurrentBytes || '(unreadable — cannot confirm delete)'}</span></span>
          </p>
          <p class="caption" style="margin-top: 6px;">
            A backup of the current value is created before deletion, and the
            delete is read-back verified. Type <strong>DELETE</strong> to arm the button.
          </p>
          <input
            type="text"
            class="input mono"
            bind:value={deleteConfirmText}
            placeholder="Type DELETE"
            style="margin-top: 8px;"
          />
        </div>
        <div class="dialog-actions">
          <button class="btn btn-secondary" onclick={closeDeleteReview}>Cancel</button>
          <button
            class="btn btn-danger"
            onclick={handleDeleteNv}
            disabled={deleteConfirmText !== 'DELETE' || !deleteCurrentBytes}
          >
            Delete NV Item
          </button>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .nvimport-screen {
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
  .reader-card, .import-card {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .path-builder {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .field-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .field-group label {
    font-size: 12px;
    color: var(--text-muted);
  }
  .row-flex {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 12px;
  }
  .slot-group {
    width: 140px;
  }
  .hex-dump {
    display: flex;
    flex-direction: column;
    gap: 8px;
    background-color: var(--surface-1);
  }
  .hex-bytes {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    font-size: 13px;
  }
  .hex-byte {
    padding: 2px 6px;
    border-radius: 4px;
  }
  .byte-zero { color: var(--text-muted); }
  .byte-one { color: var(--primary); font-weight: bold; }
  .byte-val { color: var(--success); font-weight: bold; }

  .file-picker {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .table-scroll {
    overflow-x: auto;
  }
  .preview-table-box {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .preview-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }
  .preview-table th, .preview-table td {
    padding: 8px 10px;
    border: 1px solid var(--border);
    text-align: left;
  }
  .preview-table th {
    background-color: var(--surface-2);
    color: var(--text-muted);
  }
  .op-chip.w { color: var(--primary); font-weight: bold; }
  .op-chip.d { color: var(--danger); font-weight: bold; }
  .path-cell { word-break: break-all; }

  .apply-import-btn {
    align-self: flex-end;
  }
  .status-err-card {
    border-color: var(--danger);
    background-color: rgba(255, 97, 97, 0.05);
  }
  .caption {
    font-size: 12px;
    color: var(--text-muted);
  }
  .icon-success {
    color: var(--success);
  }
</style>
