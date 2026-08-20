# GeeBee-A — Roadmap de Desenvolvimento

Visão geral do plano de desenvolvimento do emulador, dividido por fases e prioridades.

---

## Estado Atual

| Módulo | Status | Linhas |
|--------|--------|--------|
| `lib.rs` | Funcional (loop + DMA + APU) | ~110 |
| `cpu/` | ARM7TDMI completo (ARM + THUMB) | ~900 |
| `ppu/` | Mode 0/1/2/3/4/5 + OBJ + color FX | ~700 |
| `apu/` | 4 canais PSG (square, wave, noise) | ~350 |
| `memory/` | ROM + I/O + wait states + sound routing | ~160 |
| `timer/` | Prescaler + IRQ | ~80 |
| `cart/` | ROM + SRAM/Flash/EEPROM save | ~230 |
| `io/` | I/O register handler | ~150 |
| `dma/` | DMA 4ch (immediate/HBlank/VBlank) | ~170 |

---

## Fase 1 — Core Funcional (MVP)

> **Objetivo:** Emular o hardware suficiente para rodar pelo menos um ROM simples.

### 1.1 CPU — Decodificador de Instruções
- [x] Decodificador ARM (32-bit) — TODAS as instruções
  - [x] ALU (ADD, SUB, AND, ORR, EOR, MOV, MVN, CMP, TST)
  - [x] Multiply (MUL, MLA, UMULL, UMLAL, SMULL, SMLAL)
  - [x] Load/Store (LDR, STR, LDM, STM)
  - [x] Branch (B, BL, BX, BLX)
  - [x] PSR Transfer (MRS, MSR)
  - [x] Multiply Long
  - [x] Swap (SWP, SWPB)
  - [x] Barrel Shifter (LSL, LSR, ASR, ROR)
  - [ ] Coprocessor (暂未 necessário)
- [x] Decodificador THUMB (16-bit) — TODAS as instruções
  - [x] Format 1-19 (todas as categorias)
  - [x] Operações de stack (PUSH, POP)
  - [x] Load/Store de múltiplos
  - [x] Branch condicional e incondicional
- [x] Barrel Shifter completo ( Carry Out )
- [x] Pipeline de 3 estágios correto (PC+8 ARM, PC+4 THUMB)
- [x] Tratamento de interrupções (IRQ/FIQ) — handler básico

### 1.2 Memory Bus
- [x] Conectar ROM ao bus (0x08000000+ → `cartridge.read*`)
- [x] Mirror de ROM (0x09FFFFFF, 0x0AFFFFFF, 0x0BFFFFFF)
- [x] I/O Register decode (mapear registradores do PPU, Timer, DMA, APU)
- [x] Wait States (ciclos de acesso por região)
- [x] Prefetch Buffer (0x04000000+)
- [ ] BIOS execute permission

### 1.3 Timer
- [x] Prescaler (1, 64, 256, 1024)
- [x] IRQ no overflow
- [x] Integração com Memory Bus (TM0CNT_L/H → TM3CNT_L/H)

### 1.4 Cartridge
- [x] Detecção de save type por game code (GBTE, GBXP, etc.)
- [x] Detecção por conteúdo ROM (string "SRAM", "FLASH", "EEPROM")
- [x] Save RAM (SRAM 32KB, Flash 64/128KB, EEPROM)

---

## Fase 2 — Graphics Básico

> **Objetivo:** Renderizar scanlines para ver algo na tela.

### 2.1 PPU — Renderização
- [x] **Mode 0** — 4 backgrounds tiled (4bpp e 8bpp)
  - [x] Tile Data (Char Base)
  - [x] Screen Entry (Screen Base)
  - [x] Scrolling (BG0HOFS/BG0VOFS)
  - [x] Priority
- [x] **Mode 3** — Bitmap 16bpp (1 framebuffer)
- [x] **Mode 4** — Bitmap 8bpp (2 framebuffers)
- [ ] Paleta de cores (256 cores BG, 256 cores OBJ)
- [x] OAM — Sprites básicos (normal, affine)
- [x] WIN0/WIN1/WINOUT (janelas)

### 2.2 PPU — Intermediário
- [x] **Mode 1** — BG0+BG1 tiled, BG2 affine
- [x] **Mode 2** — BG2+BG3 affine
- [x] **Mode 5** — Bitmap 16bpp (2 framebuffers)
- [x] Affine backgrounds (scaling, rotation)
- [x] Affine sprites
- [x] Mosaic

