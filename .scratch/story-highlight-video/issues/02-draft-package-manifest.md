## Parent

https://github.com/XmchxUp/cutline/issues/1

## What to build

Make the `story` command generate the first ReviewableDraftPackage directory for a valid StoryHighlightVideo project, without requiring preview rendering yet. The generated package should establish the durable draft shape: DraftManifest, ReferenceMap, generated asset area, SourceAssetReference entries, and PipelineStepRun records.

This slice should be demoable by running the story command against a valid local text source and background asset and inspecting the generated machine-readable package. The package may contain placeholder creative content until the Local ScriptProvider slice lands, but it must preserve enough structure for later scripting, subtitles, preview rendering, and visual review.

## Acceptance criteria

- [ ] `cutline story <project>` creates a ReviewableDraftPackage directory for a valid StoryHighlightVideo project.
- [ ] The package contains a DraftManifest JSON file that records workflow type, platform profile, source text reference, background asset reference, output paths, draft ID, and package schema version.
- [ ] The package contains a ReferenceMap JSON file even if later creative references are not populated yet.
- [ ] The package records SourceAssetReference entries by path and fingerprint instead of copying full original source files by default.
- [ ] The package records PipelineStepRun entries for the pipeline steps reached by this slice.
- [ ] `cutline story <project> --json` emits machine-readable summary information including the package path and draft ID.
- [ ] Tests verify generated package contents, manifest shape, reference map presence, source asset fingerprinting, step-run records, and repeatable local execution.

## Blocked by

- https://github.com/XmchxUp/cutline/issues/2
