# Cutline

Cutline is a Rust CLI for declarative video cutting and reviewable short-video
draft generation.

## Current State

Implemented:

- Manual linear video cutting from TOML.
- AutoCut plan generation and rendering through the cached clip pipeline.
- StoryHighlightVideo reviewable draft packages with narration, subtitles,
  source references, step-run records, and optional 9:16 preview rendering.

Still limited:

- Manual rendering is single-track only.
- AutoCut uses ffmpeg-based scene/audio analysis with deterministic fallback.
- StoryHighlightVideo final render currently exports the generated preview;
  real TTS provider integration is not wired in yet.

Requires `ffmpeg` for rendering and `ffprobe` for media probing.

## Quick Commands

```console
$ cutline check project.toml
$ cutline plan project.toml --json
$ cutline render project.toml --force

$ cutline autocut examples/autocut.toml --json
$ cutline render examples/autocut.toml --autocut

$ cutline story examples/story.toml --json
$ cutline story examples/story.toml --json --render-preview
$ cutline render examples/story.toml --story
$ cutline clean project.toml
```

`render` refuses to overwrite existing outputs unless `--force` is passed.

## Inputs

Use the examples as the source of truth for project TOML shape:

- Manual cuts: [examples/basic.toml](examples/basic.toml)
- AutoCut: [examples/autocut.toml](examples/autocut.toml)
- StoryHighlightVideo: [examples/story.toml](examples/story.toml)
- Minimal smoke project: [examples/smoke.toml](examples/smoke.toml)

Relative paths are resolved relative to the project file, not the current
working directory.

## Outputs

- Manual render writes `[output].path`.
- AutoCut `output_mode = "single"` writes `[output].path`.
- AutoCut `output_mode = "multiple"` writes one file per candidate next to the
  configured output, named like `autocut_output-main_autocut-001.mp4`.
- StoryHighlightVideo writes `.cutline/drafts/<story-name>/` with `draft.json`,
  `references.json`, `narration.txt`, `subtitles.srt`, and optional
  `preview.mp4`.
- StoryHighlightVideo render writes `[output].path` from the generated preview.

## References

- Design overview: [docs/DESIGN.md](docs/DESIGN.md)
- Domain language and current context: [CONTEXT.md](CONTEXT.md)
- AutoCut and StoryToVideo ADR: [docs/adr/0001-autocut-and-story-to-video.md](docs/adr/0001-autocut-and-story-to-video.md)
- Reviewable draft package ADR: [docs/adr/0002-reviewable-draft-package-directory.md](docs/adr/0002-reviewable-draft-package-directory.md)
- Draft source media refs ADR: [docs/adr/0003-draft-packages-reference-source-media.md](docs/adr/0003-draft-packages-reference-source-media.md)
- Creative pipeline ADR: [docs/adr/0004-shared-creative-pipeline.md](docs/adr/0004-shared-creative-pipeline.md)
- Platform profiles ADR: [docs/adr/0005-platform-rules-as-profiles.md](docs/adr/0005-platform-rules-as-profiles.md)
- Provider replacement ADR: [docs/adr/0006-creative-providers-are-replaceable.md](docs/adr/0006-creative-providers-are-replaceable.md)
- Step-run records ADR: [docs/adr/0007-draft-packages-record-step-runs.md](docs/adr/0007-draft-packages-record-step-runs.md)
