# How open-source emulators implement fast forward

Research note, September 2026. Nothing here changes code; it is the evidence
behind a design decision we still have to make.

The question it answers: our fast forward "is not smooth, it feels like slow
motion with skipped frames". Every emulator surveyed below hits the same wall
we did - the audio device is the thing pacing the emulator, and speeding up
means deciding what happens to it - and there turn out to be only four
distinct answers in the whole field.

## What our code does today

Both files were read for this note; line numbers are from the current
`feat/interframe-blending` tree.

`android/app/src/main/java/com/geebeeayy/app/viewmodel/EmulationViewModel.kt`:

- `FAST_FORWARD_BATCH = 4` (line 90). While fast-forwarding the loop calls
  `engine.runFrames(4)` (line 490) instead of `engine.runFrame()`.
- One frame-buffer publish per batch: `_frameBuffer.value =
  engine.getFrameBuffer().copyOf()` (line 494), a 115,200-byte allocation.
- `engine.readAudio(audioSamples)` still runs every batch (line 506), so the
  core's `sample_buffer` is drained and cleared, but the samples are thrown
  away.
- The loop then does `yield(); continue` (lines 530-532), skipping both
  `audio.write` and `delayUntilDeadline`. There is no throttle of any kind.

`android/app/src/main/java/com/geebeeayy/app/engine/AudioOutput.kt`:

- `SAMPLE_RATE = 48_000`, `BUFFERED_FRAMES = 2`, so the AudioTrack holds
  1,600 samples, about 33 ms.
- `write()` uses `AudioTrack.WRITE_BLOCKING` (line 118). That call is the
  emulator's clock at normal speed.

Three consequences follow directly, and together they are a complete
explanation of the reported symptom:

1. **Speed is whatever the device gives, and nothing more.** The comment at
   line 148 already records the measurement: roughly 2.5x real time on a
   phone. If a heavy scene drops that to 1.2x, fast forward delivers 1.2x.
2. **Video cadence collapses.** At 1.2x the loop completes
   `1.2 * 59.73 / 4 = 18` batches per second, so the screen is refreshed 18
   times a second, in jumps of four emulated frames. Eighteen irregular
   updates per second on a 60 or 120 Hz panel is exactly what "slow motion
   with skipped frames" looks like. The four frames are all fully rendered by
   the PPU, and three of the four are then discarded.
3. **Audio is gone, and the AudioTrack is left running.** It underruns
   immediately; nothing repopulates it until fast forward is released.

Two smaller things found while reading, worth recording:

- `audioSamples` is `SAMPLES_PER_FRAME * 8 = 6400` floats, but the core emits
  `48000 / 59.7275 = 803.65` samples per frame, not 800. Eight frames is 6,429
  samples, so a batch of 8 truncates. `geebeeayy_audio_copy`
  (`core/src/ffi.rs:190`) copies `min(len, max_samples)` and then clears the
  whole buffer, so the overflow is silently dropped rather than corrupting
  anything. Any batch size above 7 loses samples.
- `GbaScreen` in
  `android/app/src/main/java/com/geebeeayy/app/ui/screens/EmulationScreen.kt:738`
  converts all 38,400 pixels from RGB to ARGB and calls `Bitmap.setPixels` in
  the composition phase, on the UI thread, once per published frame. Compose
  coalesces recompositions to the display refresh, so publishing more often
  than the display can present is pure waste, and publishing at an irregular
  cadence is visible judder.

---

## One section per emulator

### mGBA

mGBA is the closest match to our problem and has the most interesting answer,
because it keeps audio as the timing master while fast-forwarding.

