## Parent

https://github.com/XmchxUp/cutline/issues/1

## What to build

Represent the StoryHighlightVideo voice step as an explicit provider-backed pipeline step that can be completed or intentionally skipped. This slice should make optional TTS state auditable without requiring a working TTS provider for the first vertical slice.

When no voice provider is configured, the story command should record the voice step as a SkippedStep with a clear reason. When a simple local voice provider is configured or test provider output is supplied, the package should record a VoiceoverAsset and mark the step completed. DiaMoETtsProvider should be represented as an optional future/local VoiceProvider capability, but this issue does not need to install DiaMoE-TTS or download models.

## Acceptance criteria

- [ ] The story pipeline includes a voice step after narration/subtitle generation.
- [ ] When no voice provider is configured, the voice step is recorded as a SkippedStep with provider, status, reason, and parameters.
- [ ] When a local/test voice provider produces audio, the package records a VoiceoverAsset and marks the voice step completed.
- [ ] The DraftManifest records voice asset metadata when voiceover exists.
- [ ] The absence of voiceover does not block subtitle generation or preview rendering.
- [ ] DiaMoETtsProvider is represented as an optional VoiceProvider target in configuration or provider selection docs/modeling, without making it a required runtime dependency.
- [ ] Tests verify skipped voice state, completed local/test voice state, manifest asset records, and that no-TTS projects still complete the draft pipeline.

## Blocked by

- https://github.com/XmchxUp/cutline/issues/5
