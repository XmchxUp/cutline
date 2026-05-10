use std::fs;
use std::process::Command;

use camino::Utf8PathBuf;

#[test]
fn story_command_json_outputs_draft_summary() {
    let root = std::env::temp_dir().join(format!("cutline-story-cli-json-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("stories")).unwrap();
    fs::create_dir_all(root.join("assets")).unwrap();
    fs::write(root.join("stories/demo.txt"), "第一行\n第二行\n").unwrap();
    fs::write(root.join("assets/bg.mp4"), "not real media").unwrap();
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
    let output = Command::new(env!("CARGO_BIN_EXE_cutline"))
        .args(["story", project_path.as_str(), "--json"])
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
    assert!(root.join(".cutline/drafts/demo/draft.json").is_file());
    assert!(root.join(".cutline/drafts/demo/references.json").is_file());
    assert!(root.join(".cutline/drafts/demo/narration.txt").is_file());
    assert!(root.join(".cutline/drafts/demo/subtitles.srt").is_file());
    assert!(root.join(".cutline/drafts/demo/assets").is_dir());

    let manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join(".cutline/drafts/demo/draft.json")).unwrap(),
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

    let references: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(root.join(".cutline/drafts/demo/references.json")).unwrap(),
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

    let _ = fs::remove_dir_all(&root);
}
