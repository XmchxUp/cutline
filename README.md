# Cutline

Cutline is a Rust video cutting tool inspired by
[tsoding/markut](https://github.com/tsoding/markut). It is not compatible with
Markut's stack language. Cutline uses a declarative TOML project file and turns
a linear clip sequence into cached ffmpeg renders plus a final concat output.

## Status

Cutline currently implements the first usable V1 loop:

- TOML project files
- linear single-track clip sequences
- multiple named inputs
- millisecond timestamp precision
- clip cache under the project-local `.cutline/` directory
- `check`, `plan`, `render`, and `clean` CLI commands

`render` requires `ffmpeg`; media probing requires `ffprobe`.

## Example

```toml
[output]
path = "dist/output.mp4"

[input.main]
path = "raw/vod.mp4"

[input.brb]
path = "raw/brb.mp4"

[render]
video_codec = "libx264"
audio_codec = "aac"
video_bitrate = "4000k"
audio_bitrate = "300k"
preset = "veryfast"
crf = 23

[render.extra]
input_args = ["-hwaccel", "auto"]
output_args = ["-movflags", "+faststart"]

[[clip]]
input = "main"
start = "00:10:00"
end = "00:15:00"
chapter = "Opening"

[[clip]]
input = "brb"
start = "0s"
end = "15s"
mute = true
chapter = "Break"

[[clip]]
input = "main"
start = "01:20:00.000"
end = "01:28:30.500"
blur = true
chapter = "Main topic"
```

Relative paths are resolved relative to the project file location, not the
current working directory.

## Planned CLI

```console
$ cutline check project.toml
$ cutline check project.toml --no-probe
$ cutline plan project.toml
$ cutline plan project.toml --no-probe
$ cutline plan project.toml --json
$ cutline render project.toml
$ cutline render project.toml --force
$ cutline clean project.toml
```

`render` refuses to overwrite an existing output unless `--force` is passed.

## Design

See [docs/DESIGN.md](docs/DESIGN.md).
