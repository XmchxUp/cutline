## Parent

https://github.com/XmchxUp/cutline/issues/1

## What to build

Add the first StoryHighlightVideo configuration path so a project can describe a local UTF-8 TextStorySource, an explicit StorySourceRange, an EngagementAngle, a LocalBackgroundAsset, and a PlatformProfile. The slice should make `cutline check` validate the story source and background asset before any draft generation runs, while preserving existing manual clip project behavior.

This issue should align the existing StoryToVideo placeholder configuration with the confirmed StoryHighlightVideo domain language. It should reject unsupported V1 inputs such as EPUB files, remote URLs, generated visual sources, missing line ranges, invalid ranges, and missing local background assets.

## Acceptance criteria

- [ ] A project can configure one StoryHighlightVideo using a local UTF-8 TXT source, start/end line or paragraph range, EngagementAngle, LocalBackgroundAsset, and PlatformProfile.
- [ ] `cutline check` accepts a valid StoryHighlightVideo configuration without requiring manual `[[clip]]` entries for this creative workflow.
- [ ] `cutline check` returns clear validation errors for missing story text, non-TXT/unsupported source formats, empty or reversed ranges, out-of-bounds ranges, missing background assets, and unsupported platform profiles.
- [ ] Unknown TOML fields remain rejected.
- [ ] Existing manual clip projects still validate and plan as before.
- [ ] Tests cover valid config, unknown field rejection, missing text source, missing background asset, invalid line range, and compatibility with the existing manual workflow.

## Blocked by

None - can start immediately
