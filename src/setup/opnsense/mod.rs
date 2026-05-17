use super::{SetupStep, StepStatus, can_prompt};

const SSH_DIR: &str = "/root/.ssh";
const AUTHORIZED_KEYS: &str = "/root/.ssh/authorized_keys2";

pub(super) mod key;
pub(super) mod packages;

pub(super) use key::CheckmkKeyStep;
pub(super) use packages::PackagesStep;
