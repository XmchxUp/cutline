# Cutline Context

## Overview

Cutline is a Rust video cutting tool that uses declarative TOML project files to create linear video edits using ffmpeg.

## Domain Terms

### Core Concepts

- **Project**: A TOML file defining inputs, clips, and render settings
- **Input**: A source video file referenced in a project
- **Clip**: A manually defined segment of an input with specific start/end times
- **Render**: The process of generating output video files using ffmpeg

### Creative Workflows

- **FilmCommentaryEdit**: A short-form commentary workflow that turns a long film or video into one or more narrated vertical clips built around selected source moments
- **StoryHighlightVideo**: A short-form story workflow that turns novel or text content into narrated vertical clips focused on high-engagement moments rather than complete plot coverage

### Planned Capabilities

- **AutoCut**: Automatic source-video segment selection and trimming used by workflows such as **FilmCommentaryEdit**
- **NarrationEdit**: Video editing with voiceover narration added
- **ShortVideo**: Automatic splitting of long videos into multiple short clips
- **StoryToVideo**: Legacy name for text-to-video generation; prefer **StoryHighlightVideo** when the output is a short-form highlight rather than a full adaptation

### Output Formats

- **Single Output**: One continuous video file
- **Multiple Outputs**: Multiple separate video files
- **DraftPackage**: An editable creative draft containing candidate clips, narration script, subtitles, timeline data, and renderable outputs for user review
- **ReviewableDraftPackage**: A machine-readable draft directory that can be generated from the CLI and later opened by a visual review or editing interface
- **DraftManifest**: The main JSON file in a **ReviewableDraftPackage** that lists drafts, timelines, render settings, assets, and source references
- **ReferenceMap**: A machine-readable file that links generated narration, subtitles, and timeline beats back to source timecodes or text ranges
- **SourceAssetReference**: A reference to an original user-provided media file by path and fingerprint rather than a copied full media file
- **GeneratedAsset**: Media created by Cutline during draft generation, such as TTS audio, subtitles, covers, previews, or derived still images
- **CreativePipeline**: The shared draft-generation flow used by creative workflows, moving from source ingestion through analysis, selection, scripting, assembly, preview, review, and final render
- **VerticalSlice**: A minimal end-to-end implementation that proves one workflow path from source input to reviewable draft and preview output
- **IngestStep**: The pipeline step that records source material, metadata, file fingerprints, and source rights
- **AnalyzeStep**: The pipeline step that derives candidate video moments or text highlights from source material
- **SelectStep**: The pipeline step that chooses the content for each short-video draft
- **ScriptStep**: The pipeline step that creates adapted narration from selected source material
- **AssembleStep**: The pipeline step that maps selected content, narration, subtitles, and visuals into an editable timeline
- **PreviewStep**: The pipeline step that renders lightweight outputs for review
- **ReviewStep**: The pipeline step where a user inspects or edits the draft before final export
- **PlatformProfile**: A named set of publishing constraints such as aspect ratio, target duration, subtitle requirements, safe areas, and required engagement structure
- **DouyinProfile**: The default V1 **PlatformProfile** for vertical short videos intended for Douyin-style consumption
- **SafeArea**: The reserved video area that keeps subtitles and key visual elements clear of platform interface overlays
- **SubtitleStyle**: The visual rules for rendered subtitles, including placement, line length, size, contrast, and future typography options
- **CreativeProvider**: A replaceable external or local capability used by the creative pipeline for analysis, scripting, voice, subtitles, or visual assets
- **LocalProvider**: A **CreativeProvider** implemented locally or as a deterministic mock so the creative pipeline can run without a remote AI service
- **ScriptProvider**: A **CreativeProvider** that creates hooks, beats, adapted narration, and endings from selected source material
- **VoiceProvider**: A **CreativeProvider** that turns narration into voiceover audio and optional timing alignment
- **DiaMoETtsProvider**: An optional local **VoiceProvider** based on the DiaMoE-TTS project for Chinese or dialect voiceover generation when the user's environment is configured for it
- **VoiceoverAsset**: Generated narration audio for a short-video draft when a voice provider is available
- **SkippedStep**: A pipeline step that is intentionally not executed for a draft run while still being recorded with a reason
- **VisualAssetProvider**: A **CreativeProvider** that supplies or generates background images, videos, covers, or other visual assets
- **VideoAnalysisProvider**: A **CreativeProvider** that analyzes source video for scene boundaries, audio activity, motion, or other candidate-moment signals
- **PipelineStepRun**: An auditable record of one pipeline step's inputs, outputs, provider, parameters, status, and errors
- **PartialRegeneration**: Rerunning one selected part of a draft, such as segment selection, hook generation, narration, voice, visual matching, preview, or render, without discarding the whole draft
- **VerticalSliceDone**: The completion standard for a vertical slice, requiring a reviewable draft package, a preview render, required draft structure, source references, step-run records, and repeatable local execution
- **SeriesDraft**: A draft package containing multiple related short-video drafts that share one topic, source set, and creative direction
- **ShortVideoDraft**: A single editable short-video draft with its own hook, selected moments, narration, subtitles, and ending
- **CandidateMomentIndex**: A timecoded index of source-video moments that may be useful for editing, based on signals such as scene boundaries, audio activity, motion, and user marks
- **StorySourceRange**: A user-provided chapter, line range, or text excerpt used as the source material for a story video draft
- **TextStorySource**: A local UTF-8 TXT story source used by the V1 story workflow
- **EngagementAngle**: The intended audience hook for a story video, such as revenge, reversal, suspense, romance conflict, or underdog rise
- **NarrationFirstVideo**: A short video whose meaning is carried by voiceover and subtitles while visuals provide mood, pacing, and emphasis rather than a full scene-by-scene reenactment
- **BackgroundAsset**: A reusable image or video source used to support narration without needing to literally depict every plot action
- **LocalBackgroundAsset**: A user-provided local image or video used as the background visual for a draft
- **Hook**: The opening moment of a short-video draft designed to make the viewer continue watching within the first few seconds
- **ContentBeat**: A discrete narrative or commentary point that advances the short-video draft's tension, reveal, or payoff
- **NarrationScript**: The voiceover text for a short-video draft, structured to match selected moments, subtitles, and pacing
- **SourceReference**: A pointer from generated creative material back to the source timecode, chapter, line range, or excerpt it was based on
- **AdaptedNarration**: Originalized short-video narration derived from source material without copying long dialogue or prose verbatim
- **DraftTimeline**: The editable mapping between source moments or background assets, narration, subtitles, and output timing
- **EndingBeat**: The final beat of a short-video draft, usually a payoff, reversal, cliffhanger, or next-episode prompt
- **SourceRights**: The declared usage status of a source asset, such as user-owned, licensed, public-domain, unknown, or draft-only

