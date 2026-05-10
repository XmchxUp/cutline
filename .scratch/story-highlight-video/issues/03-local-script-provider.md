## Parent

https://github.com/XmchxUp/cutline/issues/1

## What to build

Add a local ScriptProvider for StoryHighlightVideo that turns the configured StorySourceRange and EngagementAngle into a complete ShortVideoDraft content structure. The provider should run without remote AI services and produce deterministic, inspectable output suitable for the first vertical slice.

The generated draft should include one Hook, three to five ContentBeat items, an AdaptedNarration script, one EndingBeat, and SourceReference links back to the source text range. It should update the ReviewableDraftPackage created by the story command so a user can inspect the structured draft before subtitles or preview rendering exist.

## Acceptance criteria

- [ ] The story command invokes a local ScriptProvider for a valid StoryHighlightVideo package.
- [ ] The provider produces exactly one Hook, three to five ContentBeat items, one AdaptedNarration script, and one EndingBeat for the ShortVideoDraft.
- [ ] Generated hook, beats, narration, and ending preserve SourceReference links to line or paragraph ranges in the TextStorySource.
- [ ] The DraftManifest records the generated ShortVideoDraft structure.
- [ ] The ReferenceMap records links from generated creative material back to source text ranges.
- [ ] PipelineStepRun records include the script provider name, parameters, status, and outputs.
- [ ] The implementation does not require cloud LLM access or API keys.
- [ ] Tests cover deterministic local provider output, required ShortVideoDraft structure, EngagementAngle usage, SourceReference mapping, and script step-run records.

## Blocked by

- https://github.com/XmchxUp/cutline/issues/3
