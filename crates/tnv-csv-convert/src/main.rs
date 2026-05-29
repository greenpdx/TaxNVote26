// tnv-csv-convert: OMB Public Budget Database XLSX → TNV budauth.csv
//
// Reads BUDGET-XXXX-DB-1.xlsx (Budget Authority), produces a CSV with:
//   - Synthesized parent rows: root, agency totals, bureau totals
//   - All account-level rows from the XLSX
//   - CGAC Agency Code preserved
//   - Year columns selectable via --start-year / --end-year
//   - Duplicate keys summed
//   - Optional --skip-all-zero to drop rows with no non-zero year values

use calamine::{open_workbook, Data, Reader, Xlsx};
use std::collections::BTreeMap;
use std::env;
use std::fs::File;
use std::io::{self, BufWriter, Write};

// ─── XLSX column indices ─────────────────────────────────────────

const COL_AGENCY_CODE: usize = 0;
const COL_AGENCY_NAME: usize = 1;
const COL_BUREAU_CODE: usize = 2;
const COL_BUREAU_NAME: usize = 3;
const COL_ACCOUNT_CODE: usize = 4;
const COL_ACCOUNT_NAME: usize = 5;
const COL_TREASURY_CODE: usize = 6;
const COL_CGAC_CODE: usize = 7;
const COL_FIRST_YEAR: usize = 12; // column index of "1976"

// ─── Parsed row ──────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct BudgetRow {
    agency_code: String,
    agency_name: String,
    bureau_code: String,
    bureau_name: String,
    account_code: String,
    account_name: String,
    treasury_code: String,
    cgac_code: String,
    subfunc_code: String,
    subfunc_title: String,
    bea_category: String,
    on_off_budget: String,
    year_values: Vec<f64>,
}

// ─── CLI ─────────────────────────────────────────────────────────

struct Config {
    input: String,
    output: Option<String>,
    start_year: u32,
    end_year: u32,
    skip_all_zero: bool,
    discretionary_only: bool,
    positive_only: bool,
}

fn parse_args() -> Config {
    let args: Vec<String> = env::args().collect();
    let mut config = Config {
        input: String::new(),
        output: None,
        start_year: 2016,
        end_year: 2025,
        skip_all_zero: false,
        discretionary_only: false,
        positive_only: false,
    };
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--input" | "-i" => { i += 1; config.input = args[i].clone(); }
            "--output" | "-o" => { i += 1; config.output = Some(args[i].clone()); }
            "--start-year" => { i += 1; config.start_year = args[i].parse().expect("bad start-year"); }
            "--end-year" => { i += 1; config.end_year = args[i].parse().expect("bad end-year"); }
            "--skip-all-zero" => { config.skip_all_zero = true; }
            "--discretionary-only" => { config.discretionary_only = true; }
            "--positive-only" => { config.positive_only = true; }
            "--help" | "-h" => {
                eprintln!("Usage: tnv-csv-convert --input <xlsx> [--output <csv>] [--start-year N] [--end-year N] [--skip-all-zero] [--discretionary-only] [--positive-only]");
                std::process::exit(0);
            }
            _ => {
                if config.input.is_empty() { config.input = args[i].clone(); }
                else { eprintln!("Unknown arg: {}", args[i]); std::process::exit(1); }
            }
        }
        i += 1;
    }
    if config.input.is_empty() {
        eprintln!("Error: --input <xlsx> required");
        std::process::exit(1);
    }
    config
}

// ─── Cell helpers ────────────────────────────────────────────────

fn cell_str(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.trim().to_string(),
        Data::Float(f) => {
            if *f == (*f as i64) as f64 { format!("{}", *f as i64) }
            else { format!("{}", f) }
        }
        Data::Int(n) => format!("{}", n),
        Data::Bool(b) => format!("{}", b),
        Data::DateTime(d) => format!("{}", d),
        Data::DateTimeIso(s) => s.clone(),
        Data::DurationIso(s) => s.clone(),
        Data::Error(e) => format!("{:?}", e),
    }
}

fn cell_f64(cell: &Data) -> f64 {
    match cell {
        Data::Float(f) => *f,
        Data::Int(n) => *n as f64,
        Data::Empty => 0.0,
        _ => cell_str(cell).parse().unwrap_or(0.0),
    }
}