## Current Status

V1 implements a basic video editing loop:
1. Load TOML project file
2. Validate inputs, clips, timestamps
3. Derive linear edit plan
4. Render clip cache files
5. Concatenate cached clips into final output

## Non-Goals For V1

- Multi-track timelines
- Overlay, transitions, B-roll, or audio mixing

## Feature Requirements

### AutoCut Feature

Automatic video editing that selects and trims clips from source videos.

**Input**: Source video file + target duration + rules

**Rules**:
- `scene_change`: Detect scene boundaries
- `audio_activity`: Detect speech/sound activity
- `motion`: Detect motion intensity

**Output**: 
- Single output: One continuous video
- Multiple outputs: Multiple separate短视频 files

**Use Cases**:
- Create **FilmCommentaryEdit** outputs from long videos
- Split long videos into multiple short clips for Douyin/TikTok

### StoryToVideo Feature

Generating **StoryHighlightVideo** content from text.

**Input**: Novel text file (TXT/EPUB) + TTS voice config

**Process**:
1. Parse novel text into segments
2. Extract engaging content using keywords
3. Generate voiceover using TTS
4. Match background video素材
5. Combine audio + visuals + subtitles

**Output**: Short video (60s-5min) suitable for social media

**Use Cases**:
- Convert popular web novels to short videos
- Create audiobook-style videos with static images
- Generate content for story-telling accounts

## Relationships

