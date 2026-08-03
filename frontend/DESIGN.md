# MTB Control Design System Spec

The **MTB Control Design System** is a dense, high-utility, dark-first mobile design language engineered specifically for modem EFS NV item management, band locking, feature configuration, and signal diagnostics on Qualcomm devices (POCO F6 / HyperOS).

Derived from Linear's restrained dark-mode software-craft aesthetic and combined with Raycast's high-contrast semantic status system, MTB Control delivers a calm, precise environment for complex radio operations.

---

## 1. Design Principles

1. **Dense Technical Utility**  
   Every pixel carries purpose. We avoid decorative glassmorphism, oversized hero banners, or excessive whitespace. Spacing is tight (4/8/12/16px scale), information hierarchy is razor-sharp, and controls map cleanly to underlying hardware operations.

2. **Restrained Single Chromatic Accent**  
   The primary visual anchor is Linear Lavender (`#5e6ad2`). It is reserved strictly for primary actions, active navigation states, selected toggles, and focus indicators. It never appears decoratively or as ambient gradients.

3. **Strict Semantic Status Colors**  
   Status indicators use Raycast-inspired high-visibility hues exclusively for operational truth:
   - **Info (`#57c1ff`)**: Neutral state, active polling, probe metadata.
   - **Success (`#59d499`)**: Confirmed write verification, backup generated, modem ok.
   - **Warning (`#ffc533`)**: Pending unapplied changes, missing NV items/fallbacks, stale cell data.
   - **Danger (`#ff6161`)**: Destructive operations, write errors, emergency restore, modem reset.

4. **Surface Ladder over Dropshadows**  
   Depth and elevation are expressed solely through surface luminance stepping (`--canvas` `#010102` → `--surface-1` `#0f1011` → `--surface-2` `#141516` → `--surface-3` `#18191a`) combined with hairline borders (`--border` `#23252a`). Box shadows are omitted except for modal dialog overlays (`--shadow-dialog`).

5. **Mobile-First Touch Compliance**  
   Built for one-handed thumb navigation on mobile viewports. Interactive controls enforce a strict minimum touch target of 44–48px with 8px control corner radii.

---

## 2. Typography Scale & Usage Matrix

- **UI Font (`--font-ui`)**: `Inter`, `system-ui`, `-apple-system`, `sans-serif`
- **Monospace Font (`--font-mono`)**: `'JetBrains Mono'`, `'Roboto Mono'`, `ui-monospace`, `monospace`

| Role | Font Family | Size | Weight | Line Height | Letter Spacing | Usage Target |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Screen Title** | UI | 24px–28px | 600 (Semi) | 1.2 | -0.5px | Screen header title (`MTB Control`) |
| **Section Label** | UI | 12px–13px | 600 (Semi) | 1.3 | +0.4px | Group titles (`LTE BANDS`, `DANGER ZONE`, uppercase) |
| **Card Title** | UI | 16px–18px | 600 (Semi) | 1.25 | -0.2px | Sub-card header, feature name, cell summary |
| **Body** | UI | 14px–15px | 400 (Reg) | 1.5 | 0 | Descriptions, status text, label values |
| **Caption** | UI | 12px | 400 (Reg) | 1.4 | 0 | Sub-labels, backup timestamps, slot tags |
| **Mono Primary** | Mono | 13px–14px | 500 (Med) | 1.4 | 0 | Hex payloads, NV file paths, EARFCN/PCI values |
| **Mono Logs** | Mono | 12px | 400 (Reg) | 1.5 | 0 | Raw CLI JSON output, diagnostic traces, backup IDs |

---

## 3. Color Roles & Palette Contract

### Base Tokens (Imported verbatim from `theme.css`)