`CoreController::updateFastForward()`
([src/platform/qt/CoreController.cpp:1304](https://github.com/mgba-emu/mgba/blob/master/src/platform/qt/CoreController.cpp)):

```cpp
if (m_fastForward || m_fastForwardForced) {
    if (m_fastForwardVolume >= 0) {
        m_threadContext.core->opts.volume = m_fastForwardVolume;
    }
    m_threadContext.core->opts.mute = m_fastForwardMute || m_mute;
    setSync(false);

    if(!m_fastForward) {
        if (m_fastForwardRatio > 0) {
            m_threadContext.impl->sync.fpsTarget = m_fpsTarget * m_fastForwardRatio;
            m_threadContext.impl->sync.audioWait = true;
        }
    } else {
        if (m_fastForwardHeldRatio > 0) {
            m_threadContext.impl->sync.fpsTarget = m_fpsTarget * m_fastForwardHeldRatio;
            m_threadContext.impl->sync.audioWait = true;
        }
    }
}
```

`setSync(false)` clears both `audioWait` and `videoFrameWait` (same file, line
537). So:

- **Ratio 0 or negative ("Unbounded" in the settings dialog)**: no sync at all,
  free-run. This is what our current implementation does.
- **Ratio > 0**: video sync stays off, and `audioWait` is turned back on. The
  emulator is still paced by the audio device.

The second case only works because the resample ratio is scaled by the target
frame rate. In
[src/platform/sdl/sdl-audio.c](https://github.com/mgba-emu/mgba/blob/master/src/platform/sdl/sdl-audio.c):

```c
if (audioContext->sync->fpsTarget > 0 && audioContext->core) {
    fauxClock = mCoreCalculateFramerateRatio(audioContext->core, audioContext->sync->fpsTarget);
}
...
mAudioResamplerSetSource(&audioContext->resampler, buffer, sampleRate / fauxClock, true);
```

At a 2x ratio the resampler is told the source runs at half the rate, so it
consumes two input samples for every output sample. The device still drains
48,000 samples a second, the core must therefore produce 96,000, and the
throttle comes out at exactly 2x with no timers involved. The cost is pitch:
the sound goes up an octave at 2x. mGBA does not attempt to correct it; the
maintainer's position on the request was that normalising the pitch would need
DSP work he did not want in the project
([issue #846](https://github.com/mgba-emu/mgba/issues/846)).

`mCoreSyncProduceAudio` in
[src/core/sync.c](https://github.com/mgba-emu/mgba/blob/master/src/core/sync.c)
is the block itself: `while (sync->audioWait && sync->audioHighWater &&
producedNew >= sync->audioHighWater)` waits on a condition variable. It is a
condition wait on a high-water mark, not a blocking device write, which is a
detail worth noting: mGBA can be woken by a shutdown, and it has a 50 ms
timeout on the video side.

User-facing controls
([src/platform/qt/SettingsView.cpp](https://github.com/mgba-emu/mgba/blob/master/src/platform/qt/SettingsView.cpp),
lines 554-570 and 735-736):

- `fastForwardRatio` and a separate `fastForwardHeldRatio`, each with an
  "Unbounded" checkbox that stores -1. Toggle and hold get different speeds.
- `fastForwardVolume` (-1 means "use the normal volume") and
  `fastForwardMute` (default false). Separate from the normal mute, which is
  a distinction users asked for explicitly
  ([issue #1424](https://github.com/mgba-emu/mgba/issues/1424),
  [issue #3547](https://github.com/mgba-emu/mgba/issues/3547)).

### RetroArch / libretro

RetroArch has the most settings and the most defensive defaults.

From
[config.def.h](https://github.com/libretro/RetroArch/blob/master/config.def.h):

```c
/* Maximum fast forward ratio. */
#define DEFAULT_FASTFORWARD_RATIO 0.0f
#define MAXIMUM_FASTFORWARD_RATIO 50.0f
/* Skip frames when fast forwarding. */
#define DEFAULT_FASTFORWARD_FRAMESKIP true
/* Automatically mute audio when fast forward is enabled. */
#define DEFAULT_AUDIO_FASTFORWARD_MUTE false
/* Speed up audio to match fast forward speed up. */
#define DEFAULT_AUDIO_FASTFORWARD_SPEEDUP false
/* Slowmotion ratio. */
#define DEFAULT_SLOWMOTION_RATIO 3.0f
#define DEFAULT_RATE_CONTROL_DELTA  0.005f
#define DEFAULT_AUDIO_SYNC true
#define DEFAULT_VSYNC true
```

So the shipped default is unlimited speed (0 means no cap), capped in the UI
at 50x, with video frame skipping on, audio not muted, and audio pitch not
raised.

Audio, in
[audio/audio_driver.c](https://github.com/libretro/RetroArch/blob/master/audio/audio_driver.c),
has three distinct modes inside `audio_driver_flush`:

1. Normal: dynamic rate control. `src_ratio_curr = src_ratio_orig *
   audio_driver_compute_rate_adjust(...)`, which reads the driver FIFO's
   write-available count, compares it against a half-full setpoint and nudges
   the resample ratio by at most `rate_control_delta` (0.5%). This is what
   keeps vsync and audio from fighting; the
   [Dynamic Rate Control doc](https://docs.libretro.com/development/cores/dynamic-rate-control/)
   describes 0.5% as inaudible.
2. Fast forward with `audio_fastforward_speedup` on: the ratio is multiplied
   by `audio_driver_ff_mult(...)`, the mGBA approach, pitch and all.
3. Fast forward with speedup off, which is the **default**:

```c
if (is_fastforward && !config_get_ptr()->bools.audio_fastforward_speedup)
   rs_frames = (unsigned)audio_driver_ff_discard_bound(audio_st, i16_ratio, rs_frames);
```

`audio_driver_ff_discard_bound` computes how many output frames the device can
actually accept right now and caps the resampler's *input* to
`max_out_frames / ratio`, with a comment saying it shrinks the input rather
than the ratio precisely so the pitch is not altered. In other words: normal
pitch, and whole chunks of the soundtrack are simply skipped over. That is
what RetroArch fast forward sounds like out of the box.

Muting is a separate config (`audio_fastforward_mute`, and the setting appears
in the menu as "Mute When Fast-Forwarding",
[XMB Menu Map](https://docs.libretro.com/guides/xmb-menu-map/)).

`fastforward_frameskip` defaulting to true is the video half: presentation is
skipped while fast-forwarding so the GPU and the compositor are not the
bottleneck.

Note also `DEFAULT_OUT_LATENCY 128` on Android against 64 elsewhere, with the
comment "For most Android devices, 64ms is way too low". The
[optimal vsync guide](https://docs.libretro.com/guides/optimal-vsync/)
similarly argues against pushing audio latency down.

A standing feature request asks for real-time-pitch audio during fast forward
and cites Azahar as the emulator that has it
([issue #18890](https://github.com/libretro/RetroArch/issues/18890)); there is
no maintainer response on it.

### Dolphin

Dolphin's speed control is a single float, `MAIN_EMULATION_SPEED`, default
`1.0f`
([Source/Core/Core/Config/MainSettings.cpp:233](https://github.com/dolphin-emu/dolphin/blob/master/Source/Core/Core/Config/MainSettings.cpp)),
where `0` means unlimited. The Android UI exposes it as an "Emulation speed"
slider, not a fast-forward toggle.

Throttling is in
[Source/Core/Core/CoreTiming.cpp](https://github.com/dolphin-emu/dolphin/blob/master/Source/Core/Core/CoreTiming.cpp).
`UpdateSpeedLimit` converts the speed into `m_throttle_adj_clock_per_sec`, and
`IsSpeedUnlimited()` is simply `m_throttle_adj_clock_per_sec == 0`. `Throttle()`
carries a reference cycle and a reference time forward in whole seconds "to
avoid drifting from cumulative rounding errors", then `SleepUntil(target_time)`.
It also relaxes the target when the CPU cannot keep up, unless the user asked
for "Correct Time Drift":

```cpp
const TimePoint min_target = time - m_max_fallback;
if (!m_correct_time_drift && target_time < min_target) {
  const DT adjustment = min_target - target_time;
  m_throttle_reference_time += adjustment;
  target_time += adjustment;
}
```

Video: `UpdateVISkip` sets `m_throttle_disable_vi_int` when the CPU is lagging
by a fixed amount, and `GetVISkip()` gates it on the `bVISkip` video option.
Presentation is dropped, emulation is not.

Audio: the old SoundTouch `AudioStretcher.cpp` is **gone** from the tree (both
`Source/Core/AudioCommon/AudioStretcher.cpp` and its header 404 on master).
The current `Mixer::MixerFifo::Mix`
([Source/Core/AudioCommon/Mixer.cpp:63](https://github.com/dolphin-emu/dolphin/blob/master/Source/Core/AudioCommon/Mixer.cpp))
resamples out of a windowed granule queue and does this:

```cpp
const double emulation_speed = m_mixer->m_config_emulation_speed;
if (!m_mixer->m_config_audio_preserve_pitch && 0 < emulation_speed && emulation_speed != 1.0)
  in_sample_rate *= emulation_speed;
```

`MAIN_AUDIO_PRESERVE_PITCH` defaults to `false`, so Dolphin's default is the
same pitch-shifting resample as mGBA. Turning it on leaves the input rate
alone and lets the overlap-add granule queue drop or repeat granules with a
crossfade, which is a time stretch that keeps pitch. `MAIN_AUDIO_FILL_GAPS`
(default true) and `MAIN_AUDIO_BUFFER_SIZE` (default 80 ms) are the companions.

### PPSSPP

PPSSPP's fast forward makes the throttle disappear, and the audio ring buffer
is deliberately kept shallow while it is on.

[Core/HLE/sceDisplay.cpp](https://github.com/hrydgard/ppsspp/blob/master/Core/HLE/sceDisplay.cpp):

```cpp
if (PSP_CoreParameter().fastForward)
    return 0;              // FrameTimingLimit() == 0
...
static bool FrameTimingThrottled() { return FrameTimingLimit() != 0; }
```

Alternate speeds are `iFpsLimit1` and `iFpsLimit2`, two user-configurable
values, plus an analog-trigger-driven `FPSLimit::ANALOG`. `DoFrameTiming`
carries `nextFrameTime` forward by a scaled timestep, refuses to fall more
than 5.5 frames behind (`maxFallBehindFrames`), and jumps the deadline outright
if the gap is more than two timesteps, with the comment "If time gap is huge
just jump (somebody fast-forwarded)".

Video: PPSSPP has both frameskip (`iFrameSkip`, plus auto-frameskip when the
speed limit is not the default) and a separate present-side throttle:

```cpp
bool refreshRateNeedsSkip = (fpsLimit != framerate && fpsLimit > refreshRate) || !throttle;
if (g_frameTiming.FastForwardNeedsSkipFlip() && (!FrameTimingThrottled() || refreshRateNeedsSkip)) {
    if ((now - lastFlip) < 1.0f / refreshRate) forceNoFlip = true;
    else lastFlip = now;
}
```

That is the direct answer to "vsync fighting the speed-up on mobile": when
running faster than the panel can present, PPSSPP renders everything but
presents at most once per display refresh interval.

Audio
([Core/HW/StereoResampler.cpp](https://github.com/hrydgard/ppsspp/blob/master/Core/HW/StereoResampler.cpp)):

```cpp
u32 cap = maxBufsize_ * 2;
// If fast-forwarding, no need to fill up the entire buffer, just screws up timing after releasing the fast-forward button.
if (PSP_CoreParameter().fastForward) {
    cap = targetBufsize_ * 2;
}
if (numSamples * 2 + ((indexW - indexR_.load()) & INDEX_MASK) >= cap) {
    if (!PSP_CoreParameter().fastForward) overrunCount_++;
    return;   // drop the samples
}
```

The push is non-blocking in all cases (it drops on overrun), overruns are not
even counted during fast forward, and the buffer is capped shallower so that
releasing the button does not leave a deep queue of stale audio playing.

### SkyEmu

SkyEmu has one integer, `emu_state.step_frames`, that covers slow motion,
normal, fast forward and unlocked, in
[src/main.c](https://github.com/skylersaleh/SkyEmu/blob/dev/src/main.c)
(around line 5354):

```c
int max_frames_per_tick = 1 + emu_state.step_frames;
emu_state.render_frame = true;
double sim_fps = se_get_sim_fps();                 // 59.727 for GBA
double sim_time_increment = 1./sim_fps/emu_state.step_frames;
if(emu_state.step_frames<0){                        // slow motion
    max_frames_per_tick = 1;
    sim_time_increment = 1./sim_fps*-emu_state.step_frames;
}
bool unlocked_mode = emu_state.step_frames==0;
if(unlocked_mode && emu_state.run_mode!=SB_MODE_STEP){
    sim_time_increment = 0;
    max_frames_per_tick = 1000;
    simulation_time = curr_time+1./30.;
}
while(max_frames_per_tick--){
    ...
    se_emulate_single_frame();
    simulation_time += sim_time_increment;
    emu_state.frame++;
    emu_state.render_frame = false;                 // only the first frame of a batch renders
    curr_time = se_time();
}
```

Two things matter here for us:

- The batch is driven by a wall-clock deadline (`simulation_time` vs
  `curr_time`), not by a fixed count, and even in unlocked mode it caps the
  catch-up at 1/30 s of simulated time so a stall cannot turn into a sprint.
- `emu_state.render_frame` is set true only for the *first* frame of the batch,
  and is passed into the core: `gba_tick_ppu(gba, emu->render_frame)`
  ([src/gba.h:3814](https://github.com/skylersaleh/SkyEmu/blob/dev/src/gba.h)).
  Inside `gba_tick_ppu`, `gba_ppu_compute_max_fast_forward(gba, render)`
  lets the PPU skip ahead in large steps when it does not have to produce
  pixels. Skipped frames are genuinely cheap, not merely undisplayed.

Audio is a plain ring buffer pushed from the frontend at whatever rate
`saudio_expect()` asks for, with an underrun path that resets the ring. Slow
motion is implemented by repeating each sample `-step_frames` times; fast
forward has no audio special-casing at all, so surplus samples are dropped by
the ring.

### NanoBoyAdvance

NBA is the clean counter-example: **audio never paces the emulator at all.**

`RingBuffer::Write`
([src/nba/include/nba/common/dsp/ring_buffer.hh](https://github.com/nba-emu/NanoBoyAdvance/blob/master/src/nba/include/nba/common/dsp/ring_buffer.hh))
returns without writing when the buffer is full, and the SDL callback pulls
([src/nba/src/hw/apu/callback.cc](https://github.com/nba-emu/NanoBoyAdvance/blob/master/src/nba/src/hw/apu/callback.cc)),
looping over whatever is available when it underruns. The producer never
blocks.

Pacing is a wall-clock frame limiter
([src/platform/core/src/frame_limiter.cc](https://github.com/nba-emu/NanoBoyAdvance/blob/master/src/platform/core/src/frame_limiter.cc)):

```cpp
const bool limiting_fps = !m_fast_forward || m_fast_forward_speed > 0;
if(limiting_fps) {
  const int fps_multiplier = m_fast_forward && m_fast_forward_speed > 0 ? m_fast_forward_speed : 1;
  m_timestamp_target += std::chrono::microseconds(m_frame_duration / fps_multiplier);
}
frame_advance();
...
if(limiting_fps) std::this_thread::sleep_until(m_timestamp_target);
```

Note the deadline is carried forward by a fixed period and then slept to,
which is the same shape as our `advanceDeadline` helper. NBA does *not* snap
the deadline to now when it overruns, so a device that cannot keep up
accumulates debt; our version is stricter here.

The unit of work is a quarter-frame (`k_cycles_per_subframe = 280896 / 4`,
[emulator_thread.hh](https://github.com/nba-emu/NanoBoyAdvance/blob/master/src/platform/core/include/platform/emulator_thread.hh))
so input is sampled four times per frame.

User control
([src/platform/qt/src/widget/main_window.cc:264](https://github.com/nba-emu/NanoBoyAdvance/blob/master/src/platform/qt/src/widget/main_window.cc)):
a "Fast forward speed" menu offering exactly `∞, 2x, 4x, 8x`, with the config
default `fast_forward_speed = 2`
([src/platform/qt/src/config.hh:81](https://github.com/nba-emu/NanoBoyAdvance/blob/master/src/platform/qt/src/config.hh)),
plus a "Hold fast forward key" boolean that switches the button between hold
and toggle.

### VBA-M

Two reasons this one is worth reading closely: it is a GBA emulator, and it
has a real AAudio backend, so its Android answer is directly comparable to
ours.

The core-side turbo block is in
[src/core/gba/gba.cpp](https://github.com/visualboyadvance-m/visualboyadvance-m/blob/master/src/core/gba/gba.cpp)
around line 5848:

```cpp
if (turbo_button_pressed) {
    if (coreOptions.speedup_frame_skip)
        framesToSkip = coreOptions.speedup_frame_skip;
    else {
        if (!speedup_throttle_set && coreOptions.throttle != coreOptions.speedup_throttle) {
            last_throttle = coreOptions.throttle;
            soundSetThrottle(DowncastU16(coreOptions.speedup_throttle));
            speedup_throttle_set = true;
        }
        if (coreOptions.speedup_throttle_frame_skip)
            framesToSkip += static_cast<int>(std::ceil(double(coreOptions.speedup_throttle) / 100.0) - 1);
    }
    if (coreOptions.speedup_mute && !current_volume_saved) {
        current_volume = soundGetVolume();
        current_volume_saved = true;
        soundSetVolume(0);
    }
}
```

Defaults
([src/core/base/system.h](https://github.com/visualboyadvance-m/visualboyadvance-m/blob/master/src/core/base/system.h)):
`speedup_mute = true`, `speedup_frame_skip = 9`, `speedup_throttle = 100`,
`throttle = 100`. So out of the box turbo means: mute, skip nine frames out of
ten, keep the normal throttle. The alternative branch, chosen by setting
`speedup_frame_skip` to 0, changes the *sound driver's* throttle percentage
instead - `setThrottle` is part of the `SoundDriver` interface
([src/core/base/sound_driver.h](https://github.com/visualboyadvance-m/visualboyadvance-m/blob/master/src/core/base/sound_driver.h)),
so the audio device is where speed is defined. The option descriptions are in
[option-internal.cpp:683-691](https://github.com/visualboyadvance-m/visualboyadvance-m/blob/master/src/wx/config/internal/option-internal.cpp)
("Set throttle for speedup key (0-3000 %, 0 = no throttle)").

The AAudio backend
([src/wx/audio/internal/aaudio.cpp](https://github.com/visualboyadvance-m/visualboyadvance-m/blob/master/src/wx/audio/internal/aaudio.cpp))
is the interesting part. Its `write()`:

```cpp
const bool pace = throttle_.load(std::memory_order_relaxed) == 100;
if (pace) {
    // Pace the emulator on the queue depth, in short sleeps. This runs on the
    // thread that also drives the frame loop and the UI, so a long block here
    // is both video judder and -- because it makes this producer bursty --
    // a cause of the very underruns it is meant to prevent. The deadline
    // keeps a stalled or stopped stream from wedging emulation.
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(50);
    while (Queued() > target_) {
        if (std::chrono::steady_clock::now() >= deadline) break;
        std::this_thread::sleep_for(std::chrono::microseconds(500));
    }
}
PushSamples(..., pace);
```

and inside `PushSamples`, when the ring is full:

```cpp
if (pace) {
    break;  // paced writes already waited; drop the tail
}
// Turbo: never stall the emulator. Ask the consumer to drop what it
// has so the audio that does play stays close to the present.
flush_.store(true, std::memory_order_release);
```

Two lessons. First, they deliberately do **not** use one long blocking write:
they poll the queue depth in 500 microsecond sleeps with a 50 ms escape hatch,
because a long block makes the producer bursty and causes the underruns it is
meant to prevent. Second, turbo flips a single `pace` boolean, and the
overflow path asks the consumer to flush so what you hear stays near the
present rather than lagging.

### Snes9x

Same shape, minimal machinery.
[unix/unix.cpp](https://github.com/snes9xgit/snes9x/blob/master/unix/unix.cpp):

```c
void S9xSyncSpeed (void)
{
    if (Settings.SoundSync) { return; }     // audio is the clock; nothing to do
    ...
    if (Settings.TurboMode)
    {
        if ((++IPPU.FrameSkip >= Settings.TurboSkipFrames) && !Settings.HighSpeedSeek) {
            IPPU.FrameSkip = 0; IPPU.SkippedFrames = 0; IPPU.RenderThisFrame = TRUE;
        } else {
            IPPU.SkippedFrames++; IPPU.RenderThisFrame = FALSE;
        }
        return;
    }
```

and in the ALSA output path of the same file:

```c
if (Settings.SoundSync && !Settings.TurboMode && !Settings.Mute) {
    snd_pcm_nonblock(so.pcm_handle, 0);      // blocking: audio paces the emulator
    frames = samples_to_write/2;
} else {
    snd_pcm_nonblock(so.pcm_handle, 1);      // non-blocking: drop the surplus
    frames = MIN(frames, samples_to_write/2);
}
```

`Settings.TurboSkipFrames = 15` is the default, so turbo renders one frame in
sixteen. This is the single clearest statement of the pattern anywhere in this
survey: **turbo is exactly "switch the audio write from blocking to
non-blocking, and stop rendering most frames".**

### Mesen

[Core/Shared/Emulator.cpp:756](https://github.com/SourMesen/Mesen2/blob/master/Core/Shared/Emulator.cpp):

```cpp
double Emulator::GetFrameDelay()
{
    uint32_t emulationSpeed = _settings->GetEmulationSpeed();
    double frameDelay;
    if (emulationSpeed == 0) frameDelay = 0;
    else { frameDelay = 1000 / GetFps(); frameDelay /= (emulationSpeed / 100.0); }
    return frameDelay;
}
```

Defaults in
[Core/Shared/SettingTypes.h](https://github.com/SourMesen/Mesen2/blob/master/Core/Shared/SettingTypes.h):
`EmulationSpeed = 100`, `TurboSpeed = 300`, `RewindSpeed = 100`. A wall-clock
`FrameLimiter` does the throttling, and run-ahead is disabled whenever the
speed is above 100.

Audio has dynamic rate control, and it is **switched off outside normal
speed**
([Core/Shared/Audio/SoundResampler.cpp](https://github.com/SourMesen/Mesen2/blob/master/Core/Shared/Audio/SoundResampler.cpp)):

```cpp
if(stats.AverageLatency > 0 && _emu->GetSettings()->GetEmulationSpeed() == 100) {
    // try to stay within +/- 3ms of requested latency, adjust rate by at most 0.0025
```

Below 100% speed, `SoundMixer` pitch-adjusts to slow the audio down and mutes
outright if the 64 KB buffer cannot hold the stretched result
([SoundMixer.cpp:146](https://github.com/SourMesen/Mesen2/blob/master/Core/Shared/Audio/SoundMixer.cpp)).
Above 100% it does nothing special; the surplus is dropped by the device
buffer.

### BizHawk

BizHawk is the one that treats "which clock is the master" as a first-class,
user-visible choice.
[src/BizHawk.Client.EmuHawk/MainForm.cs](https://github.com/TASEmulators/BizHawk/blob/master/src/BizHawk.Client.EmuHawk/MainForm.cs):

```csharp
public bool IsTurboing => InputManager.ClientControls["Turbo"] || IsTurboSeeking;
public bool IsFastForwarding => InputManager.ClientControls["Fast Forward"] || IsTurboing;
...
int speedPercent = fastForward ? Config.SpeedPercentAlternate : Config.SpeedPercent;
DisableSecondaryThrottling = Config.Unthrottled || turbo || fastForward || rewind;
_throttle.signal_unthrottle = Config.Unthrottled || turbo;
_throttle.signal_overrideSecondaryThrottle = (fastForward || rewind) && (Config.SoundThrottle || Config.VSyncThrottle || Config.VSync);
_throttle.SetSpeedPercent(speedPercent);
```

Config defaults
([src/BizHawk.Client.Common/config/Config.cs](https://github.com/TASEmulators/BizHawk/blob/master/src/BizHawk.Client.Common/config/Config.cs)):
`SpeedPercent = 100`, `SpeedPercentAlternate = 400`, `ClockThrottle = true`,
`VSyncThrottle = false`, `SoundThrottle = false`, `SoundEnabledRWFF = true`,
`SoundVolumeRWFF = 50`.

The important idea is `signal_overrideSecondaryThrottle`: fast forward
*suspends the audio and vsync throttles specifically* and leaves only the
clock throttle running. Two distinct speeds exist, "fast forward" (a
multiplier, clock-throttled) and "turbo" (fully unthrottled).

Both video and audio work are pushed down into the core as flags:

```csharp
bool renderSound = (Config.SoundEnabled && !IsTurboing) || _currAviWriter?.UsesAudio is true;
bool render = !_throttle.skipNextFrame || _currAviWriter?.UsesVideo is true || atTurboSeekEnd;
bool newFrame = Emulator.FrameAdvance(InputManager.ControllerOutput, render, renderSound);
```

There is also a separate rewind/fast-forward volume (default 50%) rather than
a mute, so you still get audible feedback.

### Citra / Azahar

Azahar (the maintained Citra fork) is the emulator RetroArch users point at
for pitch-correct fast forward. It keeps SoundTouch time stretching in the
audio callback
([src/audio_core/dsp_interface.cpp](https://github.com/azahar-emu/azahar/blob/master/src/audio_core/dsp_interface.cpp)):

```cpp
if (performing_time_stretching) {
    const std::vector<s16> in{fifo.Pop()};
    const std::size_t num_in{in.size() / 2};
    frames_written = time_stretcher.Process(in.data(), num_in, buffer, num_frames);
}
```

with a flush path for the transition back out of stretching, so there is no
click. Settings
([src/common/settings.h](https://github.com/azahar-emu/azahar/blob/master/src/common/settings.h)):
`frame_limit{100, 0, 1000}`, a separate `turbo_limit{200, 0, 1000}`, and
`enable_audio_stretching{true}`. A dedicated turbo speed defaulting to 200%,
with pitch preserved, is the most user-friendly configuration in this whole
survey - and it is also the most expensive one, because time stretching costs
CPU on every output buffer.

### Not verified

Stated here only so it is clear what was *not* checked, rather than guessed at:

- **Skyline** (Android Switch emulator) - the repository was taken down in
  2023; no source was inspected and no claim is made.
- **yuzu / Citron** - not inspected for this note. yuzu's `speed_limit`
  percentage setting is well known but the audio behaviour during it was not
  read from source, so nothing is asserted.
- **My Boy!, Pizza Boy** - closed source. Their store listings advertise a
  fast-forward speed control, but no primary source describes what they do to
  the audio, and no secondary source was found that was worth citing. Treat
  any claim about them as unverified.
- **mGBA's own Android build** - only the Qt and SDL frontends were read. The
  sync core (`src/core/sync.c`) is shared, so the mechanism described above
  holds, but the Android-specific settings surface was not checked.

---

## What they all agree on

1. **There are exactly four strategies, and every emulator picks one or two.**
   - *Unthrottle*: remove the clock entirely, run as fast as the CPU allows.
     RetroArch ratio 0, mGBA "unbounded", PPSSPP fast forward, BizHawk turbo,
     Snes9x turbo, SkyEmu unlocked mode. This is what we do.
   - *Scale the wall clock*: keep a deadline, divide the period by the
     multiplier. NanoBoyAdvance, Mesen, Dolphin, BizHawk fast forward.
   - *Scale the audio clock*: keep the audio device as the master and change
     the resample ratio so it consumes N times as many input samples. mGBA
     with a ratio > 0, Dolphin without preserve-pitch, RetroArch with
     `audio_fastforward_speedup`, VBA-M's `soundSetThrottle`.
   - *Time stretch*: keep the audio device as the master and stretch the
     samples to preserve pitch. Azahar (SoundTouch), Dolphin with
     `AudioPreservePitch`.
2. **Nobody keeps a blocking audio write during fast forward.** Either the
   write becomes non-blocking and drops (Snes9x's `snd_pcm_nonblock(handle,
   1)`, PPSSPP's early return, VBA-M's `pace = false`, NanoBoyAdvance always),
   or the blocking behaviour is retained but the rate it blocks at is scaled
   (mGBA, Dolphin). There is no third option, and doing neither - which is
   what we do now, by not writing at all - is what leaves the device
   underrunning.
3. **Video frames are skipped, and the skip is pushed into the core.** Snes9x
   `IPPU.RenderThisFrame`, SkyEmu `emu_state.render_frame` reaching
   `gba_tick_ppu`, BizHawk's `render` argument to `FrameAdvance`, RetroArch
   `fastforward_frameskip` (default on), VBA-M `speedup_frame_skip` (default
   9). The point is not just to present less; it is to make the frames you
   are not going to show *cheap*.
4. **Presentation is clamped to the display, separately from emulation.**
   PPSSPP's `FastForwardNeedsSkipFlip` skips the flip if less than
   `1/refreshRate` has passed. Dolphin's VI skip does the same at the
   interrupt level. On a phone this is the difference between a smooth
   fast forward and a juddering one: emulate as fast as you like, but never
   try to present more often than the panel refreshes, and never present at
   an irregular cadence.
5. **Dynamic rate control is a normal-speed feature and gets turned off.**
   Mesen gates it on `EmulationSpeed == 100`; RetroArch replaces it with the
   discard bound. DRC's job is trimming a 0.5% drift, which is meaningless at
   200%.
6. **The speed is user-visible and usually capped.** Defaults observed:
   NanoBoyAdvance 2x, Azahar 200%, Mesen 300%, BizHawk 400%, RetroArch
   unlimited (UI cap 50x), mGBA unbounded, VBA-M "skip 9 of 10 frames".
   Several offer separate hold and toggle speeds (mGBA, NanoBoyAdvance,
   BizHawk).
7. **Audio during fast forward gets its own volume, not just its own mute.**
   mGBA `fastForwardVolume` plus `fastForwardMute`, BizHawk `SoundVolumeRWFF`
   at 50%, RetroArch `audio_fastforward_mute`, VBA-M `speedup_mute` default
   true. Muting is common but not universal, and the emulators that thought
   hardest about it went for attenuation instead.

---

## Recommendation for our architecture

### The core problem, stated precisely

`AudioOutput.write` with `WRITE_BLOCKING` is our clock. Fast forward has to
answer one question: **do we keep that clock and speed it up, or do we drop it
and build another one?** Right now we do neither - we delete the clock and run
open loop - and that is the whole bug.

Keeping it is strictly better for us, because it is the only option that needs
no new timing code, cannot drift, and self-limits gracefully on a device that
cannot reach the requested speed. This is mGBA's ratio > 0 path and Dolphin's
default, and it is the one to copy.

### The change, concretely

**1. Make the fast-forward batch a real-time frame, and decimate its audio.**

In `EmulationViewModel.startEmulation()`, replace the
`if (fastForwarding) { yield(); continue }` escape at lines 530-532 with a
normal `audio.write`, and add one decimation step. With a ratio `R`:

- `engine.runFrames(R)` produces about `R * 803.65` samples in `audioSamples`.
- Reduce them to about `803.65` by averaging each run of `R` consecutive
  samples into one. Averaging rather than taking every Rth sample matters: a
  plain decimation aliases the GBA's square waves into audible garbage, and a
  box average is a cheap low-pass that costs one add per sample.
- `audio.write(out, count / R)` then blocks for exactly one real frame period,
  because the device drains 803.65 samples in that time.

The throttle is now exact and automatic: the loop runs at `R` times real time
and no faster, publishes the frame buffer once per real frame period (about
60 times a second at any ratio), and `delayUntilDeadline` and
`frameDeadlineNanos` stay exactly as they are for the no-audio-device fallback.
No new clock, no `yield()` spin, no deadline arithmetic. The pitch goes up by
`R`, which is what mGBA, Dolphin and RetroArch's speedup mode all sound like.

Two details:

- `audioSamples` is 6,400 floats and one frame is 803.65 samples, so it holds
  7.96 frames. Raise it to `SAMPLES_PER_FRAME * 12` (or size it from the
  maximum ratio) before allowing a ratio above 7, or the last frames of a
  batch are silently truncated by `geebeeayy_audio_copy`.
- If the device cannot sustain `R`, the write simply stops being the
  bottleneck and the speed settles at whatever the CPU gives, with the audio
  underrunning. That is the same failure mode we have today, but it is now the
  worst case rather than the only case.

**2. Do not publish the frame buffer more than once per batch.** With the
change above this is free: one batch is one real frame period. Keep line 494
where it is. What must not happen is the current arrangement, where the
publish rate is `speed * 59.73 / 4` and therefore both irregular and, at
realistic speeds, well under the display refresh rate. Every emulator that
looks smooth clamps presentation to the display; see PPSSPP's
`FastForwardNeedsSkipFlip` and Dolphin's VI skip.

**3. Give the frontend a way to say "do not render this frame".** This is the
one part that belongs in `core/`, and it is the difference between fast
forward that is capped by the PPU and fast forward that is capped by the CPU.
Add a render flag to `Gba::run_frames` (or a `Gba::set_render_enabled`), plumb
it through `geebeeayy_run_frames` in `core/src/ffi.rs` and
`nativeRunFrames`, and have the PPU skip pixel composition on frames the
frontend will not show, exactly as SkyEmu passes `render` into
`gba_tick_ppu` and BizHawk passes it into `FrameAdvance`. Only the last frame
of each batch needs to be rendered.

Two cautions specific to us. Our PPU now draws each scanline at its HBlank
(commit 2d70336), so the skip has to be inside the per-scanline draw, not a
"skip the frame" branch at the top. And interframe blending (commit 114d3ec)
averages each frame with the one before it; if `R - 1` frames out of `R` are
not rendered, blending would average frames `R` apart and smear badly.
Blending should be bypassed while fast-forwarding, or fed only rendered
frames.

This step is optional for correctness and worth real speed. Ship steps 1 and 2
first, measure, then decide.

**4. Cap the speed and let the user pick it.** The comment at
`EmulationViewModel.kt:148` argues that fixed 2x/4x/8x steps were dishonest
because the device only reaches about 2.5x. With step 1 that argument
inverts: a ratio is now a promise the audio clock enforces, and asking for 4x
on a device that gives 2.5x degrades to 2.5x with underruns rather than
lying. Suggested control, following NanoBoyAdvance's menu almost exactly:

- **Default 2x.** It is NanoBoyAdvance's default and Azahar's, it is inside
  the measured 2.5x headroom on the phone in question, and at 2x the audio is
  still recognisable, which is what makes fast forward usable for grinding
  and for skipping text.
- Offer **2x, 3x, 4x** and an explicit **Unlimited** that restores today's
  behaviour (no audio write, free-run). Keeping unlimited is worth it: mGBA,
  RetroArch and BizHawk all keep an unbounded mode alongside the ratio for
  seeking through long cutscenes.
- Keep the button a toggle, as it is now. A separate held-vs-toggled ratio
  (mGBA, NanoBoyAdvance) is a nice-to-have, not a first cut.
- Add **"Mute during fast forward"**, default off. With decimated audio at 2x
  the sound is informative rather than painful, so muting should be the user's
  choice, not ours. If a single toggle feels thin, BizHawk's
  `SoundVolumeRWFF = 50` (attenuate rather than mute) is the better idea and
  costs one multiply.

### On vsync and presentation on Android

We do not present through vsync directly - the frame buffer goes into a
`StateFlow`, and Compose recomposes `GbaScreen` on the UI thread, which is
already Choreographer-paced. So vsync cannot fight the *emulation* speed the
way it does in a desktop emulator with a swap-interval-blocked present. What
it does do is silently discard work: every `_frameBuffer.value = ...` is a
115,200-byte allocation and every recomposition converts 38,400 pixels and
calls `Bitmap.setPixels` on the UI thread, so publishing faster than the
display refresh burns CPU and allocator pressure for nothing. After step 1 the
publish rate is one per real frame period, which is the correct amount.

Worth knowing but not recommending: Android has two native ways to do the
"scale the audio clock" trick in the device instead of in our code, and both
are too constrained for us.

- `AudioTrack.setPlaybackRate(int)` retunes the track's consumption rate, but
  the framework source documents "The valid sample rate range is from 1 Hz to
  twice the value returned by `getNativeOutputSampleRate(int)`"
  ([AudioTrack.java, setPlaybackRate](https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/media/java/android/media/AudioTrack.java)),
  which caps us at 2x on a 48 kHz device.
- `AudioTrack.setPlaybackParams(PlaybackParams)` supports arbitrary speeds in
  principle, but the same source warns: "For speeds greater than 1.0f, the
  AudioTrack buffer on configuration must be larger than the speed multiplied
  by the minimum size `getMinBufferSize(...)`". Our track is deliberately
  *smaller* than `getMinBufferSize` (1,600 samples against a measured 3,844;
  see the comment in `AudioOutput.start()`), so this would throw
  `IllegalArgumentException` unless we gave up the low-latency buffer. Not
  worth it.

Doing the decimation ourselves has neither limit and costs one pass over about
1,600 floats per frame.

### Worth considering later: stop using one long blocking write

VBA-M's AAudio backend deliberately does not use a single blocking write. It
polls the queue depth in 500 microsecond sleeps with a 50 ms deadline, with
the comment that a long block "is both video judder and -- because it makes
this producer bursty -- a cause of the very underruns it is meant to prevent",
and that the deadline "keeps a stalled or stopped stream from wedging
emulation". We have the same hazard in a sharper form: `onCleared()` already
carries a 250 ms shutdown timeout and a comment about leaking the handle
rather than releasing an AudioTrack under a live blocking write
(`EmulationViewModel.kt:812-843`). Replacing the blocking write with a
queue-depth poll would remove that hazard and make fast forward's pacing a
one-line change (`pace = !fastForwarding`) rather than a resampling one. It is
a bigger change than step 1 and should not be bundled with it, but it is the
direction the one Android-native comparable in this survey chose.

---

## Smallest change for us

In order, with the cheapest first:

1. **Decimate and write the audio during fast forward, batch = ratio.**
   Roughly 15 lines in `EmulationViewModel.startEmulation()`, plus enlarging
   `audioSamples`. No core change, no FFI change, no new timing code. This
   alone fixes the reported symptom: the speed becomes an exact 2x, the frame
   buffer is published about 60 times a second again instead of 18, and the
   audio device stops underrunning. Everything else on this list is an
   improvement on top of a fast forward that already works.
2. **Turn `_fastForward: StateFlow<Boolean>` into a ratio** and add the
   settings entries for the ratio and for mute-or-attenuate. UI work, no
   engine work.
3. **Add a render flag to `run_frames` and skip PPU pixel work on unshown
   frames.** Real speed, and the only item that touches `core/`. Needs a test
   in `core/tests/` proving a non-rendered frame leaves the frame buffer
   untouched while registers, DMA, timers and audio still advance, per the
   testing rule in `CLAUDE.md`.
4. **Replace the blocking write with a queue-depth poll.** Larger, and it
   changes normal-speed behaviour, so it deserves its own branch and its own
   measurement.

## Measured afterwards, on the real device

The recommendation above assumed roughly 2.5x of CPU headroom, taken from a
comment in `EmulationViewModel`. That comment was wrong, and the number
changes what fast forward can be. Measured 2026-09-09, Yggdra Union from a
save state, arm64 release core:

| | frames/s | speed |
|---|---|---|
| Mi 10T Pro, one frame at a time | 80 | 1.34x |
| Mi 10T Pro, ratio 2 with the render skip | 87 | **1.45x** |
| Mi 10T Pro, ratio 4 with the render skip | 92 | 1.53x |
| Desktop (WSL2), one frame at a time | 134 | 2.24x |
| Desktop, batch of 4 with the render skip | 192 | 3.22x |

Three things follow.

**The core is the ceiling, not the plumbing.** With the frame-buffer publish,
the rewind snapshots and the audio write all removed from the loop, the phone
still only reached 80 frames/s. The frontend is not what limits fast forward.

**The render skip is worth having and is not the answer.** Skipping the pixel
work on frames that are never shown is about 30% of a frame's cost: 2.24x to
3.22x on the desktop, 1.34x to 1.45x on the phone at ratio 2. Real, and far
from a multiplier.

**Above ratio 2 there is nothing to buy.** Ratio 4 gains 0.08x of speed and
drops the picture from 43 to 23 updates a second, which is the choppiness this
whole change set out to remove.

So the shipped default is 2x, and the honest description of what this change
does is *fast forward is now smooth and evenly paced*, not *fast forward is now
fast*. Making it fast is a core optimisation project: a 2.24x interpreter on a
modern desktop is slow for a GBA, and the profile - not this report - should
decide what to fix first.
