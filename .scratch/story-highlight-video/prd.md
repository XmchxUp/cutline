---
title: StoryHighlightVideo V1 reviewable draft vertical slice
labels:
  - ready-for-agent
  - prd
status: ready-for-agent
created: 2026-05-11
---

## Problem Statement

Cutline can currently render manually defined linear video clips, and the planned StoryToVideo/AutoCut concepts exist only as early configuration and placeholder logic. The user wants Cutline to support short-form creative generation for Douyin-style accounts, starting with popular novel content that can be turned into engaging narrated videos without requiring a complete plot adaptation.

The immediate problem is that there is no end-to-end StoryHighlightVideo workflow that can take a local text range, identify a high-engagement angle, produce adapted narration and subtitles, assemble a vertical preview with local background media, and save the result as a reviewable creative draft. Without a ReviewableDraftPackage, the user cannot inspect, edit, regenerate, or later connect real AI/TTS providers safely.

## Solution

Build the first VerticalSlice for StoryHighlightVideo. The workflow will take a local UTF-8 TextStorySource, an explicit StorySourceRange, an EngagementAngle, a LocalBackgroundAsset, and a PlatformProfile. It will generate a ReviewableDraftPackage containing a DraftManifest, one ShortVideoDraft, narration text, subtitle file, ReferenceMap, PipelineStepRun records, and a 9:16 preview video.

The first implementation will use LocalProvider behavior rather than requiring remote AI services. A local ScriptProvider will produce a Hook, three to five ContentBeat items, AdaptedNarration, and an EndingBeat from the selected text range. The VoiceProvider step may be completed when configured or recorded as a SkippedStep. DiaMoETtsProvider will be designed as an optional local VoiceProvider but will not be required for the first slice to complete.

## User Stories

1. As a short-video creator, I want to provide a local TXT story file, so that Cutline can generate a video draft from text I already control.
2. As a short-video creator, I want to specify a chapter, line range, or paragraph range, so that the first version does not need to analyze an entire novel.
3. As a short-video creator, I want to specify an EngagementAngle, so that the generated draft focuses on a clear audience hook.
4. As a short-video creator, I want Cutline to generate a Hook, so that each ShortVideoDraft starts with a strong first few seconds.
5. As a short-video creator, I want Cutline to generate three to five ContentBeat items, so that the draft has a clear rhythm instead of one long summary.
6. As a short-video creator, I want Cutline to generate an EndingBeat, so that the draft can end with payoff, reversal, cliffhanger, or next-episode intent.
7. As a short-video creator, I want Cutline to generate AdaptedNarration, so that the output is a short-video narration script rather than copied novel prose.
8. As a short-video creator, I want generated narration to preserve SourceReference links, so that I can review where each creative line came from.
9. As a short-video creator, I want Cutline to generate subtitles from the narration, so that the preview is usable even when no voiceover is available.
10. As a short-video creator, I want subtitles to be split into readable lines, so that text does not overwhelm a vertical video screen.
11. As a short-video creator, I want a default SubtitleStyle, so that previews are legible without manual styling work.
12. As a short-video creator, I want subtitles to respect SafeArea constraints, so that platform UI does not cover important text.
13. As a short-video creator, I want to provide a LocalBackgroundAsset, so that the first version can create previews without image or video generation.
14. As a short-video creator, I want Cutline to crop, pad, scale, or loop a background asset, so that it fits the DouyinProfile output.
15. As a short-video creator, I want the preview to be 9:16 by default, so that it is immediately suitable for Douyin-style review.
16. As a short-video creator, I want the preview duration to follow the active PlatformProfile, so that drafts stay within the intended short-video format.
17. As a short-video creator, I want the voice step to be optional, so that I can still generate and review drafts before TTS is configured.
18. As a short-video creator, I want voice generation to be recorded as completed or skipped, so that the draft state is honest and auditable.
19. As a short-video creator, I want DiaMoE-TTS to be available as an optional local VoiceProvider, so that I can use a local Chinese/dialect TTS provider when my environment supports it.
20. As a short-video creator, I want the first workflow to run without cloud services, so that API keys, network access, cost, and provider limits do not block the core draft pipeline.
21. As a short-video creator, I want a ReviewableDraftPackage directory, so that generated files are easy to inspect and edit.
22. As a short-video creator, I want a DraftManifest, so that one machine-readable file describes the full draft and its assets.
23. As a short-video creator, I want a narration text file, so that I can read and edit the script directly.
24. As a short-video creator, I want an SRT subtitle file, so that subtitles can be inspected and reused outside Cutline.
25. As a short-video creator, I want a ReferenceMap, so that generated narration, subtitles, and beats can be traced back to source text ranges.
26. As a short-video creator, I want a preview video, so that I can evaluate pacing, readability, and visual fit before final export.
27. As a short-video creator, I want PipelineStepRun records, so that I can see which provider created each part of the draft and whether any step failed.
28. As a short-video creator, I want PartialRegeneration to be possible later, so that I can regenerate only the hook, narration, subtitles, voiceover, or preview.
29. As a short-video creator, I want source media to be referenced by path and fingerprint, so that draft packages do not copy large or sensitive original media by default.
30. As a short-video creator, I want generated assets stored in the draft package, so that TTS audio, subtitles, covers, derived stills, and previews travel with the draft.
31. As a short-video creator, I want clear validation errors for missing text files, invalid line ranges, or missing background assets, so that I can fix project configuration quickly.
32. As a short-video creator, I want unknown TOML fields rejected, so that configuration mistakes do not silently change generation behavior.
33. As a short-video creator, I want repeatable local generation, so that running the same inputs produces a comparable draft package.
34. As a short-video creator, I want the story command to produce JSON when requested, so that future tools can integrate with the generated draft metadata.
35. As a short-video creator, I want generated draft IDs to be stable or clearly recorded, so that I can find and compare draft outputs.
36. As a short-video creator, I want the system to use the active PlatformProfile rather than hard-coded Douyin rules, so that other platforms can be added later.
37. As a short-video creator, I want StoryHighlightVideo to be distinct from FilmCommentaryEdit, so that text-based and source-video-based workflows do not blur together.
38. As a short-video creator, I want the first StoryHighlightVideo workflow to avoid web scraping and EPUB parsing, so that the MVP focuses on creation rather than acquisition.
39. As a short-video creator, I want the first visual workflow to avoid generated videos and generated images, so that the preview pipeline is predictable and cheap to run.
40. As a maintainer, I want the creative workflow expressed as a CreativePipeline, so that StoryHighlightVideo and future FilmCommentaryEdit share common pipeline concepts.
41. As a maintainer, I want provider traits or interfaces for script, voice, visual assets, and video analysis, so that concrete services can be replaced without changing workflow logic.
42. As a maintainer, I want the local ScriptProvider isolated as a deep module, so that draft quality and deterministic behavior can be tested without ffmpeg.
43. As a maintainer, I want draft package writing isolated as a deep module, so that manifest, references, subtitles, and generated assets are produced consistently.
44. As a maintainer, I want subtitle generation isolated as a deep module, so that line splitting and timing behavior can be tested without rendering video.
45. As a maintainer, I want preview render command construction isolated from command execution, so that ffmpeg arguments can be tested without invoking ffmpeg.
46. As a maintainer, I want validation separated from generation, so that invalid projects fail before partial draft files are written.
47. As a maintainer, I want source asset fingerprints recorded, so that stale or moved assets can be detected later.
48. As a maintainer, I want the voice step represented even when skipped, so that optional TTS does not create missing or ambiguous pipeline state.
49. As a maintainer, I want the current manual clip rendering workflow preserved, so that existing Cutline projects keep working.
50. As a future UI implementer, I want ReviewableDraftPackage data to be machine-readable, so that a visual review/editor interface can open drafts generated by the CLI.

