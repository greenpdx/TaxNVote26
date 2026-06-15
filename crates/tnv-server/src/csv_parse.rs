// src/csv_parse.rs

use crate::models::*;
use std::collections::HashMap;

pub fn parse_template_csv(raw: &str) -> Result<ParsedTemplate, String> {
    if raw.len() > MAX_CSV_BYTES {
        return Err(format!("CSV exceeds {} bytes", MAX_CSV_BYTES));
    }

    let mut meta = HashMap::new();
    let mut data_lines: Vec<&str> = Vec::new();
    let mut found_header = false;

    let mut lines = raw.lines();

    // First line must be magic
    let first = lines.next().ok_or("empty CSV")?;
    if first.trim() != "#TNV-TEMPLATE" {
        return Err("first line must be #TNV-TEMPLATE".into());
    }

    for line in lines {
        let line = line.trim();
        if line.is_empty() { continue; }

        if line.starts_with('#') {
            let content = &line[1..];
            if let Some(pos) = content.find(',') {
                let key = content[..pos].trim().to_lowercase();
                let val = content[pos + 1..].trim().to_string();
                meta.insert(key, val);
            }
            continue;
        }

        if !found_header {
            // Templates store percentages (fractions of the total); the dollar
            // amount is derived as pct × total at display/load time.
            let lower = line.to_lowercase().replace(' ', "");
            if lower != "id,pct" {
                return Err(format!("expected header 'id,pct', got '{}'", line));
            }
            found_header = true;
            continue;
        }

        data_lines.push(line);
    }

    if !found_header {
        return Err("missing header line 'id,pct'".into());
    }

    let name = meta.get("name").cloned().ok_or("missing #name metadata")?;
    if name.len() < TPL_NAME_MIN || name.len() > TPL_NAME_MAX {
        return Err(format!("name must be {}-{} chars", TPL_NAME_MIN, TPL_NAME_MAX));
    }

    let description = meta.get("description").cloned().unwrap_or_default();
    if description.len() > TPL_DESC_MAX {
        return Err(format!("description exceeds {} chars", TPL_DESC_MAX));
    }

    // Entity = the org/person publishing the template (optional, ≤128).
    let entity_name = meta.get("entity").cloned().unwrap_or_default();
    if entity_name.len() > TPL_NAME_MAX {
        return Err(format!("entity exceeds {} chars", TPL_NAME_MAX));
    }

    let fiscal_year = meta.get("fiscal_year").cloned()
        .ok_or("missing #fiscal_year metadata")?;
    if fiscal_year.len() != FISCAL_YEAR_LEN || !fiscal_year.chars().all(|c| c.is_ascii_digit()) {
        return Err("fiscal_year must be 4 digits".into());
    }

    let mut entries = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    for line in &data_lines {
        let parts: Vec<&str> = line.splitn(2, ',').collect();
        if parts.len() != 2 {
            return Err(format!("bad data line: '{}'", line));
        }
        let node_id = parts[0].trim().to_string();
        let val_str = parts[1].trim().replace(',', "");

        if node_id.len() < NODE_ID_MIN || node_id.len() > NODE_ID_MAX {
            return Err(format!("node_id '{}' length out of range", node_id));
        }
        if !seen_ids.insert(node_id.clone()) {
            return Err(format!("duplicate node_id: '{}'", node_id));
        }

        let value: f64 = val_str.parse()
            .map_err(|_| format!("invalid value for '{}': '{}'", node_id, val_str))?;
        if value < 0.0 {
            return Err(format!("negative value for '{}'", node_id));
        }

        entries.push(TemplateEntry { node_id, value });
    }

    if entries.is_empty() || entries.len() > MAX_ENTRIES {
        return Err(format!("entries must be 1-{}", MAX_ENTRIES));
    }

    Ok(ParsedTemplate {
        name,
        entity_name,
        description,
        fiscal_year,
        entries,
        raw_csv: raw.to_string(),
    })
}

