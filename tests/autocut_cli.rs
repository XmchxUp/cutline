use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use camino::Utf8PathBuf;

#[test]
fn autocut_command_json_outputs_plan() {
    let root =
        std::env::temp_dir().join(format!("cutline-autocut-cli-json-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("raw")).unwrap();
    fs::write(root.join("raw/vod.mp4"), "not real media").unwrap();

    let fake_bin = root.join("bin");
    fs::create_dir_all(&fake_bin).unwrap();
    let fake_ffprobe = fake_bin.join("ffprobe");
    fs::write(
        &fake_ffprobe,
        "#!/bin/sh\nprintf '{\"format\":{\"duration\":\"65.0\"}}'\n",
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_ffprobe).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_ffprobe, permissions).unwrap();
    let fake_ffmpeg = fake_bin.join("ffmpeg");
    fs::write(
        &fake_ffmpeg,
        "#!/bin/sh\nprintf '[Parsed_showinfo_1 @ 0x] n: 0 pts: 8000 pts_time:8 pos:0\\n[Parsed_showinfo_1 @ 0x] n: 1 pts: 32000 pts_time:32 pos:0\\n' >&2\n",
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_ffmpeg).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_ffmpeg, permissions).unwrap();

    fs::write(
        root.join("project.toml"),
        r#"
        [output]
        path = "dist/autocut.mp4"

        [input.main]
        path = "raw/vod.mp4"

        [[auto_cut]]
        name = "main_autocut"
        input = "main"
        target_duration = "60s"
        clip_duration = "20s"
        min_clip_duration = "10s"
        rules = ["scene_change"]
        output_mode = "single"
        "#,
    )
    .unwrap();

    let project_path = Utf8PathBuf::from_path_buf(root.join("project.toml")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_cutline"))
        .env(
            "PATH",
            format!(
                "{}:{}",
                fake_bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .args(["autocut", project_path.as_str(), "--json"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["auto_cuts"][0]["name"], "main_autocut");
    assert_eq!(json["auto_cuts"][0]["analysis"]["fallback_used"], false);
    assert_eq!(
        json["auto_cuts"][0]["analysis"]["scene_changes"][0],
        "00:00:08.000"
    );
    assert_eq!(json["auto_cuts"][0]["clips"].as_array().unwrap().len(), 2);
    assert_eq!(json["auto_cuts"][0]["clips"][0]["start"], "00:00:08.000");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn render_autocut_single_outputs_video() {
    let fixture = AutoCutFixture::new("single", "single");

    let output = Command::new(env!("CARGO_BIN_EXE_cutline"))
        .env(
            "PATH",
            format!(
                "{}:{}",
                fixture.fake_bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .args(["render", fixture.project_path.as_str(), "--autocut"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(fixture.root.join("dist/autocut.mp4").is_file());
}

#[test]
fn render_autocut_multiple_outputs_one_file_per_clip() {
    let fixture = AutoCutFixture::new("multiple", "multiple");

    let output = Command::new(env!("CARGO_BIN_EXE_cutline"))
        .env(
            "PATH",
            format!(
                "{}:{}",
                fixture.fake_bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .args(["render", fixture.project_path.as_str(), "--autocut"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        fixture
            .root
            .join("dist/autocut-main_autocut-001.mp4")
            .is_file()
    );
    assert!(
        fixture
            .root
            .join("dist/autocut-main_autocut-002.mp4")
            .is_file()
    );
}

struct AutoCutFixture {
    root: std::path::PathBuf,
    fake_bin: std::path::PathBuf,
    project_path: Utf8PathBuf,
}

impl AutoCutFixture {
    fn new(name: &str, output_mode: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "cutline-autocut-render-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("raw")).unwrap();
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::write(root.join("raw/vod.mp4"), "not real media").unwrap();

        let fake_bin = root.join("bin");
        write_fake_ffprobe(&fake_bin);
        write_fake_ffmpeg(&fake_bin);

        fs::write(
            root.join("project.toml"),
            format!(
                r#"
                [output]
                path = "dist/autocut.mp4"

                [input.main]
                path = "raw/vod.mp4"

                [[auto_cut]]
                name = "main_autocut"
                input = "main"
                target_duration = "60s"
                clip_duration = "20s"
                min_clip_duration = "10s"
                rules = ["scene_change"]
                output_mode = "{output_mode}"
                "#
            ),
        )
        .unwrap();

        let project_path = Utf8PathBuf::from_path_buf(root.join("project.toml")).unwrap();

        Self {
            root,
            fake_bin,
            project_path,
        }
    }
}

impl Drop for AutoCutFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn write_fake_ffprobe(fake_bin: &std::path::Path) {
    let fake_ffprobe = fake_bin.join("ffprobe");
    fs::write(
        &fake_ffprobe,
        "#!/bin/sh\nprintf '{\"format\":{\"duration\":\"65.0\"}}'\n",
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_ffprobe).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_ffprobe, permissions).unwrap();
}

fn write_fake_ffmpeg(fake_bin: &std::path::Path) {
    let fake_ffmpeg = fake_bin.join("ffmpeg");
    fs::write(
        &fake_ffmpeg,
        "#!/bin/sh\nout=\"\"\nfor arg do\n  out=\"$arg\"\ndone\nif [ \"$out\" = \"-\" ]; then\n  printf '[Parsed_showinfo_1 @ 0x] n: 0 pts: 8000 pts_time:8 pos:0\\n[Parsed_showinfo_1 @ 0x] n: 1 pts: 32000 pts_time:32 pos:0\\n' >&2\nelse\n  mkdir -p \"$(dirname \"$out\")\"\n  printf rendered > \"$out\"\nfi\n",
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_ffmpeg).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_ffmpeg, permissions).unwrap();
}
