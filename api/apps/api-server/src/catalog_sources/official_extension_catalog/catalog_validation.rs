use super::*;

pub(super) fn validate_index(index: &CatalogIndexDocument, category: &str) -> Result<()> {
    if index.schema_version != EXTENSION_CATALOG_SCHEMA_VERSION || index.category != category {
        bail!("official extension catalog index contract mismatch");
    }
    if index.page_size == 0 || index.pages.is_empty() {
        bail!("official extension catalog index has no usable pages");
    }
    if index.search_index.schema_version != EXTENSION_CATALOG_SEARCH_SCHEMA_VERSION
        || index.search_index.entry_count != index.total_entries
        || index.search_index.checksum.trim().is_empty()
        || index.search_index.locator.trim().is_empty()
    {
        bail!("official extension catalog search reference contract mismatch");
    }
    let first = index
        .pages
        .iter()
        .find(|page| page.page == index.first_page.page)
        .ok_or_else(|| anyhow!("official extension catalog first page is missing"))?;
    if first.cursor != index.first_page.cursor || first.locator != index.first_page.locator {
        bail!("official extension catalog first page reference mismatch");
    }
    Ok(())
}

pub(super) fn validate_search_document(
    document: &CatalogSearchDocument,
    index: &CatalogIndexDocument,
) -> Result<()> {
    if document.schema_version != EXTENSION_CATALOG_SEARCH_SCHEMA_VERSION
        || document.category != index.category
        || document.entries.len() != index.search_index.entry_count
        || document.source_fingerprint.trim().is_empty()
        || document.entries.iter().any(|entry| {
            entry.category != index.category
                || entry.slot_codes.iter().any(|value| value.trim().is_empty())
                || entry.keywords.iter().any(|value| value.trim().is_empty())
                || domain::ExtensionCategory::parse(&index.category)
                    .and_then(|category| {
                        domain::ExtensionCatalogIdentity::parse(category, &entry.id)
                    })
                    .is_none_or(|identity| {
                        identity.organization() != entry.organization
                            || identity.artifact_id() != entry.artifact
                    })
                || !index.pages.iter().any(|page| {
                    page.page == entry.catalog_page.page
                        && page.cursor == entry.catalog_page.cursor
                        && page.checksum == entry.catalog_page.checksum
                        && page.locator == entry.catalog_page.locator
                })
        })
    {
        bail!("official extension catalog search document contract mismatch");
    }
    Ok(())
}

pub(super) fn normalized_filter(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_lowercase)
}

pub(super) fn search_query_binding(
    category: &str,
    slot_code: Option<&str>,
    q: Option<&str>,
    limit: usize,
) -> String {
    digest_text(&format!(
        "{category}\u{0}{}\u{0}{}\u{0}{limit}",
        slot_code.unwrap_or_default(),
        q.unwrap_or_default()
    ))
}

pub(super) fn digest_text(value: &str) -> String {
    format!(
        "{:x}",
        base16ct::HexDisplay(&Sha256::digest(value.as_bytes()))
    )
}

pub(super) fn encode_search_cursor(
    checksum: &str,
    source_fingerprint: &str,
    query_binding: &str,
    offset: usize,
) -> String {
    format!(
        "v1.{}.{}.{}.{offset}",
        digest_text(checksum),
        digest_text(source_fingerprint),
        query_binding
    )
}

pub(super) fn decode_search_cursor(
    cursor: Option<&str>,
    checksum: &str,
    source_fingerprint: &str,
    query_binding: &str,
) -> Result<usize> {
    let Some(cursor) = cursor.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(0);
    };
    let mut parts = cursor.split('.');
    let valid = parts.next() == Some("v1")
        && parts.next() == Some(digest_text(checksum).as_str())
        && parts.next() == Some(digest_text(source_fingerprint).as_str())
        && parts.next() == Some(query_binding)
        && parts.clone().count() == 1;
    if !valid {
        bail!("official extension catalog search cursor does not match this snapshot and query");
    }
    parts
        .next()
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| anyhow!("official extension catalog search cursor is invalid"))
}

pub(super) fn select_search_entries<'a>(
    category: &str,
    snapshot: &'a VerifiedSearchSnapshot,
    query: &OfficialExtensionCatalogSearchQuery,
) -> Result<CatalogSearchSelection<'a>> {
    let normalized_slot = normalized_filter(query.slot_code.as_deref());
    let normalized_q = normalized_filter(query.q.as_deref());
    let query_binding = search_query_binding(
        category,
        normalized_slot.as_deref(),
        normalized_q.as_deref(),
        query.limit,
    );
    let offset = decode_search_cursor(
        query.cursor.as_deref(),
        &snapshot.checksum,
        &snapshot.document.source_fingerprint,
        &query_binding,
    )?;
    let matches = snapshot
        .document
        .entries
        .iter()
        .filter(|entry| {
            normalized_slot
                .as_ref()
                .is_none_or(|slot| entry.slot_codes.iter().any(|candidate| candidate == slot))
                && normalized_q
                    .as_ref()
                    .is_none_or(|needle| entry.matches(needle))
        })
        .collect::<Vec<_>>();
    if offset > matches.len() {
        bail!("official extension catalog search cursor is out of range");
    }
    let selected = matches
        .iter()
        .skip(offset)
        .take(query.limit)
        .copied()
        .collect::<Vec<_>>();
    let total_entries = matches.len();
    let next_offset = offset + selected.len();
    let next_cursor = (next_offset < total_entries).then(|| {
        encode_search_cursor(
            &snapshot.checksum,
            &snapshot.document.source_fingerprint,
            &query_binding,
            next_offset,
        )
    });
    Ok(CatalogSearchSelection {
        selected,
        total_entries,
        next_cursor,
    })
}

pub(super) fn validate_page(
    page: &CatalogPageDocument,
    category: &str,
    page_reference: &CatalogPageReference,
) -> Result<()> {
    if page.schema_version != EXTENSION_CATALOG_SCHEMA_VERSION
        || page.category != category
        || page.page != page_reference.page
        || page.cursor != page_reference.cursor
        || page.entries.len() != page_reference.entry_count
        || page.entries.iter().any(|entry| {
            entry.category != category
                || entry.catalog_page != page_reference.page
                || entry.slot_codes.iter().any(|value| value.trim().is_empty())
                || entry.keywords.iter().any(|value| value.trim().is_empty())
                || domain::ExtensionCategory::parse(category)
                    .and_then(|category| {
                        domain::ExtensionCatalogIdentity::parse(category, &entry.id)
                    })
                    .is_none_or(|identity| {
                        identity.organization() != entry.organization
                            || identity.artifact_id() != entry.artifact
                    })
        })
    {
        bail!("official extension catalog page contract mismatch");
    }
    Ok(())
}
