pub const ALLOWED_TAGS: &[&str] = &[
    "alternative",
    "ambient",
    "blues",
    "classical",
    "comedy",
    "country",
    "culture",
    "dance",
    "disco",
    "drum and bass",
    "dubstep",
    "easy listening",
    "electronic",
    "experimental",
    "folk",
    "funk",
    "gospel",
    "gothic",
    "grime",
    "hardcore",
    "hip-hop",
    "house",
    "indie",
    "industrial",
    "jazz",
    "jungle",
    "latin",
    "lounge",
    "metal",
    "new wave",
    "news",
    "oldies",
    "pop",
    "public radio",
    "punk",
    "r&b",
    "reggae",
    "rock",
    "ska",
    "soul",
    "sport",
    "talk",
    "techno",
    "trance",
    "world",
];

pub fn normalize_tag(tag: &str) -> Option<String> {
    let tag = tag.trim().to_lowercase();
    let tag = tag.replace('_', " ");
    let tag = tag.split_whitespace().collect::<Vec<_>>().join(" ");

    let normalized = match tag.as_str() {
        "dnb" | "drum & bass" | "drum'n'bass" | "drum n bass" => "drum and bass",
        "rap" | "urban" => "hip-hop",
        "spoken word" | "speech" => "talk",
        "chillout" | "relaxation" => "ambient",
        "fusion" | "swing" => "jazz",
        "tropical" => "latin",
        "sports" => "sport",
        "student radio" | "community radio" | "college radio" => "public radio",
        "music" | "mainstream" | "variety" | "instrumental" | "guitar" | "online" | "radio"
        | "stream" | "streaming" | "live" | "fm" | "am" | "dab" | "hd" | "regional" | "local"
        | "international" | "catalan" | "italian" | "spanish" | "english" | "french" | "german"
        | "germany" | "uk" | "gb" | "france" | "spain" | "christian" | "business"
        | "traditional" | "progressive" => return None,
        other => other,
    };

    is_allowed(normalized).then(|| normalized.to_string())
}

pub fn is_allowed(tag: &str) -> bool {
    ALLOWED_TAGS.contains(&tag)
}

#[cfg(test)]
mod tests {
    use super::normalize_tag;

    #[test]
    fn maps_aliases() {
        assert_eq!(
            normalize_tag("drum & bass").as_deref(),
            Some("drum and bass")
        );
        assert_eq!(normalize_tag("rap").as_deref(), Some("hip-hop"));
        assert_eq!(normalize_tag("speech").as_deref(), Some("talk"));
        assert_eq!(normalize_tag("sports").as_deref(), Some("sport"));
    }

    #[test]
    fn drops_noise() {
        assert_eq!(normalize_tag("music"), None);
        assert_eq!(normalize_tag("regional"), None);
    }
}
