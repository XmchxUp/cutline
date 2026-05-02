# Cutline Design

## Goal

Cutline is a redesigned Rust video cutting tool inspired by Markut. It keeps
Markut's practical VOD editing workflow but does not preserve its stack-based
language or exact CLI behavior.

The first version should produce a complete video editing loop:

1. Load a declarative TOML project file.
2. Validate inputs, clip references, timestamps, and output settings.
3. Derive a linear edit plan.
4. Render clip cache files.
5. Concatenate cached clips into the final output.

## Non-Goals For V1

- Markut DSL compatibility.
- Multi-track timelines.
- Overlay, transitions, B-roll, or audio mixing.
- Twitch chat downloading.
- Chat subtitle rendering.
- Per-clip raw ffmpeg argument injection.
- Source span diagnostics for TOML errors.
- `init` command.

## Project File

Cutline uses TOML. The project file describes named inputs, output settings,
render settings, and a linear sequence of clips.

```toml
[output]
path = "dist/output.mp4"

[input.main]
path = "raw/vod.mp4"

[[clip]]
input = "main"
start = "00:10:00"
end = "00:15:00"
chapter = "Opening"
blur = true
mute = false
```

Relative paths are resolved against the directory containing the project file.

## Timeline Model

V1 supports a single linear sequence of clips. Each clip references exactly one
input. Clip `start` and `end` timestamps are relative to that input file.
Output timestamps are derived by accumulating clip durations in sequence.

Multiple inputs are supported for intro, outro, BRB, or alternate source clips,
but clips never overlap in V1.

## Time

Internal time precision is milliseconds.

Accepted input forms:

- `"12:34"` for `mm:ss`
- `"01:12:34"` for `hh:mm:ss`
- `"01:12:34.567"` for `hh:mm:ss.millis`
- `"754.2s"` for seconds
- `754200` for integer milliseconds

Canonical display format is `HH:MM:SS.mmm`.

## Rendering

V1 uses two-stage rendering:

1. Render each clip into a project-local cache file.
2. Concatenate cached clips into the final output.

Default render output is normalized to MP4/H.264/AAC:

```toml
[render]
video_codec = "libx264"
audio_codec = "aac"
video_bitrate = "4000k"
audio_bitrate = "300k"
pixel_format = "yuv420p"
container = "mp4"
preset = "veryfast"
crf = 23
```

All clips are re-encoded by default. Stream copy optimization is deferred.

## Effects

V1 supports only:

- `blur = true`
- `mute = true`

These effects are enough to prove cache keying, filter generation, and plan
output without building a general filter graph system.

## ffmpeg Options

Cutline exposes common render settings as structured TOML fields. V1 also
allows global escape hatches:

```toml
[render.extra]
input_args = ["-hwaccel", "auto"]
output_args = ["-movflags", "+faststart"]
```

Per-clip raw ffmpeg args are deferred because they complicate validation,
filter composition, and cache invalidation.

## Cache

The cache lives next to the project file:

```text
project/
  project.toml
  .cutline/
    cache/
    concat.txt
    plan.json
```

Cache keys include:

- schema version
- canonical input path
- input metadata, initially size and modified time
- clip start and end
- render settings
- clip effects
- global ffmpeg extra args

## CLI

V1 has four commands:

```console
cutline check project.toml
cutline plan project.toml
cutline render project.toml
cutline clean project.toml
```

`check` parses and validates the project. By default it probes media duration
with `ffprobe`; `--no-probe` skips media probing.

`plan` prints the derived timeline, cache keys, chapter times, and ffmpeg
commands. It defaults to human-readable text and supports `--json`.

`render` renders missing cache clips and then concatenates them. It refuses to
overwrite an existing final output unless `--force` is passed.

`clean` removes this project's cache directory.

## Diagnostics

V1 diagnostics should include structured paths such as `clip[2].end` and
actionable reasons. Pretty source spans are deferred.

Example:

```text
error: invalid clip[2].end
  reason: end must be greater than start
  start: 00:12:00.000
  end:   00:10:00.000
```

## Module Plan

```text
src/
  lib.rs
  main.rs
  cli.rs
  config.rs
  time.rs
  model.rs
  validate.rs
  planner.rs
  ffmpeg.rs
  cache.rs
  error.rs
```