pub fn parse_taxdollar_csv(raw: &str) -> Result<ParsedTaxDollar, String> {
    if raw.len() > MAX_CSV_BYTES {
        return Err(format!("CSV exceeds {} bytes", MAX_CSV_BYTES));
    }

    let mut meta = HashMap::new();
    let mut data_lines: Vec<&str> = Vec::new();
    let mut found_header = false;

    let mut lines = raw.lines();

    let first = lines.next().ok_or("empty CSV")?;
    if first.trim() != "#TNV-TAXDOLLAR" {
        return Err("first line must be #TNV-TAXDOLLAR".into());
    }

    for line in lines {
        let line = line.trim();
        if line.is_empty() { continue; }

        if line.starts_with('#') {
            let content = &line[1..];
            if let Some(pos) = content.find(',') {
                let key = content[..pos].trim().to_lowercase();
                let val = content[pos + 1..].trim().to_string();
                meta.insert(key, val);
            }
            continue;
        }

        if !found_header {
            let lower = line.to_lowercase().replace(' ', "");
            if lower != "id,pct" {
                return Err(format!("expected header 'id,pct', got '{}'", line));
            }
            found_header = true;
            continue;
        }

        data_lines.push(line);
    }

    if !found_header {
        return Err("missing header line 'id,pct'".into());
    }

    let version = meta.get("version").cloned().unwrap_or_default();
    if version != "1" {
        return Err(format!("unsupported version: '{}'", version));
    }

    let fiscal_year = meta.get("fiscal_year").cloned()
        .ok_or("missing #fiscal_year metadata")?;
    if fiscal_year.len() != FISCAL_YEAR_LEN || !fiscal_year.chars().all(|c| c.is_ascii_digit()) {
        return Err("fiscal_year must be 4 digits".into());
    }

    let template_id = meta.get("template_id").cloned()
        .ok_or("missing #template_id metadata")?;
    if template_id.len() < NODE_ID_MIN || template_id.len() > NODE_ID_MAX {
        return Err(format!("template_id must be {}-{} chars", NODE_ID_MIN, NODE_ID_MAX));
    }

    let timestamp = meta.get("timestamp").cloned().unwrap_or_default();

    let checksum = meta.get("checksum").cloned()
        .ok_or("missing #checksum metadata")?;
    if checksum.len() != CHECKSUM_LEN || !checksum.starts_with("sha256:") {
        return Err(format!("checksum must be {} chars starting with 'sha256:'", CHECKSUM_LEN));
    }

    let mut allocations = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    for line in &data_lines {
        let parts: Vec<&str> = line.splitn(2, ',').collect();
        if parts.len() != 2 {
            return Err(format!("bad data line: '{}'", line));
        }
        let node_id = parts[0].trim().to_string();
        let pct_str = parts[1].trim();

        if node_id.len() < NODE_ID_MIN || node_id.len() > NODE_ID_MAX {
            return Err(format!("node_id '{}' length out of range", node_id));
        }
        if !seen_ids.insert(node_id.clone()) {
            return Err(format!("duplicate node_id: '{}'", node_id));
        }

        let pct: f64 = pct_str.parse()
            .map_err(|_| format!("invalid pct for '{}': '{}'", node_id, pct_str))?;
        if pct < 0.0 || pct > 1.0 {
            return Err(format!("pct for '{}' out of range [0,1]: {}", node_id, pct));
        }

        allocations.push(Allocation { node_id, pct });
    }

    if allocations.is_empty() || allocations.len() > MAX_ENTRIES {
        return Err(format!("allocations must be 1-{}", MAX_ENTRIES));
    }

    // Sum check
    let total: f64 = allocations.iter().map(|a| a.pct).sum();
    if (total - 1.0).abs() > 0.0001 {
        return Err(format!("allocation pct sum = {:.6}, must equal 1.0", total));
    }

    Ok(ParsedTaxDollar {
        fiscal_year,
        template_id,
        timestamp,
        checksum,
        allocations,
        raw_csv: raw.to_string(),
    })
}