## Implementation Decisions

- The first implementation target is StoryHighlightVideo, not FilmCommentaryEdit. FilmCommentaryEdit will reuse the CreativePipeline later after the story vertical slice proves draft packaging, scripting, subtitles, provider boundaries, and preview rendering.
- V1 produces a ReviewableDraftPackage first. Direct publishing is outside scope.
- The ReviewableDraftPackage is a directory, not a single MP4 or single JSON file. It contains a DraftManifest, narration text, subtitle files, optional previews, generated assets, and a ReferenceMap.
- Draft packages reference original source media by path and fingerprint. Full source videos or large original assets are not copied by default.
- Generated assets, such as subtitles, preview renders, optional TTS audio, covers, or derived still images, are stored inside the draft package.
- The shared CreativePipeline stages are IngestStep, AnalyzeStep, SelectStep, ScriptStep, AssembleStep, PreviewStep, ReviewStep, and Render. The first slice must record PipelineStepRun data for the steps it performs or intentionally skips.
- The first StoryHighlightVideo source format is a local UTF-8 TextStorySource with explicit line or paragraph range. EPUB, remote URLs, web novel scraping, and whole-novel analysis are out of scope.
- The first visual input is a LocalBackgroundAsset. Generated images, generated videos, remote media search, and visual style generation are out of scope.
- The default PlatformProfile is DouyinProfile: vertical 9:16 output, visible subtitles, safe-area-aware layout, required hook, required ending beat, and short-form duration constraints. Workflow logic should depend on PlatformProfile rather than hard-coded Douyin assumptions.
- SubtitleStyle is explicit in the draft model, but V1 only needs a default readability-first style.
- VoiceProvider is optional for the first vertical slice. When no voice provider is configured, the voice step is recorded as a SkippedStep and the preview can be rendered without real voiceover.
- DiaMoETtsProvider is an optional local VoiceProvider to support DiaMoE-TTS when the user's environment is configured for it. It must not be a required dependency for VerticalSliceDone.
- CreativeProvider interfaces should be replaceable. The first slice should include LocalProvider implementations for script generation and visual asset selection rather than requiring cloud LLM, TTS, or visual-generation services.
- The local ScriptProvider should be a deep module: it accepts source text range and EngagementAngle and returns a Hook, three to five ContentBeat items, AdaptedNarration, EndingBeat, and SourceReference links.
- Subtitle generation should be a deep module: it accepts the NarrationScript or draft beats and returns timed subtitle lines plus SRT output using the default SubtitleStyle constraints.
- Draft package writing should be a deep module: it accepts the generated ShortVideoDraft, source references, provider step records, assets, and render settings and writes a consistent ReviewableDraftPackage.
- Preview rendering should split command construction from execution. The system should be able to inspect ffmpeg command arguments before running ffmpeg.
- Project configuration needs to align with the confirmed domain model: StoryHighlightVideo config should include text source path, source range, EngagementAngle, LocalBackgroundAsset, PlatformProfile, optional VoiceProvider, and output draft location.
- Current manual clip rendering behavior remains compatible. Existing check, plan, render, clean commands should continue to work for manual projects.
- Existing placeholder AutoCut and StoryToVideo concepts should be normalized toward the domain language: StoryToVideo is legacy naming for StoryHighlightVideo when the output is a short-form highlight rather than a full adaptation.

