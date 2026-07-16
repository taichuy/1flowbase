use std::collections::BTreeSet;

use crate::compiled_plan::{
    CompileIssue, CompileIssueCode, CompiledBinding, CompiledNode, CompiledPlan,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnswerPresentationPlan {
    pub answer_node_id: String,
    pub answer_output_key: String,
    pub segments: Vec<AnswerPresentationSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnswerPresentationSegment {
    StaticText(String),
    NodeOutput { node_id: String, output_key: String },
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
            .filter_map(Self::from_answer_node)
            .collect()
    }

    fn from_answer_node(answer_node: &CompiledNode) -> Option<Self> {
        let answer_output_key = first_output_key(answer_node);
        let binding = answer_node
            .bindings
            .get("answer_template")
            .or_else(|| answer_node.bindings.values().next())?;
        let segments = segments_from_binding(binding);

        (!segments.is_empty()).then(|| Self {
            answer_node_id: answer_node.node_id.clone(),
            answer_output_key,
            segments,
        })
    }

    pub fn node_output_segments(&self) -> Vec<(usize, &str, &str)> {
        self.segments
            .iter()
            .enumerate()
            .filter_map(|(index, segment)| match segment {
                AnswerPresentationSegment::NodeOutput {
                    node_id,
                    output_key,
                } => Some((index, node_id.as_str(), output_key.as_str())),
                AnswerPresentationSegment::StaticText(_) => None,
            })
            .collect()
    }
}

pub fn validate_answer_presentation(plan: &CompiledPlan) -> Vec<CompileIssue> {
    let mut issues = Vec::new();

    for presentation in AnswerPresentationPlan::candidates_from_plan(plan) {
        let outputs = presentation.node_output_segments();
        let mut seen = BTreeSet::new();

        for (_, node_id, output_key) in &outputs {
            if !seen.insert(((*node_id).to_string(), (*output_key).to_string())) {
                issues.push(CompileIssue {
                    node_id: presentation.answer_node_id.clone(),
                    code: CompileIssueCode::DuplicateAnswerPresentationReference,
                    message: format!(
                        "answer presentation references {node_id}.{output_key} more than once"
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
        output_key: selector[1].clone(),
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
