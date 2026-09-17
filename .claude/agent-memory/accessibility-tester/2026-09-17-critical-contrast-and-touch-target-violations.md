---
name: critical-contrast-and-touch-target-violations
description: Touch target sizing violation (buttons can shrink to 20dp), contrast failures across all four color schemes (all below 4.5:1 WCAG AA)
metadata:
  type: project
---

## Touch Target Violation

**Severity:** CRITICAL - fails Android 48dp minimum

**Where:** `android/app/src/main/java/com/geebeeayy/app/ui/screens/EmulationScreen.kt` lines 1350 (48.dp pill height), 1709 (144.dp D-pad)

**Root cause:** Global scale minimum (DisplaySettings.MIN_CONTROL_SCALE = 0.7, line 254 of DisplaySettings.kt) multiplies per-control scale minimum (ControlLayoutStore.MIN_CONTROL_SCALE = 0.6, line 288 of ControlLayoutStore.kt): 0.7 × 0.6 = 0.42x

**Result:** All buttons shrink to 20.16dp at worst case:
- D-Pad arms: 48dp → 20.16dp
- Pill buttons: 48dp → 20.16dp
- Face buttons: 56dp → 23.52dp

**Fix approach:** Increase global MIN_CONTROL_SCALE to at least 0.81 (0.81 × 0.6 = 48.6dp), or enforce minimum rendered size separately from both multipliers.

---

## Contrast Ratio Failures

**Severity:** CRITICAL - all four schemes fail WCAG AA (needs 4.5:1)

**Where:** `android/app/src/main/java/com/geebeeayy/app/data/ControlTint.kt` lines 38-78, colors defined in `ui/theme/Color.kt`

**Measured ratios (label-on-fill at rest):**
- NIGHT: PineGlowMist on NightPanel = 3.81:1 ❌
- HONEY: PineGlowMist on AmberResin = 1.99:1 ❌
- VIOLET: PineGlowMist on NeonViolet = 2.56:1 ❌
- CLASSIC: Color(0xFFE6E6EE) on Color(0xFF4A4A55) = 2.80:1 ❌

**Compounds with opacity:** Control opacity slider (DisplaySettings lines 243-247, range 0.3-1.0) allows 30% opacity, making even marginal ratios unreadable.

**Fix approach:** Redesign palettes to meet 4.5:1 minimum (NIGHT closest at 3.81:1), or add separate high-contrast mode with darker fills/lighter text.

---

## D-Pad Description Limitation

**Severity:** MAJOR - accessibility (TalkBack) - cannot determine active direction

**Path:** EmulationScreen.kt:1582

**Current:** `contentDescription = "Direction pad"` (single generic label)

**Issue:** Canvas-based control: screen reader announces "Direction pad" regardless of direction pressed (up/down/left/right all identical announcement).

**Note:** Not fixable without breaking diagonal input (Canvas design choice). Reasonable compromise but known limitation.

---

## StatusStrip Position Risk

**Severity:** MODERATE - potential overlap with START/SELECT

**Path:** EmulationScreen.kt:591-593, 2252-2268

**Detail:** START_SELECT_BOTTOM = 8dp (line 1317), StatusStrip at ~19dp from bottom (4dp padding + 15px text). Risk of overlap on small screens.

**Mitigation:** Already hidden during editing (line 591: `&& !editingLayout`)

---

## Scale multiplier chain
- User setting: DisplaySettings.getControlScale() returns 0.7-1.0
- Per-control override: ControlLayoutStore allows 0.6-1.8
- Applied via: graphicsLayer with scaleX/scaleY (EmulationScreen.kt line 608-611)
- Affects: touch targets (via Compose's automatic hit test mapping), not just visuals
