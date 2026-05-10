# Draft packages reference source media

Cutline draft packages will reference original user-provided media by path and fingerprint instead of copying full source videos into the package by default. This avoids duplicating large or sensitive files while still allowing the draft manifest to validate whether the original media is available and unchanged; generated assets such as TTS audio, subtitles, covers, and previews remain inside the package.
