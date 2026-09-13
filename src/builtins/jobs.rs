// jobs.rs - Background job management
//
// Features:
//   - Spawn background jobs (cmd &)
//   - jobs builtin
//   - Zombie reaping
//   - Job ID recycling

use std::collections::BTreeMap;
use std::process::{Child, ExitStatus};
use std::sync::{Arc, Mutex};

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum JobStatus {
    Running,
    Done(i32),   // exit code
    Killed,      // died by signal
}

pub struct Job {
    pub id:      usize,
    pub pid:     u32,
    pub command: String,
    pub status:  JobStatus,
    pub child:   Option<Child>,
}

impl std::fmt::Debug for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Job")
            .field("id",      &self.id)
            .field("pid",     &self.pid)
            .field("command", &self.command)
            .field("status",  &self.status)
            .finish()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// JobTable - Global background jobs list
// ─────────────────────────────────────────────────────────────────────────────

pub struct JobTable {
    // BTreeMap for ordered IDs; key = job ID (1-based)
    jobs: BTreeMap<usize, Job>,
}

impl JobTable {
    pub fn new() -> Self {
        Self { jobs: BTreeMap::new() }
    }

    /// Add a new job; returns the assigned job ID.
    pub fn add(&mut self, child: Child, command: &str) -> usize {
        let id  = self.next_id();
        let pid = child.id();
        self.jobs.insert(id, Job {
            id,
            pid,
            command: command.to_string(),
            status: JobStatus::Running,
            child: Some(child),
        });
        println!("[{}] {}", id, pid);
        id
    }

    /// Get next job ID - recycles Done/Killed IDs.
    fn next_id(&self) -> usize {
        // Find a hole in the sequence
        for i in 1.. {
            if !self.jobs.contains_key(&i) {
                return i;
            }
        }
        unreachable!()
    }

    /// Reap finished jobs; prints notification "[N]+ Done cmd".
    /// Called right before printing the prompt.
    pub fn reap_done(&mut self) {
        let mut done_ids: Vec<usize> = Vec::new();

        for (&id, job) in self.jobs.iter_mut() {
            if job.status != JobStatus::Running {
                continue;
            }
            if let Some(ref mut child) = job.child {
                // try_wait: non-blocking
                match child.try_wait() {
                    Ok(Some(status)) => {
                        job.status = exit_status_to_job_status(status);
                        done_ids.push(id);
                    }
                    Ok(None) => { /* still running */ }
                    Err(_)   => {
                        job.status = JobStatus::Killed;
                        done_ids.push(id);
                    }
                }
            }
        }

        // Print notification and remove from table
        for id in done_ids {
            if let Some(job) = self.jobs.remove(&id) {
                let status_str = match job.status {
                    JobStatus::Done(0) => "Done".to_string(),
                    JobStatus::Done(c) => format!("Done({})", c),
                    JobStatus::Killed  => "Killed".to_string(),
                    JobStatus::Running => unreachable!(),
                };
                eprintln!("\r\n[{}]+ {} \x1b[2m{}\x1b[0m", id, status_str, job.command);
            }
        }
    }

    /// Show all Running jobs.
    pub fn print_all(&mut self) {
        self.reap_done(); // ensure status is current

        if self.jobs.is_empty() {
            println!("No background jobs.");
            return;
        }
        for (id, job) in &self.jobs {
            let status = match job.status {
                JobStatus::Running  => "\x1b[1;32mRunning\x1b[0m".to_string(),
                JobStatus::Done(c)  => format!("\x1b[2mDone({})\x1b[0m", c),
                JobStatus::Killed   => "\x1b[1;31mKilled\x1b[0m".to_string(),
            };
            println!("[{}] {:>6}  {}  {}", id, job.pid, status, job.command);
        }
    }

    /// Show specific job by ID (%N).
    pub fn print_job(&mut self, spec: &str) {
        let id = parse_job_spec(spec);
        match id {
            None => eprintln!("jobs: invalid job spec: {}", spec),
            Some(n) => {
                if let Some(job) = self.jobs.get(&n) {
                    let status = match job.status {
                        JobStatus::Running  => "\x1b[1;32mRunning\x1b[0m".to_string(),
                        JobStatus::Done(c)  => format!("\x1b[2mDone({})\x1b[0m", c),
                        JobStatus::Killed   => "\x1b[1;31mKilled\x1b[0m".to_string(),
                    };
                    println!("[{}] {:>6}  {}  {}", n, job.pid, status, job.command);
                } else {
                    eprintln!("jobs: no such job: {}", spec);
                }
            }
        }
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper
// ─────────────────────────────────────────────────────────────────────────────

/// Parse job spec: "%1" or "1" -> Some(1)
fn parse_job_spec(spec: &str) -> Option<usize> {
    spec.trim_start_matches('%').parse::<usize>().ok()
}

fn exit_status_to_job_status(status: ExitStatus) -> JobStatus {
    match status.code() {
        Some(c) => JobStatus::Done(c),
        None    => JobStatus::Killed, // signal
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Global job table (shared via Arc<Mutex>)
// ─────────────────────────────────────────────────────────────────────────────

pub type SharedJobTable = Arc<Mutex<JobTable>>;

pub fn new_job_table() -> SharedJobTable {
    Arc::new(Mutex::new(JobTable::new()))
}
