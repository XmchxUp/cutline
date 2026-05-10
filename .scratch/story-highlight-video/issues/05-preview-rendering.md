## Parent

https://github.com/XmchxUp/cutline/issues/1

## What to build

Render a lightweight 9:16 preview video for a StoryHighlightVideo ReviewableDraftPackage using the configured LocalBackgroundAsset and generated subtitles. This slice completes the visible demo path for the first vertical slice: a user can run the story command, open the package, and watch a vertical preview with readable subtitles.

Preview rendering should construct inspectable ffmpeg commands separately from execution, then execute them when rendering is requested. The renderer should adapt a local image or video background to the active PlatformProfile by scaling, cropping, padding, or looping as needed, and write the preview as a generated asset in the package.

## Acceptance criteria

- [ ] The story command can render `preview.mp4` for a generated ReviewableDraftPackage.
- [ ] The preview uses the configured LocalBackgroundAsset and generated subtitle file.
- [ ] The preview targets the active PlatformProfile, with DouyinProfile defaulting to 9:16 vertical output.
- [ ] Video backgrounds are scaled/cropped/padded/looped as needed for the preview duration.
- [ ] Static image backgrounds can be rendered as a video preview.
- [ ] The DraftManifest records the preview as a GeneratedAsset.
- [ ] PipelineStepRun records include preview render status, ffmpeg command information, and output path.
- [ ] Preview command construction can be tested without invoking ffmpeg.
- [ ] Tests verify ffmpeg argument construction for 9:16 output, subtitle inclusion, background input handling, preview asset manifest records, and existing manual render command behavior.

## Blocked by

- https://github.com/XmchxUp/cutline/issues/5