```css
:root {
  --canvas: #010102;
  --surface-1: #0f1011;
  --surface-2: #141516;
  --surface-3: #18191a;
  --surface-selected: #202126;
  --border: #23252a;
  --border-strong: #34343a;
  --text-primary: #f7f8f8;
  --text-secondary: #d0d6e0;
  --text-muted: #8a8f98;
  --text-disabled: #62666d;
  --primary: #5e6ad2;
  --primary-hover: #828fff;
  --primary-pressed: #4f59b8;
  --success: #59d499;
  --warning: #ffc533;
  --danger: #ff6161;
  --info: #57c1ff;
  --radius-control: 8px;
  --radius-card: 12px;
  --radius-dialog: 16px;
  --radius-pill: 9999px;
}
```

### Role Allocation

- `--canvas` (`#010102`): Main application page background.
- `--surface-1` (`#0f1011`): Base card background, input fields, unselected band tiles.
- `--surface-2` (`#141516`): Secondary containers, hover states, active segmented tabs, status chip pills.
- `--surface-3` (`#18191a`): Tertiary surfaces, header bars, scrollbar thumbs.
- `--surface-selected` (`#202126`): Active selected band tile background, active list items.
- `--border` (`#23252a`): 1px hairline dividers and standard container edges.
- `--border-strong` (`#34343a`): Focused container borders, active tab borders, selected tiles.
- `--text-primary` (`#f7f8f8`): High-contrast titles, primary values, active text.
- `--text-secondary` (`#d0d6e0`): Standard body text, table cells, form labels.
- `--text-muted` (`#8a8f98`): Secondary metadata, inactive tabs, captions.
- `--text-disabled` (`#62666d`): Disabled buttons, missing NV items, placeholder text.
- `--primary` (`#5e6ad2`): Primary CTAs, active switch toggles, focus rings.
- `--success` (`#59d499`): Verification OK, backup created, active cell signal.
- `--warning` (`#ffc533`): Unapplied band changes, missing NV path fallback, high staleness.
- `--danger` (`#ff6161`): Destructive action CTAs, write error state, emergency restore zone.
- `--info` (`#57c1ff`): Active polling indicator, probe information status.

---

## 4. Component Specifications

### Buttons
- **Height**: 44px–48px (touch safe)
- **Radius**: `8px` (`--radius-control`)
- **Typography**: UI Sans, 14px, Weight 500
- **Variants**:
  - **Primary (`.btn-primary`)**: Background `--primary`, text `#ffffff`. Hover `--primary-hover`, pressed `--primary-pressed`.
  - **Secondary (`.btn-secondary`)**: Background `--surface-2`, border `--border`, text `--text-primary`. Hover `--surface-3`.
  - **Danger (`.btn-danger`)**: Background `rgba(255, 97, 97, 0.15)`, text `--danger`, border `rgba(255, 97, 97, 0.3)`. Hover background `--danger`, text `#ffffff`.
  - **Ghost (`.btn-ghost`)**: Background transparent, text `--text-secondary`. Hover background `--surface-2`, text `--text-primary`.

### Cards & Panels
- **Radius**: `12px` (`--radius-card`)
- **Border**: 1px solid `--border` (`#23252a`)
- **Background**: Surface stepping (`.card` = `--surface-1`, sub-panel = `--surface-2`)
- **Padding**: 16px (`--space-4`)

### Inputs & Selects
- **Height**: Minimum 44px
- **Radius**: `8px` (`--radius-control`)
- **Background**: `--surface-1`, border `--border`, text `--text-primary`
- **Focus State**: Border `--primary`, box-shadow `0 0 0 2px rgba(94, 106, 210, 0.3)` (`--focus-ring`)

### Segmented Controls (Pill Tabs)
- **Container**: Background `--canvas`, border `--border`, radius `9999px` (`--radius-pill`), 3px padding
- **Tabs**: Minimum 36px height, text `--text-muted`, radius `9999px`
- **Active Tab**: Background `--surface-2`, border `--border-strong`, text `--text-primary`

