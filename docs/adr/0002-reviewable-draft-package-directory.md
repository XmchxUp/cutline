# Reviewable draft packages are directories

Cutline will write generated creative drafts as a directory containing a manifest, narration, subtitles, previews, assets, and source references instead of only producing a final MP4 or a single JSON file. This keeps CLI generation compatible with a future visual review interface, allows large media artifacts to be managed separately from structured draft data, and makes failed or partial generation easier to inspect and resume.
