// src/topics.rs
//
// The 9 top-level "simple form" budget topics and the rules that assign each
// federal account to exactly one of them. See TNV_Category_Mapping.md.
//
// Assignment is by OMB agency code, with bureau-level overrides for the
// departments whose bureaus span multiple topics (Energy, Interior, Commerce,
// Agriculture). Anything not matched falls through to "Other".

pub struct Topic {
    /// Short id used in node ids, e.g. "def" -> node id "t:def".
    pub id: &'static str,
    pub label: &'static str,
}

/// Fixed display order of the 9 topics.
pub const TOPICS: [Topic; 9] = [
    Topic { id: "def",    label: "Defense" },
    Topic { id: "va",     label: "Veterans Affairs" },
    Topic { id: "edu",    label: "Education" },
    Topic { id: "health", label: "Health" },
    Topic { id: "infra",  label: "Infrastructure" },
    Topic { id: "sci",    label: "Science" },
    Topic { id: "env",    label: "Environment" },
    Topic { id: "dhs",    label: "Homeland Security" },
    Topic { id: "oth",    label: "Other" },
];

/// Topic for an account, given its OMB agency and bureau codes.
/// Bureau-level overrides take precedence over the agency-level default.
pub fn topic_for(agency_code: &str, bureau_code: &str) -> &'static str {
    match (agency_code, bureau_code) {
        // Energy (019): NNSA + defense environmental cleanup -> Defense;
        // remaining energy R&D programs -> Science.
        ("019", "05") | ("019", "10") => "def",
        ("019", "20") => "sci",
        // Interior (010): USGS -> Science; Indian Education -> Education;
        // Indian Affairs -> Other (rest of Interior defaults to Environment).
        ("010", "12") => "sci",
        ("010", "77") => "edu",
        ("010", "76") => "oth",
        // Commerce (006): NOAA + NIST -> Science; NTIA broadband -> Infrastructure.
        ("006", "48") | ("006", "55") => "sci",
        ("006", "60") => "infra",
        // Agriculture (005): Forest Service + NRCS -> Environment.
        ("005", "96") | ("005", "53") => "env",
        _ => topic_for_agency(agency_code),
    }
}

fn topic_for_agency(agency_code: &str) -> &'static str {
    match agency_code {
        "007" | "200" | "467" => "def", // DoD-Military, Other Defense-Civil, Intelligence
        "029" => "va",                  // Veterans Affairs
        "018" => "edu",                 // Education
        "009" => "health",              // HHS (discretionary)
        "024" => "dhs",                 // Homeland Security
        "021" | "202" => "infra",       // Transportation, Army Corps Civil Works
        "026" | "422" | "452" => "sci", // NASA, NSF, Smithsonian
        "020" | "010" => "env",         // EPA, Interior (default)
        _ => "oth",
    }
}
