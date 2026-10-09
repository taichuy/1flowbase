use super::*;

/// Scan only recovered metadata and explicitly selected semantic sections. Raw
/// archives are never opened. Memory is bounded by one source page + result page.
pub(in crate::orchestration_runtime_repository) async fn trajectory(
    store: &PgControlPlaneStore,
    application_id: Uuid,
    record_id: Uuid,
    query: &RecordClientTrajectoryQuery,
) -> Result<RecordClientTrajectoryQueryPage> {
    let lookahead = usize::try_from(log_query_page_lookahead(query.limit)?)
        .map_err(|_| invalid("log_query_limit"))?;
    let limit = lookahead - 1;
    let fields = record_trajectory_query_fields();
    validate_log_query_filter(&query.filter, &fields)?;
    let sections = log_query_search_sections(query)?;
    let identity = record_trajectory_query_fingerprint(application_id, record_id, query, &sections);
    let mut cursor: Option<String> = decode(query.cursor.as_deref(), &identity)?;
    let native =
        super::super::agent_logs::reads::record_native_run(store, application_id, record_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("log_record"))?
            .is_some();
    if native
        && cursor
            .as_ref()
            .is_some_and(|c| c.parse::<i64>().ok().is_none_or(|n| n < 0))
    {
        return Err(invalid("log_query_cursor"));
    }
    let mut items = Vec::new();
    let mut matches = Vec::new();
    let mut positions = Vec::new();
    let mut integrity = "complete".to_owned();
    loop {
        let page =
            super::super::agent_logs::page(store, application_id, record_id, cursor.clone(), 100)
                .await?;
        if page.integrity != "complete" {
            integrity = page.integrity;
        }
        for step in page.items {
            // Native compact metadata is restored by the canonical reader before
            // this predicate; NUL and source identity remain with their step.
            if !log_query_filter_matches(&query.filter, &serde_json::to_value(&step)?, &fields) {
                continue;
            }
            let mut step_matches = Vec::new();
            if let Some(keyword) = query.keyword.as_deref() {
                for section in &sections {
                    if !step.available_sections.contains(section) {
                        continue;
                    }
                    let mut section_cursor = None;
                    loop {
                        let Some(part) = super::super::agent_logs::section(
                            store,
                            application_id,
                            record_id,
                            step.id,
                            section,
                            section_cursor,
                            32,
                        )
                        .await?
                        else {
                            break;
                        };
                        let found = part.items.iter().find_map(|item| {
                            semantic_snippet(&item.value, keyword).map(|snippet| {
                                RecordTrajectoryMatch {
                                    step_id: step.id,
                                    section: section.clone(),
                                    sequence: item.sequence,
                                    snippet,
                                }
                            })
                        });
                        if let Some(found) = found {
                            step_matches.push(found);
                            break;
                        }
                        match part.next_cursor {
                            Some(next) if section_cursor.is_none_or(|previous| next > previous) => {
                                section_cursor = Some(next)
                            }
                            Some(_) => {
                                return Err(anyhow!("log_query.section_cursor_not_advancing"));
                            }
                            None => break,
                        }
                    }
                }
                if step_matches.is_empty() {
                    continue;
                }
            }
            let position = if native {
                step.sequence.to_string()
            } else {
                match RecordClientTrajectoryCursor::imported(step.sequence, step.id) {
                    RecordClientTrajectoryCursor::Imported(c) => c,
                    _ => unreachable!(),
                }
            };
            positions.push(position);
            matches.extend(step_matches);
            items.push(step);
            if items.len() > limit {
                break;
            }
        }
        if items.len() > limit {
            break;
        }
        let next = page.next_cursor.map(|c| match c {
            RecordClientTrajectoryCursor::Native(n) => n.to_string(),
            RecordClientTrajectoryCursor::Imported(c) => c,
        });
        match next {
            Some(next) if cursor.as_ref() != Some(&next) => cursor = Some(next),
            Some(_) => return Err(anyhow!("log_query.cursor_not_advancing")),
            None => break,
        }
    }
    let more = items.len() > limit;
    items.truncate(limit);
    matches.retain(|m| items.iter().any(|s| s.id == m.step_id));
    let next_cursor = if more {
        Some(encode(&positions[limit - 1], &identity)?)
    } else {
        None
    };
    Ok(RecordClientTrajectoryQueryPage {
        items,
        matches,
        next_cursor,
        search_sections: if query.keyword.is_some() {
            sections
        } else {
            Vec::new()
        },
        integrity,
    })
}
fn semantic_snippet(value: &Value, keyword: &str) -> Option<String> {
    match value {
        Value::String(text) => log_query_snippet(text, keyword),
        Value::Array(items) => items.iter().find_map(|v| semantic_snippet(v, keyword)),
        Value::Object(object) => object.iter().find_map(|(key, value)| {
            log_query_snippet(key, keyword).or_else(|| semantic_snippet(value, keyword))
        }),
        Value::Null => None,
        other => log_query_snippet(&other.to_string(), keyword),
    }
}