- A **FilmCommentaryEdit** uses one source **Input** and produces a **SeriesDraft** when multiple clips are requested.
- A source **Input** used by a **FilmCommentaryEdit** has **SourceRights**; Cutline may generate a local draft from an input without asserting that the result can be publicly published.
- A **FilmCommentaryEdit** may use **AutoCut** to find candidate source moments, but the commentary workflow is not the same thing as **AutoCut**.
- A **FilmCommentaryEdit** creates a **CandidateMomentIndex** before generating narration or final timelines.
- A **SeriesDraft** contains one or more **ShortVideoDraft** items; each **ShortVideoDraft** must be understandable on its own.
- A **ShortVideoDraft** contains one **Hook**, multiple **ContentBeat** items, one **NarrationScript**, one **DraftTimeline**, and one **EndingBeat**.
- A **NarrationScript** is **AdaptedNarration** and keeps **SourceReference** links for user review.
- A **StoryHighlightVideo** is generated from a **StorySourceRange** and does not need to preserve the complete plot.
- The V1 **StoryHighlightVideo** source format is **TextStorySource** with an explicit line or paragraph range; EPUB files, web novel scraping, and remote URLs are outside the first vertical slice.
- A **StoryHighlightVideo** uses an **EngagementAngle** to decide which moments from a **StorySourceRange** are worth adapting.
- A **StoryHighlightVideo** is a **NarrationFirstVideo** in V1 and may use **BackgroundAsset** visuals instead of continuous acted scenes or lip-sync video.
- The V1 **StoryHighlightVideo** visual input is a **LocalBackgroundAsset**; generated images, generated videos, and remote visual search are outside the first vertical slice.
- Cutline may crop, loop, scale, or pad a **LocalBackgroundAsset** to satisfy the active **PlatformProfile**.
- A **FilmCommentaryEdit** and a **StoryHighlightVideo** produce a **ReviewableDraftPackage** first; direct publishing is outside the V1 workflow.
- A **ReviewableDraftPackage** may be generated by CLI/TOML workflows in V1, but its structure must preserve enough data for future visual review and editing.
- A **ReviewableDraftPackage** contains a **DraftManifest**, narration text, subtitle files, optional previews, copied or referenced assets, and a **ReferenceMap**.
- A **ReviewableDraftPackage** references original source media through **SourceAssetReference** and stores **GeneratedAsset** files inside the package.
- A **FilmCommentaryEdit** and a **StoryHighlightVideo** both run through the **CreativePipeline**.
- The **AnalyzeStep** and **AssembleStep** may differ between creative workflows, but the pipeline stages remain shared.
- A **PlatformProfile** constrains **ShortVideoDraft** duration, aspect ratio, subtitles, **SafeArea**, **Hook**, and **EndingBeat** expectations.
- **DouyinProfile** is the default V1 **PlatformProfile**, but workflow logic should depend on **PlatformProfile** rather than hard-coded Douyin rules.
- V1 uses a default **SubtitleStyle** optimized for Douyin-style vertical readability while keeping subtitle style as an explicit draft setting for future editing.
- The **CreativePipeline** uses **CreativeProvider** capabilities; individual workflows should not be tied to a specific LLM, TTS service, visual generator, or video-analysis service.
- A **ScriptProvider**, **VoiceProvider**, **VisualAssetProvider**, or **VideoAnalysisProvider** may be local or remote as long as it satisfies the pipeline contract.
- The first **StoryHighlightVideo** **VerticalSlice** should use **LocalProvider** implementations before relying on cloud LLM, TTS, or visual-generation services.
- **DiaMoETtsProvider** may be used as a concrete **VoiceProvider**, but it is optional and must not be required for the first **StoryHighlightVideo** **VerticalSlice** to complete.
- A **CreativePipeline** execution records **PipelineStepRun** entries so generated drafts can be inspected, debugged, and partially regenerated.
- A **ReviewableDraftPackage** preserves enough **PipelineStepRun** history to support **PartialRegeneration** during review.
- The first **VerticalSlice** targets **StoryHighlightVideo** so text selection, adapted narration, TTS, subtitles, background assets, draft packaging, and preview rendering can be validated before adding film video analysis.
- The first **StoryHighlightVideo** **VerticalSlice** reaches **VerticalSliceDone** only when it can generate a **ReviewableDraftPackage** containing a **DraftManifest**, narration text, subtitle file, **ReferenceMap**, preview video, one **ShortVideoDraft** with one **Hook**, three to five **ContentBeat** items, one **EndingBeat**, and **PipelineStepRun** records without depending on remote services.
- The first preview video should follow **DouyinProfile** defaults with vertical 9:16 output, visible subtitles, a background asset, and target duration within the configured platform range.
- The first **StoryHighlightVideo** preview may omit **VoiceoverAsset** generation, but the voice step must be recorded as a completed or **SkippedStep** **PipelineStepRun**.

## Implementation Constraints

- Maintain compatibility with existing manual editing workflow
- Extend existing `Clip` model with `ClipType` enum
- New CLI commands: `autocut`, `story`
- Reuse existing render/concatenate infrastructure

## Implementation Priority

The first creative implementation should proceed in this order:
1. Define **ReviewableDraftPackage**, **DraftManifest**, and **ShortVideoDraft** data structures
2. Define **StoryHighlightVideo** TOML configuration for text range, **EngagementAngle**, **LocalBackgroundAsset**, and **PlatformProfile**
3. Implement a local **ScriptProvider** that creates a **Hook**, three to five **ContentBeat** items, **AdaptedNarration**, and an **EndingBeat**
4. Write **DraftManifest**, narration text, subtitle file, **ReferenceMap**, and **PipelineStepRun** records
5. Render a 9:16 preview using ffmpeg with visible subtitles and a local background asset
6. Add optional **DiaMoETtsProvider** voiceover generation after the local draft pipeline is working