### Status Badges & Chips
- **Radius**: `9999px` (`--radius-pill`)
- **Padding**: 2px 10px, font 12px weight 500
- **Variants**:
  - `.status-ok`: Background `rgba(89,212,153,0.1)`, border `rgba(89,212,153,0.3)`, text `--success`
  - `.status-warn`: Background `rgba(255,197,51,0.1)`, border `rgba(255,197,51,0.3)`, text `--warning`
  - `.status-err`: Background `rgba(255,97,97,0.1)`, border `rgba(255,97,97,0.3)`, text `--danger`
  - `.status-info`: Background `rgba(87,193,255,0.1)`, border `rgba(87,193,255,0.3)`, text `--info`

### Band Grid Tiles
- **Aspect Ratio**: Square `1:1`, minimum height 48px
- **Background**: `--surface-1`, border `--border`, font Mono 14px weight 600
- **Selected State**: Background `--surface-selected`, border `--border-strong`, text `--text-primary`, inset primary accent outline `inset 0 0 0 1px #5e6ad2`.

### Dialogs & Overlays
- **Overlay**: `--overlay` (`rgba(0,0,0,0.75)`), backdrop blur 4px
- **Dialog Body**: Background `--surface-2`, border `--border-strong`, radius `16px` (`--radius-dialog`), shadow `--shadow-dialog`
- **Padding**: 20px–24px

### Sticky Bottom Action Bar
- **Position**: `position: sticky; bottom: 0; z-index: 100;`
- **Style**: Background `--canvas`, border-top 1px `--border`, padding 12px 16px, backdrop blur 8px. Contains action summary + primary CTA.

### Switches & Toggles
- **Track**: Width 44px, height 24px, radius `9999px`. Off: background `--surface-3`, border `--border`. On: background `--primary`.
- **Thumb**: 20px circle, background `#ffffff`, smooth 0.15s sliding transition.

### Danger Zone Section
- **Style**: Background `rgba(255,97,97,0.04)`, border 1px solid `rgba(255,97,97,0.3)`, radius `12px`. Contains high-risk actions (Emergency Restore, NV Delete).

---

## 5. Screen-by-Screen Layout Specifications

### 1. Dashboard (Home Reference Mock)
```
+-------------------------------------------------------------+
| MTB Control  [POCO F6 / SM8635]             (Slot 0 v / Slot 1) |
+-------------------------------------------------------------+
| [CARD: Modem Status]                                        |
|  Status: [CHIP: OK / mtb present]     Modem: /vendor/bin/mtb|
|  mtbctl: v1.0.0                       SDK: 34 (HyperOS)     |
+-------------------------------------------------------------+
| [CARD: Current Connection]                                  |
|  LTE: Band 1 (2100 MHz)   RSRP: -92 dBm   RSRQ: -11 dB       |
|  NR:  Band n78 (3500 MHz) RSRP: -85 dBm   SNR: 18.2 dB       |
+-------------------------------------------------------------+
| [CARD: Quick Actions & Band Control Entry]                  |
|  [Btn: Band Locking ->]       [Btn: Feature Toggles ->]     |
|  [Btn: NV Explorer ->]        [Btn: Diagnostic Cells ->]    |
+-------------------------------------------------------------+
| [CARD: Latest Backup Summary]                               |
|  ID: 1772649120_bandlock_set   Time: 2026-08-03 14:22:05    |
|  Entries: 4 items (LTE/NR masks)              [Btn: Details] |
+-------------------------------------------------------------+
```

### 2. Bandlock Screen
- **Tabs**: `[Detected Bands]` | `[Manual Select]` (Pill Segmented Control)
- **Grids**: Separate Band Grid sections for `LTE`, `NR NSA`, and `NR SA`.
- **Tiles**: 48px square tiles showing band numbers (`B1`, `B3`, `B7`, `n78`). Selected tiles highlight in `--surface-selected` with `--primary` inset ring.
- **Sticky Apply Bar**: Appears at bottom when selections differ from current state. Displays pending band counts + `[Preview & Apply]` button.
- **Preview Dialog Flow**: Modal dialog showing 4 NV mask file paths with hex differences:
  ```
  /nv/item_files/modem/mmode/lte_bandpref
  Old: 05 00 00 00 00 00 00 00
  New: 45 00 00 00 00 00 00 00  (Diff highlighted in --warning)
  ```
  Includes two-step confirm CTA: `[Confirm & Write]` → triggers modem restart prompt.

