use crate::planner::Plan;

#[derive(Debug, Clone)]
pub struct FfmpegCommand {
    pub program: String,
    pub args: Vec<String>,
}

pub fn render_commands(_plan: &Plan) -> Vec<FfmpegCommand> {
    Vec::new()
}
