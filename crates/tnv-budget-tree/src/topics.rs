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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nine_topics_in_fixed_order() {
        assert_eq!(TOPICS.len(), 9);
        let ids: Vec<&str> = TOPICS.iter().map(|t| t.id).collect();
        assert_eq!(ids, vec!["def", "va", "edu", "health", "infra", "sci", "env", "dhs", "oth"]);
    }

    #[test]
    fn agency_level_defaults() {
        // Wholesale-mapped agencies (bureau code doesn't matter unless overridden).
        assert_eq!(topic_for("007", "00"), "def");   // DoD-Military
        assert_eq!(topic_for("200", "00"), "def");   // Other Defense-Civil
        assert_eq!(topic_for("467", "00"), "def");   // Intelligence
        assert_eq!(topic_for("029", "00"), "va");
        assert_eq!(topic_for("018", "00"), "edu");
        assert_eq!(topic_for("009", "00"), "health");
        assert_eq!(topic_for("024", "00"), "dhs");
        assert_eq!(topic_for("021", "00"), "infra");
        assert_eq!(topic_for("202", "00"), "infra"); // Army Corps Civil Works
        assert_eq!(topic_for("026", "00"), "sci");   // NASA
        assert_eq!(topic_for("422", "00"), "sci");   // NSF
        assert_eq!(topic_for("452", "00"), "sci");   // Smithsonian
        assert_eq!(topic_for("020", "00"), "env");   // EPA
        assert_eq!(topic_for("010", "99"), "env");   // Interior default (not an override)
    }

    #[test]
    fn energy_bureau_overrides() {
        assert_eq!(topic_for("019", "05"), "def"); // NNSA
        assert_eq!(topic_for("019", "10"), "def"); // Defense env activities
        assert_eq!(topic_for("019", "20"), "sci"); // Energy programs (Office of Science etc.)
        assert_eq!(topic_for("019", "60"), "oth"); // Departmental admin → default OTH
    }

    #[test]
    fn interior_bureau_overrides() {
        assert_eq!(topic_for("010", "12"), "sci"); // USGS
        assert_eq!(topic_for("010", "77"), "edu"); // Bureau of Indian Education
        assert_eq!(topic_for("010", "76"), "oth"); // Bureau of Indian Affairs
        assert_eq!(topic_for("010", "24"), "env"); // NPS → Interior default
    }

    #[test]
    fn commerce_bureau_overrides() {
        assert_eq!(topic_for("006", "48"), "sci");   // NOAA
        assert_eq!(topic_for("006", "55"), "sci");   // NIST
        assert_eq!(topic_for("006", "60"), "infra"); // NTIA (broadband)
        assert_eq!(topic_for("006", "07"), "oth");   // Census → default
    }

    #[test]
    fn agriculture_bureau_overrides() {
        assert_eq!(topic_for("005", "96"), "env"); // Forest Service
        assert_eq!(topic_for("005", "53"), "env"); // NRCS
        assert_eq!(topic_for("005", "84"), "oth"); // Food and Nutrition Service → default
    }

    #[test]
    fn unmapped_agency_falls_to_other() {
        assert_eq!(topic_for("999", "00"), "oth");
        assert_eq!(topic_for("", ""), "oth");
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