### 2.3 PPU — Avançado
- [x] HBlank / VBlank DMA
- [ ] OAM DMA
- [x] BLDCNT/BLDALPHA (efeitos de blending)
- [x] BLDY (brightness)

---

## Fase 3 — Áudio

> **Objetivo:** Áudio funcional sem crackle.

### 3.1 APU — Canais Básicos
- [x] Canal 1 — PSG Quadrada (square wave)
- [x] Canal 2 — PSG Quadrada
- [x] Canal 3 — PSG Onda (wave)
- [x] Canal 4 — PSG Ruído (noise)
- [x] Sweep (Canal 1)
- [x] Envelope (todos os canais)
- [x] Sound Length Counter

### 3.2 APU — FIFO
- [x] Canal A — Sound A (FIFO/Timer 0/1)
- [x] Canal B — Sound B (FIFO/Timer 2/3)
- [x] DMA Sound
- [x] Mixing (PSG + FIFO)

### 3.3 APU — Sincronização
- [ ] Master timer (Timer 0 como timing master)
- [ ] Double buffering
- [ ] Buffer de áudio com back-pressure
- [ ] Cross-platform audio API (AAudio Android, CoreAudio iOS)

---

## Fase 4 — Android Frontend

> **Objetivo:** App funcional para testar em dispositivo.

### 4.1 UI Básica
- [x] Rom browser com lista de jogos
- [ ] Tela de emulação (OpenGL ES rendering)
- [x] Controles touch na tela
- [x] Menu de pausa

### 4.2 Funcionalidades Core
- [x] Save states (10 slots)
- [x] Fast forward (2x, 4x)
- [ ] Controle Bluetooth/USB (Xbox, PS, Switch Pro)
- [ ] Screen scaling (1x, 2x, 3x, fit)
- [ ] Screen filters (2xSaI, CRT, pixel-perfect)

### 4.3 UX
- [ ] Customização de controles (tamanho, posição, opacidade)
- [ ] ROM com capas e metadata
- [ ] Swipe gestures (rewind, save state)
- [ ] Landscape/Portrait auto-detect

---

## Fase 5 — iOS Frontend

> **Objetivo:** App nativo para iOS.

- [x] SwiftUI UI (Splash, ROM Browser, Emulation, Settings)
- [x] GeeBeeTheme (Color extensions, Design tokens)
- [x] Assets: bee_logo, bee_mascot
- [ ] MFi controller support
- [ ] Touch controls + gesture support
- [ ] Save states + iCloud sync
- [ ] Widget para retomada rápida
- [ ] App Store distribution

---

## Fase 6 — Avançado

> **Objetivo:** Features que diferenciam de outros emuladores.

- [ ] JIT recompilation (ARM host only)
- [ ] Link cable emulation (local WiFi)
- [ ] Cheat codes (GameShark / CodeBreaker)
- [ ] Rewind support
- [ ] Screen recording / screenshots
- [ ] Lua scripting interface
- [ ] Debug tools (breakpoints, memory viewer, register inspector)
- [ ] RetroAchievements support

---

## Ordem de Prioridade Recomendada

```
1. CPU (ARM + THUMB)  ─────┐
2. Memory Bus (ROM, I/O)   ├──→  Fase 1 (Core funcional)
3. Timer completo          │
4. Cart save               ─┘
                             │
5. PPU Mode 0 + Mode 3  ────┤  Fase 2 (Graphics)
6. OAM (sprites básicos)    │
7. Palette + blending       ─┘
                             │
8. APU (PSG channels)    ────┤  Fase 3 (Áudio)
9. FIFO + sync             ─┘
                             │
10. Android frontend     ────┤  Fase 4 (App)
11. Save states, FF       ─┘
```

---

## Referências

| Recurso | Link |
|---------|------|
| GBATEK (hardware reference) | https://problemkaputt.de/gbatek.htm |
| TONC (programação GBA) | https://www.coranac.com/tonc/text/toc.htm |
| ARM7TDMI TRM | https://developer.arm.com/documentation/ddi0029/ |
| mGBA (código de referência) | https://github.com/mgba-emu/mgba |
| SkyEmu (per-pixel PPU) | https://github.com/skylersaleh/SkyEmu |
| rustboyadvance-ng (Rust reference) | https://github.com/rustboyadvance-ng |
