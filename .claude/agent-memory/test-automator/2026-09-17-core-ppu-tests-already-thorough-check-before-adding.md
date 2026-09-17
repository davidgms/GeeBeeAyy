---
name: core-ppu-tests-already-thorough-check-before-adding
description: core/tests/ppu.rs already covers sprite-priority-vs-OAM-index, interframe blend on/off, and render_enabled skip before adding more PPU tests
metadata:
  type: project
---

As of 2026-09-17 (HEAD bffe0a6), `core/tests/ppu.rs` already had thorough,
passing coverage for three things a task brief described as gaps:

- Sprite priority beating OAM index, and OAM index only breaking a priority
  tie: `a_sprite_with_a_better_priority_beats_a_lower_oam_index` and
  `between_sprites_of_equal_priority_the_lower_oam_index_wins` (~line 1028,
  1038).
- Interframe blending on and off: `interframe_blending_averages_each_frame_with_the_one_before`
  and `interframe_blending_off_leaves_the_frame_untouched` (~line 848, 891).
- `render_enabled` skipping the draw, and `run_frames` restoring it after a
  fast-forward batch: `a_frame_the_frontend_will_not_show_is_not_drawn` and
  `run_frames_leaves_rendering_on_for_whoever_runs_next` (~line 924, 949).

The one genuine gap in that cluster was the *interaction*: `blend_with_previous_frame`
(`core/src/ppu/mod.rs` ~line 1581) is guarded on `!self.render_enabled` as
well as `!self.interframe_blend`, and nothing exercised fast-forward
(`render_enabled = false`) with interframe blending turned on at the same
time. Added `interframe_blending_ignores_frames_skipped_by_fast_forward`
right before `a_frame_the_frontend_will_not_show_is_not_drawn`. Confirmed it
actually catches the regression by dropping the `render_enabled` half of
that guard locally and rerunning - `mid` (frame_buffer during the skipped
frame) dropped from 124 to 0, i.e. `blend_out` got flattened back to the
unblended picture, exactly what the code comment two lines above warns
about.

Before adding a PPU/CPU test, grep `core/tests/*.rs` for the feature name
first - this repo's suite is denser than it looks, and the one worthwhile
gap is often in the *interaction* between two already-tested features, not
in the feature itself.

See also [[android-context-classes-pure-function-extraction-pattern]].
