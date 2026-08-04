<script lang="ts">
  import './lib/theme.css';
  import { rpc } from './lib/bridge';
  import Dashboard from './lib/screens/Dashboard.svelte';
  import Bandlock from './lib/screens/Bandlock.svelte';
  import Features from './lib/screens/Features.svelte';
  import NvImport from './lib/screens/NvImport.svelte';
  import Cells from './lib/screens/Cells.svelte';
  import Backups from './lib/screens/Backups.svelte';
  import { Home, Radio, Sliders, Search, Activity, Archive, Cpu } from 'lucide-svelte';

  let activeScreen = $state<string>('dashboard');
  let probeOk = $state<boolean | null>(null);
  let probeModel = $state<string>('Unknown Model');

  async function checkStatus() {
    try {
      const res = await rpc('probe') as { ok: boolean; model?: string };
      probeOk = res && res.ok;
      if (res && res.model) probeModel = res.model;
    } catch {
      probeOk = false;
    }
  }

  $effect(() => {
    checkStatus();
  });
</script>

<div class="app-container">
  <!-- Top Navigation Header / Status Bar -->
  <header class="app-header">
    <div class="brand">
      <Cpu size={20} class="brand-icon" />
      <span class="brand-name">MTB Control</span>
      <span class="mono caption model-tag">{probeModel}</span>
    </div>
    <div class="status-indicator">
      {#if probeOk === null}
        <span class="chip status-info">Probing Backend...</span>
      {:else if probeOk}
        <span class="chip status-ok">Backend Connected</span>
      {:else}
        <span class="chip status-err">CLI Error / Disconnected</span>
      {/if}
    </div>
  </header>

  <!-- Main Content Viewport -->
  <main class="content-area">
    {#if activeScreen === 'dashboard'}
      <Dashboard onNavigate={(screen) => activeScreen = screen} />
    {:else if activeScreen === 'bandlock'}
      <Bandlock />
    {:else if activeScreen === 'features'}
      <Features />
    {:else if activeScreen === 'nvimport'}
      <NvImport />
    {:else if activeScreen === 'cells'}
      <Cells />
    {:else if activeScreen === 'backups'}
      <Backups />
    {/if}
  </main>

  <!-- Mobile-First Bottom Navigation Bar -->
  <nav class="bottom-nav">
    <button
      class={`nav-item ${activeScreen === 'dashboard' ? 'active' : ''}`}
      onclick={() => activeScreen = 'dashboard'}
    >
      <Home size={18} />
      <span>Dashboard</span>
    </button>
    <button
      class={`nav-item ${activeScreen === 'bandlock' ? 'active' : ''}`}
      onclick={() => activeScreen = 'bandlock'}
    >
      <Radio size={18} />
      <span>Bandlock</span>
    </button>
    <button
      class={`nav-item ${activeScreen === 'features' ? 'active' : ''}`}
      onclick={() => activeScreen = 'features'}
    >
      <Sliders size={18} />
      <span>Features</span>
    </button>
    <button
      class={`nav-item ${activeScreen === 'nvimport' ? 'active' : ''}`}
      onclick={() => activeScreen = 'nvimport'}
    >
      <Search size={18} />
      <span>NV Import</span>
    </button>
    <button
      class={`nav-item ${activeScreen === 'cells' ? 'active' : ''}`}
      onclick={() => activeScreen = 'cells'}
    >
      <Activity size={18} />
      <span>Cells</span>
    </button>
    <button
      class={`nav-item ${activeScreen === 'backups' ? 'active' : ''}`}
      onclick={() => activeScreen = 'backups'}
    >
      <Archive size={18} />
      <span>Backups</span>
    </button>
  </nav>
</div>

<style>
  .app-container {
    display: flex;
    flex-direction: column;
    min-height: 100vh;
    background-color: var(--canvas);
    color: var(--text-primary);
  }
  .app-header {
    height: 52px;
    background-color: var(--surface-3);
    border-bottom: 1px solid var(--border);
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0 16px;
    position: sticky;
    top: 0;
    z-index: 100;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .brand-icon {
    color: var(--primary);
  }
  .brand-name {
    font-weight: 600;
    font-size: 15px;
    color: var(--text-primary);
  }
  .model-tag {
    background-color: var(--surface-1);
    border: 1px solid var(--border);
    padding: 2px 6px;
    border-radius: 4px;
    font-size: 11px;
    color: var(--text-muted);
  }
  .content-area {
    flex: 1;
    padding: 16px;
    padding-bottom: 76px;
    max-width: 900px;
    width: 100%;
    margin: 0 auto;
  }
  .bottom-nav {
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    height: 56px;
    background-color: var(--surface-3);
    border-top: 1px solid var(--border);
    display: flex;
    justify-content: space-around;
    align-items: center;
    z-index: 100;
  }
  .nav-item {
    background: transparent;
    border: none;
    color: var(--text-muted);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
    padding: 6px 10px;
    border-radius: 8px;
    transition: all 0.15s ease;
  }
  .nav-item:hover {
    color: var(--text-secondary);
  }
  .nav-item.active {
    color: var(--primary);
  }
  .caption {
    font-size: 12px;
  }
</style>