// ─── Tests ───────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_template() {
        let csv = "#TNV-TEMPLATE\n#name,Test Budget\n#fiscal_year,2021\nid,pct\na:010,500000\na:020,150000\n";
        let t = parse_template_csv(csv).unwrap();
        assert_eq!(t.name, "Test Budget");
        assert_eq!(t.fiscal_year, "2021");
        assert_eq!(t.entries.len(), 2);
    }

    #[test]
    fn test_template_missing_magic() {
        let csv = "#name,Test\n#fiscal_year,2021\nid,pct\na:010,500000\n";
        assert!(parse_template_csv(csv).is_err());
    }

    #[test]
    fn test_template_name_too_long() {
        let long_name = "x".repeat(200);
        let csv = format!("#TNV-TEMPLATE\n#name,{}\n#fiscal_year,2021\nid,pct\na:010,100\n", long_name);
        assert!(parse_template_csv(&csv).is_err());
    }

    #[test]
    fn test_template_duplicate_ids() {
        let csv = "#TNV-TEMPLATE\n#name,Dup\n#fiscal_year,2021\nid,pct\na:010,100\na:010,200\n";
        assert!(parse_template_csv(csv).is_err());
    }

    #[test]
    fn test_template_negative_value() {
        let csv = "#TNV-TEMPLATE\n#name,Neg\n#fiscal_year,2021\nid,pct\na:010,-100\n";
        assert!(parse_template_csv(csv).is_err());
    }

    #[test]
    fn test_parse_valid_taxdollar() {
        let csv = "#TNV-TAXDOLLAR\n#version,1\n#fiscal_year,2021\n#template_id,TPL-2026-000001\n#timestamp,2026-05-21T00:00:00Z\n#checksum,sha256:0000000000000000000000000000000000000000000000000000000000000000\nid,pct\na:010,0.600000\na:020,0.400000\n";
        let td = parse_taxdollar_csv(csv).unwrap();
        assert_eq!(td.fiscal_year, "2021");
        assert_eq!(td.allocations.len(), 2);
    }

    #[test]
    fn test_taxdollar_bad_sum() {
        let csv = "#TNV-TAXDOLLAR\n#version,1\n#fiscal_year,2021\n#template_id,TPL-2026-000001\n#timestamp,2026-05-21T00:00:00Z\n#checksum,sha256:0000000000000000000000000000000000000000000000000000000000000000\nid,pct\na:010,0.300000\na:020,0.300000\n";
        let err = parse_taxdollar_csv(csv).unwrap_err();
        assert!(err.contains("sum"));
    }

    #[test]
    fn test_taxdollar_bad_version() {
        let csv = "#TNV-TAXDOLLAR\n#version,2\n#fiscal_year,2021\n#template_id,TPL-001\n#timestamp,x\n#checksum,sha256:0000000000000000000000000000000000000000000000000000000000000000\nid,pct\na:010,1.000000\n";
        let err = parse_taxdollar_csv(csv).unwrap_err();
        assert!(err.contains("version"), "expected version error, got: {}", err);
    }

    #[test]
    fn test_taxdollar_pct_out_of_range() {
        let csv = "#TNV-TAXDOLLAR\n#version,1\n#fiscal_year,2021\n#template_id,TPL-001\n#timestamp,x\n#checksum,sha256:0000000000000000000000000000000000000000000000000000000000000000\nid,pct\na:010,1.500000\n";
        assert!(parse_taxdollar_csv(csv).is_err());
    }

    // ─── Length limit tests ──────────────────────────────────────

    #[test]
    fn test_template_name_too_short() {
        let csv = "#TNV-TEMPLATE\n#name,ab\n#fiscal_year,2021\nid,pct\na:010,100\n";
        let err = parse_template_csv(csv).unwrap_err();
        assert!(err.contains("name"), "got: {}", err);
    }

    #[test]
    fn test_template_desc_too_long() {
        let long_desc = "x".repeat(513);
        let csv = format!("#TNV-TEMPLATE\n#name,Good Name\n#description,{}\n#fiscal_year,2021\nid,pct\na:010,100\n", long_desc);
        let err = parse_template_csv(&csv).unwrap_err();
        assert!(err.contains("description"), "got: {}", err);
    }

    #[test]
    fn test_template_bad_fiscal_year() {
        let csv = "#TNV-TEMPLATE\n#name,Good Name\n#fiscal_year,20\nid,pct\na:010,100\n";
        let err = parse_template_csv(csv).unwrap_err();
        assert!(err.contains("fiscal_year"), "got: {}", err);
    }

    #[test]
    fn test_template_node_id_too_long() {
        let long_id = "a:".to_string() + &"0".repeat(31);
        let csv = format!("#TNV-TEMPLATE\n#name,Good Name\n#fiscal_year,2021\nid,pct\n{},100\n", long_id);
        let err = parse_template_csv(&csv).unwrap_err();
        assert!(err.contains("node_id"), "got: {}", err);
    }

    #[test]
    fn test_template_csv_too_large() {
        // 512001 bytes should fail
        let padding = "a:010,100\n".repeat(60000); // ~600KB
        let csv = format!("#TNV-TEMPLATE\n#name,Big\n#fiscal_year,2021\nid,pct\n{}", padding);
        let err = parse_template_csv(&csv).unwrap_err();
        assert!(err.contains("bytes") || err.contains("exceed"), "got: {}", err);
    }

    #[test]
    fn test_taxdollar_template_id_too_short() {
        let csv = "#TNV-TAXDOLLAR\n#version,1\n#fiscal_year,2021\n#template_id,ab\n#timestamp,x\n#checksum,sha256:0000000000000000000000000000000000000000000000000000000000000000\nid,pct\na:010,1.000000\n";
        let err = parse_taxdollar_csv(csv).unwrap_err();
        assert!(err.contains("template_id"), "got: {}", err);
    }

    #[test]
    fn test_template_too_many_entries() {
        let mut csv = "#TNV-TEMPLATE\n#name,Big Template\n#fiscal_year,2021\nid,pct\n".to_string();
        for i in 0..5001 {
            csv.push_str(&format!("c:{:06},100\n", i));
        }
        let err = parse_template_csv(&csv).unwrap_err();
        // Could be entries limit or CSV size limit
        assert!(err.contains("entries") || err.contains("bytes"), "got: {}", err);
    }
}
