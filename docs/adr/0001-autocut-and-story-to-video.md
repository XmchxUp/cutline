# ADR 0001: AutoCut and Story-to-Video Features

## Status

Accepted - Implementation Ready

## Context

The project needs to add two new features:
1. **AutoCut**: Automatic video editing that selects and trims clips from source videos
2. **StoryToVideo**: Generating video content from text (novel-to-video)

## Decision

### Architecture

We will extend the existing `Clip` model with a `ClipType` enum to support different editing modes:

```rust
enum ClipType {
    Manual,                    // Current behavior
    Auto(AutoClipConfig),      // AutoCut feature
    Narration(NarrationConfig), // Narration feature
}
```

### AutoCut Feature

AutoCut will automatically select and trim clips based on rules:

**Configuration in project.toml**:
```toml
[[auto_cut]]
input = "main"
duration = "5min"           # Total target duration
clip_duration = "30s"       # Individual clip duration
rules = ["scene_change", "audio_activity"]
output_mode = "multiple"    # "single" or "multiple"
```

**Rules**:
- `scene_change`: Detect scene boundaries using ffmpeg's scene detection
- `audio_activity`: Detect speech/sound activity to avoid silent segments
- `motion`: Detect motion intensity

### StoryToVideo Feature

StoryToVideo generates videos from text files:

**Configuration**:
```toml
[[story_video]]
novel_file = "stories/chapter1.txt"
output_duration = "60s"
voice = "zh-CN-XiaoxiaoNeural"  # TTS voice
background_music = "assets/bg.mp3"
aspect_ratio = "9:16"           # Vertical for Douyin
```

**Implementation Steps**:
1. Parse novel text into segments
2. Extract "精彩" (engaging) content using keywords
3. Generate voiceover using TTS
4. Match background video素材
5. Combine audio + visuals + subtitles

### CLI Commands

New commands:
- `cutline autocut project.toml` - Generate AutoCut plan
- `cutline story project.toml` - Generate StoryToVideo plan
- `cutline render --autocut` - Execute AutoCut rendering
- `cutline render --story` - Execute StoryToVideo rendering

## Consequences

### Positive

- Extends Cutline to support AI-assisted video creation
- Maintains compatibility with existing manual editing workflow
- Modular design allows incremental feature development

### Negative

- Increased complexity in the codebase
- New dependencies on TTS services and video analysis tools
- More complex validation logic

## Implementation Plan

1. Extend `model.rs` with new clip types
2. Update `config.rs` for new configuration options
3. Add `autocut.rs` module for automatic clip selection
4. Add `story.rs` module for novel-to-video generation
5. Update `cli.rs` with new commands
6. Add validation logic in `validate.rs`

## Detailed Configuration Examples

### AutoCut Project.toml Example

```toml
[output]
path = "dist/output.mp4"
format = "mp4"
aspect_ratio = "9:16"  # Vertical for Douyin

[input.main]
path = "raw/vod.mp4"

# AutoCut configuration
[[auto_cut]]
input = "main"
target_duration = "300s"      # Target total duration (5 minutes)
clip_duration = "30s"         # Target individual clip duration
min_clip_duration = "15s"     # Minimum clip length
rules = ["scene_change", "audio_activity"]
output_mode = "multiple"      # "single" or "multiple"
```

### StoryToVideo Project.toml Example

```toml
[output]
path = "dist/story_video.mp4"
format = "mp4"
aspect_ratio = "9:16"

[input.background]
path = "assets/default_background.mp4"

[story]
novel_file = "stories/chapter1.txt"
voice = "zh-CN-XiaoxiaoNeural"    # TTS voice
voice_speed = "1.0"               # 0.5-2.0
background_music = "assets/bg.mp3"
bg_music_volume = "0.3"           # 0.0-1.0
aspect_ratio = "9:16"             # Vertical video

# Text segments to include (can be auto-detected)
[[story.segments]]
start_line = 10
end_line = 50
```

## TTS Integration

Supported TTS services (configurable):
- **Azure Cognitive Services**: `provider = "azure"`
- **Alibaba Cloud TTS**: `provider = "aliyun"`
- **Google Cloud TTS**: `provider = "google"`
- **Local TTS**: `provider = "local"` (for offline use)

## Video Analysis Tools

- **Scene Detection**: ffmpeg's `scene_detect` filter
- **Audio Activity**: ffmpeg's `astats` filter
- **Motion Detection**: ffmpeg's `motiondetect` filter

## Data Model Extensions

