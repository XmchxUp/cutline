## Parent

https://github.com/XmchxUp/cutline/issues/1

## What to build

Generate the user-editable narration and subtitle artifacts for a StoryHighlightVideo ReviewableDraftPackage. The story command should write narration text and a readable SRT subtitle file from the ShortVideoDraft produced by the local ScriptProvider.

This slice should make the package reviewable as text even before preview rendering exists. Subtitle generation should use the V1 default SubtitleStyle constraints for readable Douyin-style vertical output: short lines, clear sentence segmentation, monotonic timings, and safe-area-aware assumptions recorded in the draft data.

## Acceptance criteria

- [ ] The story command writes `narration.txt` containing the AdaptedNarration for the generated ShortVideoDraft.
- [ ] The story command writes an SRT subtitle file with readable line segmentation and monotonic timings.
- [ ] The DraftManifest records narration and subtitle generated assets.
- [ ] The ReferenceMap links subtitle lines and narration lines back to source references where available.
- [ ] The default SubtitleStyle is recorded in the draft data and is suitable for Douyin-style vertical readability.
- [ ] PipelineStepRun records include subtitle generation status, parameters, and outputs.
- [ ] Tests verify narration file output, valid SRT structure, long-line handling, monotonic timing, subtitle reference mapping, and manifest asset records.

## Blocked by

- https://github.com/XmchxUp/cutline/issues/4
