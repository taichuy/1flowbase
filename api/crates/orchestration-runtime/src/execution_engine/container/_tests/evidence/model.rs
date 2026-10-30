//! Independent specification: standard-library data only. No production runtime,
//! state, scope, binding or transition helper is imported into this oracle.
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Policy {
    Fail,
    Break,
    Continue,
}
#[derive(Clone, Copy, Debug)]
pub struct Spec {
    pub limit: u8,
    pub parallelism: u8,
    pub policy: Policy,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Phase {
    Running,
    Waiting,
    Due,
    Succeeded,
    Failed,
    Cancelled,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stop {
    None,
    Break,
    Global,
    Ancestor,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct State {
    pub started: u8,
    pub rounds: Vec<Phase>,
    pub stop: Stop,
    pub generation: u8,
    pub owner_up: bool,
    pub crashed: bool,
    pub rejected_mask: u8,
    pub closed: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            started: 0,
            rounds: Vec::new(),
            stop: Stop::None,
            generation: 0,
            owner_up: true,
            crashed: false,
            rejected_mask: 0,
            closed: false,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Start {
        index: u8,
        reported_started: u8,
    },
    Wait(u8),
    Expire(u8),
    Complete {
        index: u8,
        generation: u8,
        accepted: bool,
    },
    Fail {
        index: u8,
        global: bool,
    },
    Timeout {
        index: u8,
        ancestor: bool,
    },
    Cancel(u8),
    Crash,
    Recover {
        generation: u8,
        reported_started: u8,
    },
    Finish {
        global_failure: bool,
        propagated: bool,
        outputs: Vec<u8>,
    },
    Yield,
}
#[derive(Clone, Debug)]
pub struct Violation {
    pub offset: usize,
    pub reason: String,
    pub shortest_prefix: Vec<Event>,
}
fn active(phase: Phase) -> bool {
    matches!(phase, Phase::Running | Phase::Waiting | Phase::Due)
}
fn failure_stop(policy: Policy) -> Stop {
    match policy {
        Policy::Fail => Stop::Global,
        Policy::Break => Stop::Break,
        Policy::Continue => Stop::None,
    }
}
pub fn step(spec: Spec, state: &State, event: &Event) -> Result<State, String> {
    let mut next = state.clone();
    let require = |yes: bool, reason: &str| if yes { Ok(()) } else { Err(reason.to_owned()) };
    let live = || {
        require(
            state.owner_up && !state.closed,
            "event without an active owner",
        )
    };
    let phase = |index: u8| {
        state
            .rounds
            .get(index as usize)
            .copied()
            .ok_or_else(|| "unknown round".to_owned())
    };
    match event {
        Event::Start {
            index,
            reported_started,
        } => {
            live()?;
            require(state.stop == Stop::None, "start after a committed stop")?;
            require(state.started < spec.limit, "fixed budget exceeded")?;
            require(
                *index == state.started && *reported_started == state.started + 1,
                "round identity/count mismatch",
            )?;
            require(
                state.rounds.iter().filter(|phase| active(**phase)).count()
                    < spec.parallelism as usize,
                "parallelism exceeded",
            )?;
            next.started += 1;
            next.rounds.push(Phase::Running);
        }
        Event::Wait(index) => {
            live()?;
            require(
                phase(*index)? == Phase::Running,
                "wait is not a running round",
            )?;
            next.rounds[*index as usize] = Phase::Waiting;
        }
        Event::Expire(index) => {
            live()?;
            require(
                matches!(phase(*index)?, Phase::Running | Phase::Waiting),
                "expiry after a terminal",
            )?;
            next.rounds[*index as usize] = Phase::Due;
        }
        Event::Complete {
            index,
            generation,
            accepted,
        } => {
            let old = phase(*index)?;
            if *accepted {
                live()?;
                require(*generation == state.generation, "stale generation accepted")?;
                require(state.stop == Stop::None, "success admitted after stop")?;
                require(
                    matches!(old, Phase::Running | Phase::Waiting),
                    "late or duplicate terminal accepted",
                )?;
                next.rounds[*index as usize] = Phase::Succeeded;
            } else {
                require(
                    *generation != state.generation
                        || state.closed
                        || state.stop != Stop::None
                        || !matches!(old, Phase::Running | Phase::Waiting),
                    "valid current completion was spuriously rejected",
                )?;
                require(
                    state.rejected_mask & (1 << index) == 0,
                    "repeated diagnostic outside finite domain",
                )?;
                next.rejected_mask |= 1 << index;
            }
        }
        Event::Fail { index, global } => {
            live()?;
            require(active(phase(*index)?), "second round terminal")?;
            next.rounds[*index as usize] = Phase::Failed;
            next.stop = if *global {
                Stop::Global
            } else {
                failure_stop(spec.policy)
            };
        }
        Event::Timeout { index, ancestor } => {
            live()?;
            require(phase(*index)? == Phase::Due, "timeout without expiry")?;
            next.rounds[*index as usize] = Phase::Failed;
            next.stop = if *ancestor {
                Stop::Ancestor
            } else {
                failure_stop(spec.policy)
            };
        }
        Event::Cancel(index) => {
            live()?;
            require(
                state.stop != Stop::None && active(phase(*index)?),
                "cancellation without stop/active round",
            )?;
            next.rounds[*index as usize] = Phase::Cancelled;
        }
        Event::Crash => {
            live()?;
            require(!state.crashed, "more than one crash in this finite domain")?;
            next.crashed = true;
            next.owner_up = false;
        }
        Event::Recover {
            generation,
            reported_started,
        } => {
            require(!state.owner_up && state.crashed, "recovery without crash")?;
            require(
                *generation == state.generation + 1,
                "owner generation not advanced",
            )?;
            require(
                *reported_started == state.started,
                "recovery reset the consumed budget",
            )?;
            next.generation = *generation;
            next.owner_up = true;
        }
        Event::Finish {
            global_failure,
            propagated,
            outputs,
        } => {
            live()?;
            require(
                !state.rounds.iter().any(|phase| active(*phase)),
                "unfinished round at closure",
            )?;
            require(
                state.started == spec.limit || state.stop != Stop::None,
                "unexplained early closure",
            )?;
            require(
                *global_failure == (state.stop == Stop::Global),
                "global failure was swallowed or invented",
            )?;
            require(
                *propagated == (state.stop == Stop::Ancestor),
                "ancestor timeout ownership was lost",
            )?;
            let expected: Vec<_> = state
                .rounds
                .iter()
                .enumerate()
                .filter(|(_, phase)| **phase == Phase::Succeeded)
                .map(|(index, _)| index as u8)
                .collect();
            require(
                *outputs == expected,
                "outputs differ from committed successes in input order",
            )?;
            next.closed = true;
        }
        Event::Yield => {
            live()?;
        }
    }
    Ok(next)
}
pub fn verify(spec: Spec, events: &[Event]) -> Result<State, Violation> {
    let mut state = State::default();
    for (offset, event) in events.iter().enumerate() {
        state = step(spec, &state, event).map_err(|reason| Violation {
            offset,
            reason,
            shortest_prefix: events[..=offset].to_vec(),
        })?;
    }
    Ok(state)
}
fn enabled(spec: Spec, state: &State) -> Vec<Event> {
    if state.closed {
        return Vec::new();
    }
    if !state.owner_up {
        return vec![Event::Recover {
            generation: state.generation + 1,
            reported_started: state.started,
        }];
    }
    let mut events = Vec::new();
    if !state.crashed {
        events.push(Event::Crash);
    }
    if state.stop == Stop::None
        && state.started < spec.limit
        && state.rounds.iter().filter(|phase| active(**phase)).count() < spec.parallelism as usize
    {
        events.push(Event::Start {
            index: state.started,
            reported_started: state.started + 1,
        });
    }
    for (index, phase) in state.rounds.iter().copied().enumerate() {
        let index = index as u8;
        if active(phase) {
            if state.stop != Stop::None {
                events.push(Event::Cancel(index));
                continue;
            }
            if phase == Phase::Running {
                events.push(Event::Wait(index));
            }
            if phase != Phase::Due {
                events.extend([
                    Event::Expire(index),
                    Event::Complete {
                        index,
                        generation: state.generation,
                        accepted: true,
                    },
                    Event::Fail {
                        index,
                        global: false,
                    },
                    Event::Fail {
                        index,
                        global: true,
                    },
                ]);
            } else {
                events.extend([
                    Event::Timeout {
                        index,
                        ancestor: false,
                    },
                    Event::Timeout {
                        index,
                        ancestor: true,
                    },
                ]);
            }
        }
        if state.generation > 0 && state.rejected_mask & (1 << index) == 0 {
            events.push(Event::Complete {
                index,
                generation: state.generation - 1,
                accepted: false,
            });
        }
    }
    if !state.rounds.iter().any(|phase| active(*phase))
        && (state.started == spec.limit || state.stop != Stop::None)
    {
        events.push(Event::Finish {
            global_failure: state.stop == Stop::Global,
            propagated: state.stop == Stop::Ancestor,
            outputs: state
                .rounds
                .iter()
                .enumerate()
                .filter(|(_, phase)| **phase == Phase::Succeeded)
                .map(|(index, _)| index as u8)
                .collect(),
        });
    }
    events
}
#[derive(Debug, Default)]
pub struct Exploration {
    pub states: usize,
    pub transitions: usize,
    pub terminals: usize,
    pub deadlocks: usize,
    pub cycle: Option<Vec<usize>>,
    pub violation: Option<Violation>,
}
#[derive(Clone, Copy, Debug)]
pub enum Variant {
    Valid,
    ResetBudget,
    SwallowGlobal,
    AcceptStale,
    NonterminalSpin,
}
fn path(mut at: usize, parents: &[Option<(usize, Event)>]) -> Vec<Event> {
    let mut result = Vec::new();
    while let Some((parent, event)) = &parents[at] {
        result.push(event.clone());
        at = *parent;
    }
    result.reverse();
    result
}
/// No exploration cap: every reachable finite state is queued once. BFS makes
/// the first rejected edge a shortest counterexample in this event alphabet.
pub fn explore(spec: Spec, variant: Variant) -> Exploration {
    let mut states = vec![State::default()];
    let mut indices = BTreeMap::from([(State::default(), 0usize)]);
    let mut queue = VecDeque::from([0usize]);
    let mut parents = vec![None];
    let mut edges = vec![Vec::new()];
    let mut report = Exploration::default();
    while let Some(at) = queue.pop_front() {
        let state = states[at].clone();
        let mut events = enabled(spec, &state);
        match variant {
            Variant::ResetBudget => {
                for event in &mut events {
                    if let Event::Recover {
                        reported_started, ..
                    } = event
                    {
                        if *reported_started > 0 {
                            *reported_started = 0;
                        }
                    }
                }
            }
            Variant::SwallowGlobal => {
                for event in &mut events {
                    if let Event::Finish { global_failure, .. } = event {
                        if *global_failure {
                            *global_failure = false;
                        }
                    }
                }
            }
            Variant::AcceptStale => {
                for event in &mut events {
                    if let Event::Complete {
                        generation,
                        accepted,
                        ..
                    } = event
                    {
                        if *generation < state.generation {
                            *accepted = true;
                        }
                    }
                }
            }
            Variant::NonterminalSpin if !state.closed && state.owner_up => {
                events.push(Event::Yield)
            }
            _ => {}
        }
        if !state.closed && events.is_empty() {
            report.deadlocks += 1;
        }
        for event in events {
            report.transitions += 1;
            let next = match step(spec, &state, &event) {
                Ok(next) => next,
                Err(reason) => {
                    let mut prefix = path(at, &parents);
                    prefix.push(event);
                    report.states = states.len();
                    report.violation = Some(Violation {
                        offset: prefix.len() - 1,
                        reason,
                        shortest_prefix: prefix,
                    });
                    return report;
                }
            };
            let index = if let Some(index) = indices.get(&next) {
                *index
            } else {
                let index = states.len();
                indices.insert(next.clone(), index);
                states.push(next);
                parents.push(Some((at, event)));
                edges.push(Vec::new());
                queue.push_back(index);
                index
            };
            edges[at].push(index);
        }
    }
    report.states = states.len();
    report.terminals = states.iter().filter(|state| state.closed).count();
    report.cycle = nonterminal_cycle(
        &edges,
        &states.iter().map(|state| state.closed).collect::<Vec<_>>(),
    );
    report
}
/// Iterative DFS avoids recursion depth depending on graph size. Cycle returns
/// its actual closed walk; BFS exploration separately provides shortest failures.
pub fn nonterminal_cycle(edges: &[Vec<usize>], terminal: &[bool]) -> Option<Vec<usize>> {
    let mut colors = vec![0u8; edges.len()];
    for root in 0..edges.len() {
        if terminal[root] || colors[root] != 0 {
            continue;
        }
        colors[root] = 1;
        let mut stack = vec![(root, 0usize)];
        while let Some((at, next)) = stack.last_mut() {
            if *next == edges[*at].len() {
                colors[*at] = 2;
                stack.pop();
                continue;
            }
            let target = edges[*at][*next];
            *next += 1;
            if terminal[target] {
                continue;
            }
            if colors[target] == 1 {
                let start = stack.iter().position(|(node, _)| *node == target).unwrap();
                let mut cycle: Vec<_> = stack[start..].iter().map(|(node, _)| *node).collect();
                cycle.push(target);
                return Some(cycle);
            }
            if colors[target] == 0 {
                colors[target] = 1;
                stack.push((target, 0));
            }
        }
    }
    None
}