## Testing Decisions

- Tests should verify external behavior: parsed configuration, validation results, generated draft package contents, generated subtitle content, provider step records, and preview command arguments. They should not lock tests to private helper functions or incidental formatting beyond public output contracts.
- Configuration tests should cover valid StoryHighlightVideo TOML, unknown-field rejection, missing text source, missing background asset, invalid line ranges, unsupported source format, and optional voice provider behavior.
- Text range tests should verify that a TextStorySource reads the requested range and reports useful errors for empty or out-of-bounds ranges.
- Local ScriptProvider tests should verify that a source range and EngagementAngle produce one Hook, three to five ContentBeat items, one AdaptedNarration script, one EndingBeat, and SourceReference links.
- Subtitle generation tests should verify readable segmentation, monotonic timings, non-empty SRT output, and safe handling of long narration lines.
- Draft package tests should verify that the package contains the DraftManifest, narration text, subtitle file, ReferenceMap, generated assets, and PipelineStepRun records.
- Source reference tests should verify that generated hooks, beats, narration, and subtitle lines can be traced back to source line or paragraph ranges.
- Pipeline step tests should verify that completed, failed, and skipped steps are represented explicitly and include provider and parameter metadata.
- Preview command tests should verify that ffmpeg arguments target 9:16 output, include the background asset, include subtitles, and write to the expected preview output.
- Optional voice tests should verify both paths: voice step skipped without provider and voice step completed when a local voice provider produces a VoiceoverAsset.
- Regression tests should preserve existing manual editing behavior: project validation, plan building, clip cache planning, and render command construction.
- Prior art in the codebase includes existing tests around config parsing, unknown field rejection, validation, plan building, cache key behavior, and ffmpeg argument construction. New tests should follow that pattern: small, deterministic Rust tests that do not require real media unless explicitly marked as integration tests.

## Out of Scope

- FilmCommentaryEdit implementation.
- AutoCut video analysis for scene boundaries, audio activity, motion, or candidate film moments.
- Whole-novel analysis.
- EPUB parsing.
- Web novel scraping, remote URL ingestion, login flows, or anti-scraping handling.
- Generated images or generated videos.
- Remote visual search.
- Full visual review/editor UI.
- Direct publishing to Douyin or any other platform.
- Platform account management.
- Public-release rights verification or legal determinations.
- Full self-contained media export that copies all original source files into a draft package.
- Cloud LLM integration as a required dependency.
- Cloud TTS integration as a required dependency.
- DiaMoE-TTS installation, model download, or environment provisioning as part of the first vertical slice.
- Lip-sync video, acted scene generation, or full plot reenactment.
- Complex subtitle editor behavior beyond the default SubtitleStyle.
- Multi-track timeline editing beyond what is needed for the preview render.

## Further Notes

- The PRD follows the domain language established in the Cutline context document and the accepted ADRs for reviewable draft packages, source media references, shared creative pipeline, platform profiles, replaceable providers, and step-run records.
- The current codebase already has early StoryToVideo and AutoCut structs and CLI placeholders. The implementation should treat those as scaffolding, not as a completed workflow.
- The first VerticalSlice reaches VerticalSliceDone only when it can generate a ReviewableDraftPackage with a preview video, required draft structure, source references, step records, and repeatable local execution without remote services.
- Once the local vertical slice is stable, DiaMoETtsProvider can be added as an optional VoiceProvider and FilmCommentaryEdit can start reusing the same draft package and pipeline model.
