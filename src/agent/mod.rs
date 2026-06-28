//! Core agent logic.
//!
//! The agent orchestrates:
//! - Message routing from channels
//! - Job scheduling and execution
//! - Tool invocation with safety
//! - Self-repair for stuck jobs
//! - Proactive heartbeat execution
//! - Routine-based scheduled and reactive jobs
//! - Turn-based session management with undo
//! - Context compaction for long conversations

pub mod agentic_loop;
pub mod background_tasks;
pub mod compaction;
pub mod context_monitor;
pub mod cost_guard;
pub mod dead_letter_queue;
pub mod gate_controller;
mod heartbeat;
pub mod job_monitor;
mod router;
pub mod routine;
pub mod routine_engine;
pub(crate) mod scheduler;
mod self_repair;
pub mod session;
mod session_manager;
pub mod submission;
pub mod task;
pub mod turn_builder;
pub mod undo;

pub use compaction::{CompactionResult, ContextCompactor};
pub use context_monitor::{CompactionStrategy, ContextBreakdown, ContextMonitor};
// pub(crate) use dispatcher::strip_suggestions; // V1 - dispatcher disabled
pub use gate_controller::{AutoApprovingGateController, GateMode};
pub use heartbeat::{
    HeartbeatConfig, HeartbeatResult, HeartbeatRunner, spawn_heartbeat, spawn_multi_user_heartbeat,
};
pub use router::{MessageIntent, Router};
pub use routine::{Routine, RoutineAction, RoutineRun, Trigger};
pub use routine_engine::{RoutineEngine, SandboxReadiness};
pub use scheduler::{Scheduler, SchedulerDeps};
pub use self_repair::{BrokenTool, RepairResult, RepairTask, SelfRepair, StuckJob};
pub use session::{
    PendingApproval, PendingAuth, Session, Thread, ThreadState, Turn, TurnOutcome, TurnState,
};
pub use session_manager::SessionManager;
pub use submission::{Submission, SubmissionParser, SubmissionResult};
pub use task::{Task, TaskContext, TaskHandler, TaskOutput};
pub use undo::{Checkpoint, UndoManager};