fn csv_quote(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn fmt_val(v: f64) -> String {
    let iv = v as i64;
    if (v - iv as f64).abs() < 0.01 { format!("{}", iv) }
    else { format!("{:.0}", v) }
}

fn fmt_vals(vals: &[f64]) -> String {
    vals.iter().map(|v| fmt_val(*v)).collect::<Vec<_>>().join(",")
}

// ─── Main ────────────────────────────────────────────────────────

fn main() {
    let config = parse_args();
    eprintln!("Reading: {}", config.input);

    let mut wb: Xlsx<_> = open_workbook(&config.input)
        .unwrap_or_else(|e| { eprintln!("Cannot open: {}", e); std::process::exit(1); });

    let sheet = wb.sheet_names().first().expect("no sheets").clone();
    let range = wb.worksheet_range(&sheet)
        .unwrap_or_else(|e| { eprintln!("Cannot read: {}", e); std::process::exit(1); });

    let (nrows, ncols) = range.get_size();
    eprintln!("Sheet '{}': {} rows x {} cols", sheet, nrows, ncols);

    // ─── Parse header & find year columns ────────────────────────

    let h0 = cell_str(&range[(0, 0)]);
    if h0 != "Agency Code" {
        eprintln!("Error: expected 'Agency Code' in A1, got '{}'", h0);
        std::process::exit(1);
    }

    let mut year_cols: Vec<(u32, usize)> = Vec::new();
    for c in COL_FIRST_YEAR..ncols {
        let s = cell_str(&range[(0, c)]);
        if s == "TQ" { continue; }
        if let Ok(y) = s.parse::<u32>() {
            if y >= config.start_year && y <= config.end_year {
                year_cols.push((y, c));
            }
        }
    }
    if year_cols.is_empty() {
        eprintln!("No year columns for {}-{}", config.start_year, config.end_year);
        std::process::exit(1);
    }
    let years: Vec<u32> = year_cols.iter().map(|(y, _)| *y).collect();
    eprintln!("Years: {:?}", years);
    let ny = year_cols.len();

    // ─── Pass 1: Read rows ───────────────────────────────────────

    let mut rows: Vec<BudgetRow> = Vec::with_capacity(nrows);
    for r in 1..nrows {
        let ac = cell_str(&range[(r, COL_AGENCY_CODE)]);
        if ac.is_empty() { continue; }

        let yv: Vec<f64> = year_cols.iter().map(|&(_, c)| cell_f64(&range[(r, c)])).collect();

        rows.push(BudgetRow {
            agency_code: ac,
            agency_name: cell_str(&range[(r, COL_AGENCY_NAME)]),
            bureau_code: cell_str(&range[(r, COL_BUREAU_CODE)]),
            bureau_name: cell_str(&range[(r, COL_BUREAU_NAME)]),
            account_code: cell_str(&range[(r, COL_ACCOUNT_CODE)]),
            account_name: cell_str(&range[(r, COL_ACCOUNT_NAME)]),
            treasury_code: cell_str(&range[(r, COL_TREASURY_CODE)]),
            cgac_code: cell_str(&range[(r, COL_CGAC_CODE)]),
            subfunc_code: cell_str(&range[(r, 8)]),
            subfunc_title: cell_str(&range[(r, 9)]),
            bea_category: cell_str(&range[(r, 10)]),
            on_off_budget: cell_str(&range[(r, 11)]),
            year_values: yv,
        });
    }
    eprintln!("Parsed {} rows", rows.len());

    // ─── Pass 1b: Discretionary-only filter ──────────────────────

    if config.discretionary_only {
        let before = rows.len();
        rows.retain(|r| r.bea_category == "Discretionary");
        eprintln!("Discretionary-only: kept {} of {} rows", rows.len(), before);
    }

    // ─── Pass 2: Deduplicate (sum same-key rows) ─────────────────

    type Key = (String, String, String, String, String, String);
    let mut deduped: BTreeMap<Key, BudgetRow> = BTreeMap::new();
    let mut dups = 0usize;

    for row in rows {
        let key: Key = (
            row.agency_code.clone(), row.bureau_code.clone(),
            row.account_code.clone(), row.subfunc_code.clone(),
            row.bea_category.clone(), row.on_off_budget.clone(),
        );
        if let Some(e) = deduped.get_mut(&key) {
            dups += 1;
            for (i, v) in row.year_values.iter().enumerate() { e.year_values[i] += v; }
            if e.account_name.is_empty() && !row.account_name.is_empty() { e.account_name = row.account_name; }
            if e.treasury_code.is_empty() && !row.treasury_code.is_empty() { e.treasury_code = row.treasury_code; }
            if e.cgac_code.is_empty() && !row.cgac_code.is_empty() { e.cgac_code = row.cgac_code; }
        } else {
            deduped.insert(key, row);
        }
    }
    let mut account_rows: Vec<BudgetRow> = deduped.into_values().collect();
    eprintln!("After dedup: {} rows ({} merged)", account_rows.len(), dups);

    // ─── Pass 2b: Positive-only filter ───────────────────────────
    // Drop accounts that are <= 0 in the active (last/end) year — offsetting
    // collections / receivables. Applied after dedup so summed duplicates count.
    if config.positive_only {
        let before = account_rows.len();
        account_rows.retain(|r| r.year_values.last().map(|&v| v > 0.0).unwrap_or(false));
        eprintln!("Positive-only (end year): kept {} of {} accounts", account_rows.len(), before);
    }

    // ─── Pass 3: Build parent summaries ──────────────────────────

    // Agency: code → (name, sums)
    let mut agency_totals: BTreeMap<String, (String, Vec<f64>)> = BTreeMap::new();
    // Bureau: (agency_code, bureau_code) → (agency_name, bureau_name, sums)
    let mut bureau_totals: BTreeMap<(String, String), (String, String, Vec<f64>)> = BTreeMap::new();
    let mut root_total = vec![0.0; ny];

    for row in &account_rows {
        for (i, v) in row.year_values.iter().enumerate() { root_total[i] += v; }

        let ae = agency_totals.entry(row.agency_code.clone())
            .or_insert_with(|| (row.agency_name.clone(), vec![0.0; ny]));
        for (i, v) in row.year_values.iter().enumerate() { ae.1[i] += v; }

        let be = bureau_totals.entry((row.agency_code.clone(), row.bureau_code.clone()))
            .or_insert_with(|| (row.agency_name.clone(), row.bureau_name.clone(), vec![0.0; ny]));
        for (i, v) in row.year_values.iter().enumerate() { be.2[i] += v; }
    }

    eprintln!("Agencies: {}, Bureaus: {}", agency_totals.len(), bureau_totals.len());

    // ─── Pass 4: Write CSV ───────────────────────────────────────

    let writer: Box<dyn Write> = if let Some(ref p) = config.output {
        Box::new(BufWriter::new(File::create(p)
            .unwrap_or_else(|e| { eprintln!("Cannot create {}: {}", p, e); std::process::exit(1); })))
    } else {
        Box::new(BufWriter::new(io::stdout()))
    };
    let mut w = writer;

    let yh = years.iter().map(|y| y.to_string()).collect::<Vec<_>>().join(",");
    writeln!(w, "Agency Code,Agency Name,Bureau Code,Bureau Name,Account Code,Account Name,Treasury Agency Code,CGAC Agency Code,Subfunction Code,Subfunction Title,BEA Category,On- or Off- Budget,{}", yh).unwrap();

    let mut written = 0usize;
    let mut skipped = 0usize;

    // Root row
    writeln!(w, ",,,,,,,,,,,,{}", fmt_vals(&root_total)).unwrap();
    written += 1;

    // Walk by agency (sorted by BTreeMap key)
    for (agency_code, (agency_name, agency_vals)) in &agency_totals {
        if config.skip_all_zero && agency_vals.iter().all(|v| *v == 0.0) {
            skipped += 1;
            continue;
        }

        // Agency total row
        writeln!(w, "{},{},,,,,,,,,,,{}", csv_quote(agency_code), csv_quote(agency_name), fmt_vals(agency_vals)).unwrap();
        written += 1;

        // Bureaus for this agency
        for ((ba, bb), (_, bname, bvals)) in &bureau_totals {
            if ba != agency_code { continue; }
            if config.skip_all_zero && bvals.iter().all(|v| *v == 0.0) {
                skipped += 1;
                continue;
            }
            writeln!(w, "{},{},{},{},,,,,,,,,{}", csv_quote(agency_code), csv_quote(agency_name),
                csv_quote(bb), csv_quote(bname), fmt_vals(bvals)).unwrap();
            written += 1;
        }

        // Account rows for this agency
        for row in &account_rows {
            if &row.agency_code != agency_code { continue; }
            if config.skip_all_zero && row.year_values.iter().all(|v| *v == 0.0) {
                skipped += 1;
                continue;
            }
            writeln!(w, "{},{},{},{},{},{},{},{},{},{},{},{},{}",
                csv_quote(&row.agency_code), csv_quote(&row.agency_name),
                csv_quote(&row.bureau_code), csv_quote(&row.bureau_name),
                csv_quote(&row.account_code), csv_quote(&row.account_name),
                csv_quote(&row.treasury_code), csv_quote(&row.cgac_code),
                csv_quote(&row.subfunc_code), csv_quote(&row.subfunc_title),
                csv_quote(&row.bea_category), csv_quote(&row.on_off_budget),
                fmt_vals(&row.year_values)).unwrap();
            written += 1;
        }
    }

    w.flush().unwrap();

    let n_accts = written - 1 - agency_totals.len() - bureau_totals.len();
    eprintln!("Written: {} rows (1 root + {} agencies + {} bureaus + {} accounts)",
        written, agency_totals.len(), bureau_totals.len(), n_accts);
    if skipped > 0 { eprintln!("Skipped all-zero: {}", skipped); }
    if let Some(ref p) = config.output { eprintln!("Output: {}", p); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csv_quote_plain() { assert_eq!(csv_quote("hello"), "hello"); }

    #[test]
    fn test_csv_quote_comma() { assert_eq!(csv_quote("hello, world"), "\"hello, world\""); }

    #[test]
    fn test_csv_quote_quotes() { assert_eq!(csv_quote("say \"hi\""), "\"say \"\"hi\"\"\""); }

    #[test]
    fn test_fmt_val_int() { assert_eq!(fmt_val(25000.0), "25000"); }

    #[test]
    fn test_fmt_val_negative() { assert_eq!(fmt_val(-1234.0), "-1234"); }

    #[test]
    fn test_fmt_val_zero() { assert_eq!(fmt_val(0.0), "0"); }

    #[test]
    fn test_fmt_vals() { assert_eq!(fmt_vals(&[1.0, -2.0, 0.0]), "1,-2,0"); }
}