### 3. Features Screen
- **Header**: Features Check Status + NR Mode Selector dropdown.
- **List**: Vertical stacked cards for the 12 modem features (e.g. `r17_2t2t`, `ul_mimo`, `nr_ulca`).
- **Feature Card Layout**:
  - Left: Feature label + 3GPP description + NV path list status.
  - Right: `[CHIP: Enabled / Disabled / Absent]` status badge + Switch toggle (`[Restore / Disable]`).

### 4. NV & Import Screen
- **Segmented Control**: `[Single NV Read/Write]` | `[Batch Import JSON]`
- **Single NV Mode**: Dropdown with canonical NV path prefixes + sub-path text input + `[Read]` button.
  - Hex Dump Box: Monospace container (`--surface-1`, `--border`) formatted in 16-byte rows with address offset header.
- **Batch Import Mode**: Drag-and-drop / file picker for JSON configuration + JSON code editor textarea.
  - Preview Table: Columns `Slot | Operation (Write/Delete) | NV Path | Hex Payload`.
  - Action Bar: `[Apply Batch Import (N items)]` with mandatory pre-backup warning.

### 5. Cells Diagnostic Screen
- **Header**: Signal Monitor + Polling Status Chip (`[CHIP: Live Polling 2s]` with animated `--info` pulse dot). Polling stops automatically when `document.hidden` is true.
- **Cards**:
  - **LTE Serving & Neighbor Cells**: Table displaying `EARFCN`, `PCI`, `RSRP`, `RSRQ`, `RSSI`, `SNR`. Color-coded signal strength (RSRP > -90 `--success`, -90 to -105 `--warning`, < -105 `--danger`).
  - **NR Serving & Neighbor Cells**: `RSRP` and `RSRQ` cards.
  - **TX Power Output**: Metric tile displaying current modem TX power in dBm.
  - **Staleness Bar**: If poll fails 3 times consecutively, card dims and `--warning` chip (`[Stale Data]`) is rendered.

### 6. Backups Screen
- **List View**: Card per backup entry sorted newest first.
  - Header: Backup ID (`unix_ts_reason`), timestamp (`YYYY-MM-DD HH:mm:ss`), entry count badge.
  - Action: `[Restore This Backup]` secondary button.
- **Emergency Restore Danger Zone**: Red-tinted card (`.danger-zone`) at screen bottom:
  - Title: `EMERGENCY RESTORE`
  - Body: Restores `latest.json` backup payload directly to modem NV storage.
  - Action: `[Btn: Danger -> Restore Latest]` requiring explicit modal confirmation.

---

## 6. Interaction Rules & Verification Invariants

1. **Two-Step Destructive Confirmations**  
   Any modem NV write, feature disable/restore, batch import, or backup restore MUST present a preview dialog before execution detailing the exact target paths and slot numbers.

2. **Old → New Hex Payload Diffing**  
   Bandlock and NV write preview dialogs must render the current on-modem hex payload alongside the proposed new payload, visually highlighting byte differences in `--warning` (`#ffc533`) color.

3. **Disabled & Loading States**  
   Buttons in loading or disabled state must have `opacity: 0.5`, `pointer-events: none`, and display a inline spinner or status label (`Writing...`, `Polling...`).

4. **Cells Polling & Background Pause**  
   Cell diagnostics poll every 2 seconds by default. The implementation MUST subscribe to `visibilitychange` events and pause polling when `document.hidden === true` to preserve mobile CPU/battery.

5. **Strict Touch Target Invariant**  
   All buttons, list items, band grid tiles, and select triggers MUST maintain an active touch box of at least 44px × 44px.
