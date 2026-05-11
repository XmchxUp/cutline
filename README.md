# Cutline

Cutline is a Rust video cutting tool inspired by
[tsoding/markut](https://github.com/tsoding/markut). It is not compatible with
Markut's stack language. Cutline uses a declarative TOML project file and turns
a linear clip sequence into cached ffmpeg renders plus a final concat output.

## Status

Cutline currently has two usable V1 paths.

Manual video cutting is implemented:

- TOML project files
- linear single-track clip sequences
- multiple named inputs
- millisecond timestamp precision
- clip cache under the project-local `.cutline/` directory
- `check`, `plan`, `render`, and `clean` CLI commands

StoryHighlightVideo reviewable drafts are also implemented:

- validates a local text source range and local background asset
- writes a reviewable draft package under `.cutline/drafts/<story-name>/`
- creates deterministic local short-video script drafts with hook, 3-5 beats,
  adapted narration, ending, and source references
- writes editable `narration.txt` and readable `subtitles.srt`
- can render a 9:16 `preview.mp4` from the configured background and subtitles
  with `cutline story --render-preview`
- records generated assets, source references, and provider step runs in
  `draft.json` and `references.json`
- records an explicit voice step as skipped when no TTS provider is configured
- accepts `voice_provider = "diamoetts"` as an optional future/local provider
  target, but does not download models or require DiaMoE-TTS at runtime

GitHub issue progress: StoryHighlightVideo child issues #2 through #7 are
closed; parent issue #1 remains open as the umbrella tracking item.

`render` and preview rendering require `ffmpeg`; media probing requires
`ffprobe`.

## Manual Cut Example

```toml
[output]
path = "dist/output.mp4"

[input.main]
path = "raw/vod.mp4"

[input.brb]
path = "raw/brb.mp4"

[render]
video_codec = "libx264"
audio_codec = "aac"
video_bitrate = "4000k"
audio_bitrate = "300k"
preset = "veryfast"
crf = 23

[render.extra]
input_args = ["-hwaccel", "auto"]
output_args = ["-movflags", "+faststart"]

[[clip]]
input = "main"
start = "00:10:00"
end = "00:15:00"
chapter = "Opening"

[[clip]]
input = "brb"
start = "0s"
end = "15s"
mute = true
chapter = "Break"

[[clip]]
input = "main"
start = "01:20:00.000"
end = "01:28:30.500"
blur = true
chapter = "Main topic"
```

Relative paths are resolved relative to the project file location, not the
current working directory.

## Planned CLI

```console
$ cutline check project.toml
$ cutline check project.toml --no-probe
$ cutline plan project.toml
$ cutline plan project.toml --no-probe
$ cutline plan project.toml --json
$ cutline render project.toml
$ cutline render project.toml --force
$ cutline clean project.toml
$ cutline story project.toml
$ cutline story project.toml --json
$ cutline story project.toml --json --render-preview
```

`render` refuses to overwrite an existing output unless `--force` is passed.

## StoryHighlightVideo Example

```toml
[output]
path = "dist/story_video.mp4"

[[story]]
name = "chapter1"
source = "stories/chapter1.txt"
start_line = 10
end_line = 50
engagement_angle = "reversal"
background = "assets/default_background.mp4"
platform = "douyin"

# Optional future/local TTS target. This is recorded as a skipped voice step
# unless a local provider implementation and model are configured later.
voice_provider = "diamoetts"
```

Generate the reviewable package:

```console
$ cutline story examples/story.toml --json
```

Generate the package and render a local preview:

```console
$ cutline story examples/story.toml --json --render-preview
```

The package is written to `.cutline/drafts/<story-name>/` and contains:

- `draft.json`: manifest, StoryHighlightVideo draft data, generated asset
  fingerprints, subtitle style, and optional preview/voiceover asset metadata
- `references.json`: source references and pipeline step runs
- `narration.txt`: editable adapted narration
- `subtitles.srt`: readable short-line SRT subtitles
- `preview.mp4`: generated only when `--render-preview` is passed

StoryHighlightVideo is currently a reviewable-draft workflow, not a full final
publish render. Real TTS audio generation is not wired in yet; the voice step is
audited as skipped unless a test/local provider is supplied by code.

## Design

See [docs/DESIGN.md](docs/DESIGN.md).
