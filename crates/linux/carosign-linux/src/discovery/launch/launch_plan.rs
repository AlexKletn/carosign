/// Startup plan for the build root.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LaunchPlan {
    /// Arguments (zero is the name of the process)
    pub argv: Vec<String>,
    /// Surrounding.
    pub env: Vec<String>,
}

impl LaunchPlan {
    /// The plan is "only a familiar environment."
    ///
    /// ⚠️ Parents' environment is not inherited: the worker should not have other
    /// variables, including: `LD_PRELOAD`.
    pub fn clean(program: &str) -> Self {
        Self {
            argv: vec![program.to_string()],
            env: vec!["PATH=/usr/bin:/bin".to_string()],
        }
    }
}
