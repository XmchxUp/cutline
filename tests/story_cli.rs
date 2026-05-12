use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use camino::Utf8PathBuf;

#[test]
fn story_command_json_outputs_draft_summary() {
    let fixture = StoryFixture::new("json");

    let output = Command::new(env!("CARGO_BIN_EXE_cutline"))
        .env(
            "PATH",
            format!(
                "{}:{}",
                fixture.fake_bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .args([
            "story",
            fixture.project_path.as_str(),
            "--json",
            "--render-preview",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["draft_id"], "demo");
    assert!(
        json["package_path"]
            .as_str()
            .unwrap()
            .ends_with(".cutline/drafts/demo")
    );
    assert!(
        fixture
            .root
            .join(".cutline/drafts/demo/draft.json")
            .is_file()
    );
    assert!(
        fixture
            .root
            .join(".cutline/drafts/demo/references.json")
            .is_file()
    );
    assert!(
        fixture
            .root
            .join(".cutline/drafts/demo/narration.txt")
            .is_file()
    );
    assert!(
        fixture
            .root
            .join(".cutline/drafts/demo/subtitles.srt")
            .is_file()
    );
    assert!(
        fixture
            .root
            .join(".cutline/drafts/demo/preview.mp4")
            .is_file()
    );
    assert!(fixture.root.join(".cutline/drafts/demo/assets").is_dir());

    let manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(fixture.root.join(".cutline/drafts/demo/draft.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["short_video_drafts"][0]["id"], "demo-001");
    assert_eq!(
        manifest["short_video_drafts"][0]["hook"]["source_reference"]["source"],
        "stories/demo.txt"
    );
    assert_eq!(
        manifest["generated_assets"]["narration"]["path"],
        "narration.txt"
    );
    assert_eq!(
        manifest["generated_assets"]["subtitles"]["path"],
        "subtitles.srt"
    );
    assert_eq!(manifest["subtitle_style"]["platform"], "douyin");
    assert_eq!(
        manifest["generated_assets"]["preview"]["path"],
        "preview.mp4"
    );

    let references: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(fixture.root.join(".cutline/drafts/demo/references.json")).unwrap(),
    )
    .unwrap();
    assert!(
        references["pipeline_step_runs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step["step"] == "script" && step["provider"] == "local_script_provider")
    );
    assert!(
        references["pipeline_step_runs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step["step"] == "subtitle"
                && step["provider"] == "local_subtitle_provider")
    );
    assert!(
        references["pipeline_step_runs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step["step"] == "preview" && step["provider"] == "ffmpeg")
    );
    assert!(
        references["pipeline_step_runs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|step| step["step"] == "voice"
                && step["status"] == "skipped"
                && step["provider"] == "none")
    );
}

#[test]
fn render_story_outputs_configured_video_path() {
    let fixture = StoryFixture::new("render");

    let output = Command::new(env!("CARGO_BIN_EXE_cutline"))
        .env(
            "PATH",
            format!(
                "{}:{}",
                fixture.fake_bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .args(["render", fixture.project_path.as_str(), "--story"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(fixture.root.join("dist/story.mp4").is_file());
    assert!(
        fixture
            .root
            .join(".cutline/drafts/demo/preview.mp4")
            .is_file()
    );
}

#[test]
fn render_story_refuses_to_overwrite_without_force() {
    let fixture = StoryFixture::new("overwrite");
    fs::create_dir_all(fixture.root.join("dist")).unwrap();
    fs::write(fixture.root.join("dist/story.mp4"), "existing").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_cutline"))
        .env(
            "PATH",
            format!(
                "{}:{}",
                fixture.fake_bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .args(["render", fixture.project_path.as_str(), "--story"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("output file already exists"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

struct StoryFixture {
    root: std::path::PathBuf,
    fake_bin: std::path::PathBuf,
    project_path: Utf8PathBuf,
}

impl StoryFixture {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("cutline-story-cli-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("stories")).unwrap();
        fs::create_dir_all(root.join("assets")).unwrap();
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::write(root.join("stories/demo.txt"), "第一行\n第二行\n").unwrap();
        fs::write(root.join("assets/bg.mp4"), "not real media").unwrap();
        let fake_bin = root.join("bin");
        write_fake_ffmpeg(&fake_bin);
        fs::write(
            root.join("project.toml"),
            r#"
            [output]
            path = "dist/story.mp4"

            [[story]]
            name = "demo"
            source = "stories/demo.txt"
            start_line = 1
            end_line = 2
            engagement_angle = "reversal"
            background = "assets/bg.mp4"
            platform = "douyin"
            "#,
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

impl Drop for StoryFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn write_fake_ffmpeg(fake_bin: &std::path::Path) {
    let fake_ffmpeg = fake_bin.join("ffmpeg");
    fs::write(
        &fake_ffmpeg,
        "#!/bin/sh\nout=\"\"\nfor arg do\n  out=\"$arg\"\ndone\nmkdir -p \"$(dirname \"$out\")\"\nprintf preview > \"$out\"\n",
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_ffmpeg).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_ffmpeg, permissions).unwrap();
}
