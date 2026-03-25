use crate::config::{
    schema::root::RootConfig,
    lifecycle::{
        state::{LiveState, SourceInstanceState},
        diff::{RootConfigDiff, Operation},
    },
};

use std::collections::{HashSet, HashMap, VecDeque};

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub enum Subsystem {
    CoreAgent,
    Security,
    Storage,
    Processor,
    SourceDrivers,
}

pub struct ApplyPlan {
    pub steps: Vec<Subsystem>,
    pub total_steps: usize,
}

pub fn apply_config(
    state: &mut LiveState,
    diff: RootConfigDiff,
    new_config: RootConfig
) -> Result<(), String> {
    if !state.can_apply_config() {
        return Err("Unable to apply config in current lifecycle state".into());
    }

    let total_required_ops = estimate_total_ops(&diff);

    state.begin_apply(total_required_ops);

    if let Err(e) = apply_diff(state, &diff) {
        state.fail_apply(e.clone());
        return Err(e);
    }

    state.applied_config(new_config);
    state.finish_apply();
    state.refigure_health();

    Ok(())
}

fn apply_diff(
    state: &mut LiveState,
    diff: &RootConfigDiff,
) -> Result<(), String> {
    let plan = build_apply_plan(&diff)?;

    for step in plan.steps {
        if let Err(e) = execute_step(state, diff, step) {
            state.fail_apply(e.clone());
            return Err(e);
        }
    }

    Ok(())
}

fn build_apply_plan(diff: &RootConfigDiff) -> Result<ApplyPlan, String> {
    if !diff.validation_errors.is_empty() {
        return Err("Invalid config, unable to build plan".into());
    }

    if !diff.has_changes {
        return Ok(ApplyPlan {
            steps: Vec::new(),
            total_steps: 0,
        });
    }

    let mut subsystems = Vec::new();

    for op in &diff.operations {
        let s = operation_subsystem(op);
        if !subsystems.contains(&s) {
            subsystems.push(s);
        }
    }

    config_dependency_order(&mut subsystems)?;

    Ok(ApplyPlan {
        steps: subsystems,
        total_steps: subsystems.len(),
    })
}

fn operation_subsystem(op: &Operation) -> Subsystem {
    match op {
        Operation::UpdateCoreAgentRuntime(_) =>
            Subsystem::CoreAgent,
        Operation::EnableSource { .. }
        | Operation::DisableSource { .. }
        | Operation::AddSource { .. }
        | Operation::RemoveSource { .. }
        | Operation::UpdateSource { .. } =>
            Subsystem::SourceDrivers,
        Operation::UpdateProcessor(_) =>
            Subsystem::Processor,
        Operation::EnableStorage(_)
        | Operation::DisableStorage
        | Operation::UpdateStorage(_) =>
            Subsystem::Storage,
        Operation::EnableSecurity(_)
        | Operation::DisableSecurity
        | Operation::UpdateSecurity(_) =>
            Subsystem::Security,
    }
}

fn config_dependency_order(steps: &mut Vec<Subsystem>) -> Result<(), String> {
    let step_set: HashSet<_> = steps.iter().copied().collect();

    let mut in_degree: HashMap<Subsystem, usize> = HashMap::new();

    for &s in &step_set {
        let deps = dependencies(s)
            .iter()
            .filter(|d| step_set.contains(d))
            .count();

        in_degree.insert(s, deps);
    }

    let mut queue = VecDeque::new();

    for (&s, &deg) in &in_degree {
        if deg == 0 {
            queue.push_back(s);
        }
    }

    let mut ordered = Vec::new();

    while let Some(s) = queue.pop_front() {
        ordered.push(s);
        for &other in &step_set {
            if dependencies(other).contains(&s) {
                let deg = in_degree.get_mut(&other).unwrap();
                *deg -= 1;

                if *deg == 0 {
                    queue.push_back(other);
                }
            }
        }
    }

    if ordered.len() != step_set.len() {
        return Err(
            "Circular dependency detected in core agent subsystem graph".into()
        );
    }

    *steps = ordered;

    Ok(())
}

fn dependencies(s: Subsystem) -> &'static [Subsystem] {
    match s {
        Subsystem::CoreAgent => &[],
        Subsystem::Security => &[Subsystem::CoreAgent],
        Subsystem::Storage => &[Subsystem::Security],
        Subsystem::Processor => &[Subsystem::Storage],
        Subsystem::SourceDrivers => &[Subsystem::Processor],
    }
}

fn execute_step(
    state: &mut LiveState,
    diff: &RootConfigDiff,
    step: Subsystem,
) -> Result<(), String> {
    match step {
        Subsystem::CoreAgent => apply_runtime(state, diff),
        Subsystem::Security => apply_security(state, diff),
        Subsystem::Storage => apply_storage(state, diff),
        Subsystem::Processor => apply_processor(state, diff),
        Subsystem::SourceDrivers => apply_sources(state, diff),
    }
}

#[allow(unused_variables)]
fn apply_runtime(
    state: &mut LiveState,
    diff: &RootConfigDiff
) -> Result<(), String> {
    unimplemented!("Apply Core Agent runtime RootConfigDiffs")
}

#[allow(unused_variables)]
fn apply_security(
    state: &mut LiveState,
    diff: &RootConfigDiff
) -> Result<(), String> {
    unimplemented!("Apply Core Agent security RootConfigDiffs")
}

#[allow(unused_variables)]
fn apply_storage(
    state: &mut LiveState,
    diff: &RootConfigDiff,
) -> Result<(), String> {
    unimplemented!("Apply Core Agent storage RootConfigDiffs")
}

#[allow(unused_variables)]
fn apply_processor(
    state: &mut LiveState,
    diff: &RootConfigDiff,
) -> Result<(), String> {
    unimplemented!("Apply Core Agent processor RootConfigDiffs")
}

#[allow(unused_variables)]
fn apply_sources(
    state: &mut LiveState,
    diff: &RootConfigDiff,
) -> Result<(), String> {
    unimplemented!("Apply Core Agent source driver(s) RootConfigDiffs")
}

#[allow(unused_variables)]
fn start_source_driver(instance: &mut SourceInstanceState) -> Result<(), String> {
    unimplemented!(
        "This starts an actual Source Driver instance, performing any side effects necessary for this"
    )
}

#[allow(unused_variables)]
fn restart_source_driver(instance: &mut SourceInstanceState) -> Result<(), String> {
    unimplemented!(
        "This restarts a Source Driver instance, performing any side effects necessary for this"
    )
}

#[allow(unused_variables)]
fn stop_source(instance: &mut SourceInstanceState) -> Result<(), String> {
    unimplemented!(
        "This stops a running Source Driver instance, performing any necessary side effects for this"
    )
}

#[allow(unused_variables)]
fn estimate_total_ops(diff: &RootConfigDiff) -> usize {
    unimplemented!("Based on RootConfigDiff, determine total ops needed")
}
