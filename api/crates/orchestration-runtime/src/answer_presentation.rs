use std::collections::{BTreeMap, BTreeSet};

use crate::compiled_plan::{
    CompileIssue, CompileIssueCode, CompiledBinding, CompiledNode, CompiledPlan,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnswerPresentationPlan {
    pub answer_node_id: String,
    pub answer_output_key: String,
    pub segments: Vec<AnswerPresentationSegment>,
    pub stream_sources: BTreeMap<usize, Vec<AnswerPresentationStreamSource>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnswerPresentationStreamSource {
    pub node_id: String,
    pub output_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnswerPresentationSegment {
    StaticText(String),
    NodeOutput {
        node_id: String,
        output_path: Vec<String>,
    },
}

impl AnswerPresentationPlan {
    pub fn from_plan(plan: &CompiledPlan) -> Option<Self> {
        Self::candidates_from_plan(plan).into_iter().last()
    }

    pub fn candidates_from_plan(plan: &CompiledPlan) -> Vec<Self> {
        plan.topological_order
            .iter()
            .filter_map(|node_id| plan.nodes.get(node_id))
            .filter(|node| node.node_type == "answer")
            .filter_map(|answer_node| Self::from_answer_node(plan, answer_node))
            .collect()
    }

    fn from_answer_node(plan: &CompiledPlan, answer_node: &CompiledNode) -> Option<Self> {
        let answer_output_key = first_output_key(answer_node);
        let binding = answer_node
            .bindings
            .get("answer_template")
            .or_else(|| answer_node.bindings.values().next())?;
        let segments = segments_from_binding(binding);

        (!segments.is_empty()).then(|| {
            let stream_sources = segments
                .iter()
                .enumerate()
                .filter_map(|(index, segment)| {
                    variable_aggregator_stream_sources(plan, segment)
                        .map(|sources| (index, sources))
                })
                .collect();
            Self {
                answer_node_id: answer_node.node_id.clone(),
                answer_output_key,
                segments,
                stream_sources,
            }
        })
    }

    pub fn node_output_segments(&self) -> Vec<(usize, &str, &[String])> {
        self.segments
            .iter()
            .enumerate()
            .filter_map(|(index, segment)| match segment {
                AnswerPresentationSegment::NodeOutput {
                    node_id,
                    output_path,
                } => Some((index, node_id.as_str(), output_path.as_slice())),
                AnswerPresentationSegment::StaticText(_) => None,
            })
            .collect()
    }
}

fn variable_aggregator_stream_sources(
    plan: &CompiledPlan,
    segment: &AnswerPresentationSegment,
) -> Option<Vec<AnswerPresentationStreamSource>> {
    let AnswerPresentationSegment::NodeOutput {
        node_id,
        output_path,
    } = segment
    else {
        return None;
    };
    let [group_key] = output_path.as_slice() else {
        return None;
    };
    let aggregator = plan.nodes.get(node_id)?;
    if aggregator.node_type != "variable_aggregator" {
        return None;
    }
    let binding = aggregator.bindings.get("groups")?;
    let groups =
        crate::compiler::variable_aggregator_contract::variable_aggregator_groups(binding).ok()?;
    let group = groups.iter().find(|group| group.key == group_key)?;
    if group.value_type != "string" {
        return None;
    }
    let sources = group
        .candidates
        .iter()
        .map(|candidate| {
            let [node_id, output_key] = candidate.as_slice() else {
                return None;
            };
            (plan.nodes.get(node_id)?.node_type == "llm" && output_key == "text").then(|| {
                AnswerPresentationStreamSource {
                    node_id: node_id.clone(),
                    output_key: output_key.clone(),
                }
            })
        })
        .collect::<Option<Vec<_>>>()?;
    if sources.is_empty() {
        return None;
    }
    if sources.len() == 1 || sources_are_mutually_exclusive(plan, &sources) {
        return Some(sources);
    }
    None
}

fn sources_are_mutually_exclusive(
    plan: &CompiledPlan,
    sources: &[AnswerPresentationStreamSource],
) -> bool {
    let memberships = sources
        .iter()
        .map(|source| branch_memberships(plan, &source.node_id))
        .collect::<Vec<_>>();
    let Some(first) = memberships.first() else {
        return false;
    };

    first.iter().any(|(branch_node_id, handles)| {
        if handles.len() != 1 {
            return false;
        }
        let selected_handles = memberships
            .iter()
            .filter_map(|membership| membership.get(branch_node_id))
            .filter(|handles| handles.len() == 1)
            .filter_map(|handles| handles.first())
            .collect::<BTreeSet<_>>();
        selected_handles.len() == sources.len()
    })
}

fn branch_memberships(
    plan: &CompiledPlan,
    target_node_id: &str,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut memberships = BTreeMap::<String, BTreeSet<String>>::new();
    for edge in &plan.edges {
        let Some(source_handle) = edge.source_handle.as_ref() else {
            continue;
        };
        if plan
            .nodes
            .get(&edge.source)
            .map(|node| node.node_type.as_str())
            != Some("if_else")
            || !is_reachable(plan, &edge.target, target_node_id)
        {
            continue;
        }
        memberships
            .entry(edge.source.clone())
            .or_default()
            .insert(source_handle.clone());
    }
    memberships
}

fn is_reachable(plan: &CompiledPlan, start_node_id: &str, target_node_id: &str) -> bool {
    let mut stack = vec![start_node_id];
    let mut visited = BTreeSet::new();
    while let Some(current) = stack.pop() {
        if current == target_node_id {
            return true;
        }
        if !visited.insert(current) {
            continue;
        }
        stack.extend(
            plan.edges
                .iter()
                .filter(|edge| edge.source == current)
                .map(|edge| edge.target.as_str()),
        );
    }
    false
}

pub fn validate_answer_presentation(plan: &CompiledPlan) -> Vec<CompileIssue> {
    let mut issues = Vec::new();

    for presentation in AnswerPresentationPlan::candidates_from_plan(plan) {
        let outputs = presentation.node_output_segments();
        let mut seen = BTreeSet::new();

        for (_, node_id, output_path) in &outputs {
            if !seen.insert(((*node_id).to_string(), (*output_path).to_vec())) {
                issues.push(CompileIssue {
                    node_id: presentation.answer_node_id.clone(),
                    code: CompileIssueCode::DuplicateAnswerPresentationReference,
                    message: format!(
                        "answer presentation references {node_id}.{} more than once",
                        output_path.join(".")
                    ),
                });
            }
        }

        for (position, (_, left_node_id, _)) in outputs.iter().enumerate() {
            for (_, right_node_id, _) in outputs.iter().skip(position + 1) {
                if depends_on(plan, left_node_id, right_node_id) {
                    issues.push(CompileIssue {
                        node_id: presentation.answer_node_id.clone(),
                        code: CompileIssueCode::InvalidAnswerPresentationOrder,
                        message: format!(
                            "answer presentation places {left_node_id} before its dependency {right_node_id}"
                        ),
                    });
                    break;
                }
            }
        }
    }

    issues
}

fn depends_on(plan: &CompiledPlan, node_id: &str, dependency_node_id: &str) -> bool {
    let mut stack = vec![node_id];
    let mut visited = BTreeSet::new();

    while let Some(current) = stack.pop() {
        if !visited.insert(current.to_string()) {
            continue;
        }
        let Some(node) = plan.nodes.get(current) else {
            continue;
        };
        for dependency in &node.dependency_node_ids {
            if dependency == dependency_node_id {
                return true;
            }
            stack.push(dependency);
        }
    }

    false
}

fn first_output_key(node: &CompiledNode) -> String {
    node.outputs
        .first()
        .map(|output| output.key.clone())
        .unwrap_or_else(|| "answer".to_string())
}

fn segments_from_binding(binding: &CompiledBinding) -> Vec<AnswerPresentationSegment> {
    match binding.kind.as_str() {
        "selector" => binding
            .selector_paths
            .first()
            .and_then(|selector| selector_segment(selector))
            .into_iter()
            .collect(),
        "templated_text" => binding
            .raw_value
            .as_str()
            .map(parse_templated_text_segments)
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn selector_segment(selector: &[String]) -> Option<AnswerPresentationSegment> {
    if selector.len() < 2 {
        return None;
    }

    Some(AnswerPresentationSegment::NodeOutput {
        node_id: selector[0].clone(),
        output_path: selector[1..].to_vec(),
    })
}

fn parse_templated_text_segments(template: &str) -> Vec<AnswerPresentationSegment> {
    let mut segments = Vec::new();
    let mut cursor = 0;

    while let Some(start_offset) = template[cursor..].find("{{") {
        let start = cursor + start_offset;
        push_static_segment(&mut segments, &template[cursor..start]);
        let token_start = start + 2;
        let Some(end_offset) = template[token_start..].find("}}") else {
            push_static_segment(&mut segments, &template[start..]);
            return segments;
        };
        let token_end = token_start + end_offset;
        let selector = template[token_start..token_end]
            .trim()
            .split('.')
            .map(str::trim)
            .map(str::to_string)
            .collect::<Vec<_>>();

        if let Some(segment) = selector_segment(&selector) {
            segments.push(segment);
        } else {
            push_static_segment(&mut segments, &template[start..token_end + 2]);
        }

        cursor = token_end + 2;
    }

    push_static_segment(&mut segments, &template[cursor..]);
    segments
}

fn push_static_segment(segments: &mut Vec<AnswerPresentationSegment>, text: &str) {
    if text.is_empty() {
        return;
    }

    if let Some(AnswerPresentationSegment::StaticText(previous)) = segments.last_mut() {
        previous.push_str(text);
        return;
    }

    segments.push(AnswerPresentationSegment::StaticText(text.to_string()));
}