### New ClipType Enum

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClipType {
    Manual(ManualClip),
    Auto(AutoClip),
    Story(StoryClip),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualClip {
    pub input: String,
    pub start: TimeValue,
    pub end: TimeValue,
    pub chapter: Option<String>,
    pub blur: bool,
    pub mute: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoClip {
    pub input: String,
    pub rules: Vec<AutoCutRule>,
    pub target_duration: TimeValue,
    pub clip_duration: TimeValue,
    pub min_clip_duration: TimeValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryClip {
    pub novel_file: Utf8PathBuf,
    pub voice: String,
    pub voice_speed: f32,
    pub background_music: Option<Utf8PathBuf>,
    pub bg_music_volume: f32,
}
```

### AutoCutRule Enum

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AutoCutRule {
    SceneChange { threshold: f64 },  // Default: 0.3
    AudioActivity { threshold: f64 }, // Default: 0.01
    Motion { threshold: f64 },        // Default: 0.1
}
```

## Implementation Priority

### Phase 1: Core Infrastructure
1. Extend `model.rs` with new clip types
2. Update `config.rs` for new configuration options
3. Add validation logic in `validate.rs`

### Phase 2: AutoCut Feature
1. Add `autocut.rs` module for automatic clip selection
2. Implement scene detection using ffmpeg
3. Implement audio activity detection
4. Add CLI command `cutline autocut`

### Phase 3: StoryToVideo Feature
1. Add `story.rs` module for novel-to-video generation
2. Implement text parsing and segment extraction
3. Integrate TTS service
4. Add CLI command `cutline story`

### Phase 4: Advanced Features
1. Multiple output support
2. Background music mixing
3. Subtitle generation
4. Aspect ratio conversion

## File Structure

```
src/
  ├─ model.rs          # Extended with ClipType enum
  ├─ config.rs         # New configuration structs
  ├─ autocut.rs        # NEW: AutoCut logic
  ├─ story.rs          # NEW: StoryToVideo logic
  ├─ cli.rs            # Updated with new commands
  ├─ validate.rs       # Updated with new validation
  ├─ planner.rs        # Updated for new clip types
  └─ ffmpeg.rs         # Updated for new effects
```

## CLI Command Design

### AutoCut Command

```bash
# Generate AutoCut plan
cutline autocut project.toml

# Execute AutoCut rendering
cutline render project.toml --autocut

# Options
cutline autocut project.toml --json          # Output JSON
cutline autocut project.toml --dry-run       # Preview without rendering
```

### Story Command

```bash
# Generate StoryToVideo plan
cutline story project.toml

# Execute StoryToVideo rendering
cutline render project.toml --story

# Options
cutline story project.toml --json            # Output JSON
cutline story project.toml --voice-list      # List available TTS voices
```

## Error Handling

### AutoCut Errors
- `InputNotFound`: Specified input not found in project
- `InvalidRule`: Unknown or unsupported rule
- `DurationExceeded`: Cannot meet target duration with given rules
- `NoClipsFound`: No suitable clips found after analysis

### StoryToVideo Errors
- `FileNotFound`: Novel file not found
- `TTSError`: TTS service error
- `InvalidVoice`: Invalid voice identifier
- `SegmentError`: Invalid text segment specification

## Testing Strategy

### Unit Tests
- ClipType serialization/deserialization
- AutoCut rule parsing
- Story segment extraction
- TTS voice validation

### Integration Tests
- End-to-end AutoCut workflow
- End-to-end StoryToVideo workflow
- Output file generation and validation

### Manual Testing
- Visual inspection of AutoCut output
- Audio sync verification for StoryToVideo
- Aspect ratio verification

## Open Questions

### 1. StoryToVideo Background Video Source

**Question**: How should background videos be selected for StoryToVideo?

**Options**:
- A) User provides background video file
- B) Auto-select from a built-in library of free stock videos
- C) Generate static image from text keywords
- D) Mix multiple sources (intro/outro from user, middle from library)

**Recommendation**: Option A (user-provided) for V1, with Option B as future enhancement

### 2. AutoCut Output Mode

**Question**: How should multiple outputs be named and organized?

**Options**:
- A) Auto-generate names with timestamp and sequence number
- B) Use chapter names if available
- C) Allow custom naming pattern

**Recommendation**: Option A with Option C as future enhancement

### 3. TTS Service Authentication

**Question**: How should TTS service credentials be stored?

**Options**:
- A) Environment variables
- B) Config file with encrypted credentials
- C) User prompt at runtime
- D) Support multiple providers with different auth methods

**Recommendation**: Option D - support different auth methods per provider

### 4. Text Segment Extraction for StoryToVideo

**Question**: How should "engaging" text segments be identified?

**Options**:
- A) User manually marks segments
- B) Keyword-based scoring (action, emotion,悬念 words)
- C) AI analysis of text quality
- D) Hybrid: AI scores + user approval

**Recommendation**: Option A for V1, with Option B as future enhancement

## Next Steps

1. Implement Phase 1 (Core Infrastructure)
2. Create prototype for AutoCut feature
3. Create prototype for StoryToVideo feature
4. Gather user feedback
5. Iterate on Phase 2-4

## Implementation Checklist

### Phase 1: Core Infrastructure

- [ ] Extend `model.rs` with `ClipType` enum
- [ ] Update `config.rs` for new configuration options
- [ ] Add `AutoCutRule` enum
- [ ] Add `AutoClip` and `StoryClip` structs
- [ ] Update `validate.rs` with new validation logic
- [ ] Update `planner.rs` for new clip types

### Phase 2: AutoCut Feature

- [ ] Create `autocut.rs` module
- [ ] Implement scene detection using ffmpeg
- [ ] Implement audio activity detection
- [ ] Implement motion detection
- [ ] Add `cutline autocut` CLI command
- [ ] Add `--json` output option
- [ ] Add `--dry-run` option

### Phase 3: StoryToVideo Feature

- [ ] Create `story.rs` module
- [ ] Implement text parsing and segment extraction
- [ ] Integrate TTS service (Azure/Alibaba Cloud)
- [ ] Add voice list command
- [ ] Add `cutline story` CLI command
- [ ] Add `--voice-list` option

### Phase 4: Advanced Features

- [ ] Multiple output support for AutoCut
- [ ] Background music mixing
- [ ] Subtitle generation
- [ ] Aspect ratio conversion (9:16, 16:9)
- [ ] Background video source selection
- [ ] Custom naming patterns

## Dependencies

### New Crates
- `reqwest` - HTTP client for TTS API calls
- `serde_json` - JSON serialization
- `tokio` - Async runtime for TTS calls

### Existing Crates (expanded use)
- `ffmpeg-next` - Video analysis filters
- `toml` - Configuration parsing
